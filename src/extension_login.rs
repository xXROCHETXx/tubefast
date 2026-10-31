use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

pub const EXTENSION_ID: &str = "mhcegfjhlladfhgpdoejkliodmkeooao";
pub const PORT: u16 = 17889;
const LIMIT: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Deserialize)]
struct Cookie {
    name: String,
    value: String,
    domain: String,
}

#[derive(Deserialize)]
struct Session {
    cookies: Vec<Cookie>,
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn trusted(&self, port: u16) -> bool {
        let expected = format!("chrome-extension://{EXTENSION_ID}");
        self.header("origin").is_none_or(|origin| origin == expected)
            && (self.method == "OPTIONS" || self.header("x-tubefast-extension") == Some(EXTENSION_ID))
            && self.header("host") == Some(format!("127.0.0.1:{port}").as_str())
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while !bytes.ends_with(b"\r\n\r\n") {
        if bytes.len() >= 8192 {
            return None;
        }
        stream.read_exact(&mut byte).ok()?;
        bytes.push(byte[0]);
    }
    let head = std::str::from_utf8(&bytes).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_owned();
    let path = first.next()?.to_owned();
    if first.next()? != "HTTP/1.1" || first.next().is_some() {
        return None;
    }
    let mut headers = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (key, value) = line.split_once(':')?;
        if headers
            .iter()
            .any(|(existing, _): &(String, String)| existing.eq_ignore_ascii_case(key))
        {
            return None;
        }
        headers.push((key.to_owned(), value.trim().to_owned()));
    }
    let mut request = Request {
        method,
        path,
        headers,
        body: Vec::new(),
    };
    if request.header("transfer-encoding").is_some() {
        return None;
    }
    let length = match request.header("content-length") {
        Some(value) => value.parse::<usize>().ok()?,
        None => 0,
    };
    if length > LIMIT {
        return None;
    }
    request.body.resize(length, 0);
    stream.read_exact(&mut request.body).ok()?;
    Some(request)
}

fn respond(stream: &mut TcpStream, status: u16, body: Value, cors: bool) {
    let body = body.to_string();
    let origin = if cors {
        format!(
            "Access-Control-Allow-Origin: chrome-extension://{EXTENSION_ID}\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: authorization, content-type, x-tubefast-extension\r\n"
        )
    } else {
        String::new()
    };
    let response = format!(
        "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nVary: Origin\r\n{origin}\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn cookie_header(body: &[u8]) -> Option<String> {
    let session: Session = serde_json::from_slice(body).ok()?;
    if session.cookies.len() > 128 {
        return None;
    }
    let valid = |cookie: &Cookie| {
        (cookie.domain == "youtube.com" || cookie.domain.ends_with(".youtube.com"))
            && !cookie.name.is_empty()
            && !cookie.value.is_empty()
            && cookie
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
            && !cookie.value.chars().any(|c| c.is_control() || c == ';')
    };
    let cookies: Vec<_> = session.cookies.into_iter().filter(valid).collect();
    if !cookies
        .iter()
        .any(|cookie| cookie.name == "SAPISID" || cookie.name == "__Secure-3PAPISID")
    {
        return None;
    }
    Some(
        cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

fn nonce() -> Result<String, String> {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut random = [0u8; 32];
    SystemRandom::new()
        .fill(&mut random)
        .map_err(|_| "No se pudo preparar la conexión segura.")?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn wait<T>(
    listener: TcpListener,
    cancel: &AtomicBool,
    token: &str,
    mut verify: impl FnMut(&str) -> Result<T, String>,
) -> Result<(T, String), String> {
    listener
        .set_nonblocking(true)
        .map_err(|_| "No se pudo preparar la conexión local.")?;
    let port = listener.local_addr().map_err(|_| "No se pudo preparar la conexión local.")?.port();
    let started = Instant::now();
    while !cancel.load(Relaxed) && started.elapsed() < TIMEOUT {
        let (mut stream, _) = match listener.accept() {
            Ok(incoming) => incoming,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(_) => return Err("Se interrumpió la conexión con el navegador.".into()),
        };
        let Some(request) = read_request(&mut stream) else {
            respond(&mut stream, 400, json!({"error":"Solicitud no válida."}), false);
            continue;
        };
        if !request.trusted(port) {
            respond(&mut stream, 403, json!({"error":"Origen no autorizado."}), false);
            continue;
        }
        match (request.method.as_str(), request.path.as_str()) {
            ("OPTIONS", "/v1/challenge" | "/v1/session") => respond(&mut stream, 200, json!({}), true),
            ("GET", "/v1/challenge") => respond(&mut stream, 200, json!({"app":"Tubefast Stats","nonce":token}), true),
            ("POST", "/v1/session") => {
                if request.header("authorization") != Some(format!("Bearer {token}").as_str()) {
                    respond(
                        &mut stream,
                        403,
                        json!({"error":"La solicitud de conexión caducó. Vuelve a pulsar Conectar."}),
                        true,
                    );
                    continue;
                }
                if request
                    .header("content-type")
                    .is_none_or(|content| !content.starts_with("application/json"))
                {
                    respond(&mut stream, 400, json!({"error":"Formato no válido."}), true);
                    continue;
                }
                let Some(cookie) = cookie_header(&request.body) else {
                    respond(
                        &mut stream,
                        400,
                        json!({"error":"Inicia sesión en YouTube Music en este navegador y vuelve a pulsar Conectar."}),
                        true,
                    );
                    continue;
                };
                match verify(&cookie) {
                    Ok(account) if !cancel.load(Relaxed) => {
                        respond(&mut stream, 200, json!({"ok":true}), true);
                        return Ok((account, cookie));
                    }
                    Ok(_) => {
                        respond(&mut stream, 409, json!({"error":"Conexión cancelada en Tubefast."}), true);
                        return Err(String::new());
                    }
                    Err(error) => {
                        respond(
                            &mut stream,
                            422,
                            json!({"error":"YouTube no aceptó la sesión. Comprueba tu cuenta en YouTube Music y vuelve a intentarlo."}),
                            true,
                        );
                        return Err(error);
                    }
                }
            }
            _ => respond(&mut stream, 404, json!({"error":"Ruta no disponible."}), true),
        }
    }
    if cancel.load(Relaxed) {
        Err(String::new())
    } else {
        Err("Abre Tubefast Connect en tu navegador y pulsa Conectar para terminar.".into())
    }
}

pub fn sign_in<T>(cancel: &AtomicBool, ready: impl FnOnce(), verify: impl FnMut(&str) -> Result<T, String>) -> Result<(T, String), String> {
    if cancel.load(Relaxed) {
        return Err(String::new());
    }
    let listener =
        TcpListener::bind(("127.0.0.1", PORT)).map_err(|_| "Ya hay otra conexión de Tubefast en curso. Ciérrala y vuelve a intentarlo.")?;
    let token = nonce()?;
    ready();
    wait(listener, cancel, &token, verify)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::Arc;
    struct Server {
        address: SocketAddr,
        cancel: Arc<AtomicBool>,
        worker: Option<std::thread::JoinHandle<Result<(String, String), String>>>,
    }
    impl Server {
        fn new() -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let address = listener.local_addr().unwrap();
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = cancel.clone();
            let worker = std::thread::spawn(move || wait(listener, &flag, "test-token", |cookie| Ok(cookie.to_owned())));
            Self {
                address,
                cancel,
                worker: Some(worker),
            }
        }
        fn call(&self, method: &str, path: &str, origin: &str, token: &str, body: &str) -> String {
            let mut stream = TcpStream::connect(self.address).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            write!(stream, "{method} {path} HTTP/1.1\r\nHost: {}\r\nOrigin: {origin}\r\nX-Tubefast-Extension: {EXTENSION_ID}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", self.address, body.len()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.cancel.store(true, Relaxed);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
    fn origin() -> String {
        format!("chrome-extension://{EXTENSION_ID}")
    }
    fn fixture() -> String {
        json!({"cookies":[
            {"name":"SAPISID","value":"synthetic","domain":".youtube.com"},
            {"name":"SID","value":"foreign","domain":".google.com"},
            {"name":"SID","value":"foreign","domain":"youtube.com.evil.test"}
        ]})
        .to_string()
    }
    #[test]
    fn imports_only_youtube_cookies_and_rejects_header_injection() {
        assert_eq!(cookie_header(fixture().as_bytes()).as_deref(), Some("SAPISID=synthetic"));
        let bad = json!({"cookies":[{"name":"SAPISID","value":"a; SID=injected","domain":".youtube.com"}]}).to_string();
        assert!(cookie_header(bad.as_bytes()).is_none());
        assert!(cookie_header(br#"{"cookies":[]}"#).is_none());
    }
    #[test]
    fn unrelated_websites_cannot_read_the_challenge() {
        let server = Server::new();
        let response = server.call("GET", "/v1/challenge", "https://evil.test", "", "");
        assert!(response.starts_with("HTTP/1.1 403"));
        assert!(!response.contains("test-token") && !response.contains("Access-Control-Allow-Origin"));
    }
    #[test]
    fn requires_the_current_attempt_token_before_accepting_a_session() {
        let mut server = Server::new();
        assert!(
            server
                .call("POST", "/v1/session", &origin(), "previous-token", &fixture())
                .starts_with("HTTP/1.1 403")
        );
        assert!(server.call("GET", "/v1/challenge", &origin(), "", "").contains("test-token"));
        assert!(
            server
                .call("POST", "/v1/session", &origin(), "test-token", &fixture())
                .starts_with("HTTP/1.1 200")
        );
        let (verified, cookie) = server.worker.take().unwrap().join().unwrap().unwrap();
        assert_eq!(verified, "SAPISID=synthetic");
        assert_eq!(cookie, verified);
    }
    #[test]
    fn cancelling_closes_the_local_listener() {
        let mut server = Server::new();
        server.cancel.store(true, Relaxed);
        assert!(server.worker.take().unwrap().join().unwrap().is_err());
        assert!(TcpStream::connect(server.address).is_err());
    }
    #[test]
    fn new_attempts_have_different_unpredictable_tokens() {
        let first = nonce().unwrap();
        let second = nonce().unwrap();
        assert_eq!(first.len(), 64);
        assert_ne!(first, second);
    }
}
