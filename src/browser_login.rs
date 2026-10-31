use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

const LOGIN_URL: &str = "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com%2F";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(600);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const DISCONNECT_GRACE: Duration = Duration::from_secs(5);
const POLL_EVERY: Duration = Duration::from_millis(500);
const VERIFY_EVERY: Duration = Duration::from_secs(8);
const MAX_MESSAGE_BYTES: u64 = 64 * 1024 * 1024;
const BROWSER_CLOSED: &str = "Se cerró la ventana de acceso. Pulsa Conectar con Google para volver a intentarlo.";
const BROWSER_UNREACHABLE: &str = "No se pudo conectar con el navegador. Cierra la ventana de acceso y vuelve a intentarlo.";
const BROWSERS: [(&str, &str); 4] = [
    ("ProgramFiles(x86)", r"Microsoft\Edge\Application\msedge.exe"),
    ("ProgramFiles", r"Microsoft\Edge\Application\msedge.exe"),
    ("ProgramFiles", r"Google\Chrome\Application\chrome.exe"),
    ("LocalAppData", r"Google\Chrome\Application\chrome.exe"),
];
const BROWSERS_ELSEWHERE: [&str; 7] = [
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
];

struct Socket {
    stream: TcpStream,
    calls: u64,
}

impl Socket {
    fn open(port: u16) -> Option<Socket> {
        let version: Value = ureq::get(&format!("http://127.0.0.1:{port}/json/version"))
            .timeout(Duration::from_secs(2))
            .call()
            .ok()?
            .into_json()
            .ok()?;
        let address = version["webSocketDebuggerUrl"].as_str()?;
        let path = &address[address.find("/devtools/")?..];
        let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
        stream.set_write_timeout(Some(Duration::from_secs(3))).ok()?;
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).ok()?;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if head.len() >= 8192 {
                return None;
            }
            stream.read_exact(&mut byte).ok()?;
            head.push(byte[0]);
        }
        head.starts_with(b"HTTP/1.1 101").then_some(Socket { stream, calls: 0 })
    }

    fn call(&mut self, method: &str, params: Value) -> io::Result<Value> {
        self.calls += 1;
        let text = json!({ "id": self.calls, "method": method, "params": params }).to_string();
        let mut frame = vec![0x81];
        match text.len() {
            length @ 0..=125 => frame.push(0x80 | length as u8),
            length @ 126..=65535 => {
                frame.push(0x80 | 126);
                frame.extend((length as u16).to_be_bytes());
            }
            length => {
                frame.push(0x80 | 127);
                frame.extend((length as u64).to_be_bytes());
            }
        }
        frame.extend([0u8; 4]);
        frame.extend(text.as_bytes());
        self.stream.write_all(&frame)?;
        loop {
            let reply: Value = serde_json::from_slice(&self.message()?).unwrap_or_default();
            if reply["id"] == self.calls {
                return Ok(reply);
            }
        }
    }

    fn message(&mut self) -> io::Result<Vec<u8>> {
        let mut message = Vec::new();
        loop {
            let mut head = [0u8; 2];
            self.stream.read_exact(&mut head)?;
            let length = match head[1] & 0x7F {
                126 => {
                    let mut wide = [0u8; 2];
                    self.stream.read_exact(&mut wide)?;
                    u16::from_be_bytes(wide) as u64
                }
                127 => {
                    let mut wide = [0u8; 8];
                    self.stream.read_exact(&mut wide)?;
                    u64::from_be_bytes(wide)
                }
                short => short as u64,
            };
            if length > MAX_MESSAGE_BYTES {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let mut payload = vec![0u8; length as usize];
            self.stream.read_exact(&mut payload)?;
            match head[0] & 0x0F {
                0x0..=0x2 => {
                    if message.len() as u64 + payload.len() as u64 > MAX_MESSAGE_BYTES {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    message.extend(payload);
                }
                0x8 => return Err(io::ErrorKind::ConnectionAborted.into()),
                _ => continue,
            }
            if head[0] & 0x80 != 0 {
                return Ok(message);
            }
        }
    }
}

fn youtube_cookie(reply: &Value) -> Option<String> {
    let on_youtube = |cookie: &&Value| {
        cookie["domain"]
            .as_str()
            .is_some_and(|domain| domain == "youtube.com" || domain.ends_with(".youtube.com"))
    };
    let pairs: Vec<String> = reply["result"]["cookies"]
        .as_array()?
        .iter()
        .filter(on_youtube)
        .filter_map(|cookie| Some(format!("{}={}", cookie["name"].as_str()?, cookie["value"].as_str()?)))
        .collect();
    let cookie = pairs.join("; ");
    cookie
        .split("; ")
        .any(|pair| pair.starts_with("SAPISID=") || pair.starts_with("__Secure-3PAPISID="))
        .then_some(cookie)
}

#[cfg(windows)]
fn default_browser() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    #[link(name = "shlwapi")]
    unsafe extern "system" {
        fn AssocQueryStringW(flags: u32, string: u32, association: *const u16, extra: *const u16, out: *mut u16, length: *mut u32) -> i32;
    }
    let mut path = [0u16; 4096];
    let mut length = path.len() as u32;
    let association: Vec<u16> = "https\0".encode_utf16().collect();
    let result = unsafe { AssocQueryStringW(0x1000, 2, association.as_ptr(), std::ptr::null(), path.as_mut_ptr(), &mut length) };
    if result != 0 || length < 2 || length as usize > path.len() {
        return None;
    }
    let path = PathBuf::from(std::ffi::OsString::from_wide(&path[..length as usize - 1]));
    let name = path.file_name()?.to_str()?;
    (name.eq_ignore_ascii_case("msedge.exe") || name.eq_ignore_ascii_case("chrome.exe")).then_some(path)
}

#[cfg(not(windows))]
fn default_browser() -> Option<PathBuf> {
    None
}

struct Profile {
    path: PathBuf,
    root: PathBuf,
}

impl Profile {
    fn new(root: &Path) -> Result<Self, String> {
        use ring::rand::{SecureRandom, SystemRandom};
        std::fs::create_dir_all(root).map_err(|_| "No se pudo preparar el acceso en este equipo.")?;
        let mut nonce = [0u8; 16];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "No se pudo preparar una conexión segura.")?;
        let name: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = std::path::absolute(root)
            .map_err(|_| "La carpeta de acceso no es válida.")?
            .join(format!("signin-{}-{name}", std::process::id()));
        let root = root.canonicalize().map_err(|_| "La carpeta de acceso no está disponible.")?;
        std::fs::create_dir(&path).map_err(|_| "No se pudo crear el perfil de acceso.")?;
        Ok(Self { path, root })
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        for _ in 0..30 {
            let Ok(resolved) = self.path.canonicalize() else { break };
            if resolved.parent() != Some(self.root.as_path()) {
                break;
            }
            if std::fs::remove_dir_all(&resolved).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

fn command(program: &Path, profile: &Path, extra: &[&str]) -> Command {
    let mut command = Command::new(program);
    command
        .arg(format!("--user-data-dir={}", profile.display()))
        .args([
            "--remote-debugging-port=0",
            "--remote-debugging-address=127.0.0.1",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-sync",
            "--disable-background-mode",
        ])
        .args(extra)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

struct Browser {
    launcher: Child,
    profile: Profile,
    port: Option<u16>,
}

impl Browser {
    fn launch(root: &Path, extra: &[&str]) -> Result<Self, String> {
        let installed = BROWSERS
            .iter()
            .filter_map(|(root, path)| Some(PathBuf::from(std::env::var_os(root)?).join(path)));
        let programs: Vec<PathBuf> = if cfg!(windows) {
            default_browser().into_iter().chain(installed).collect()
        } else {
            BROWSERS_ELSEWHERE.iter().map(PathBuf::from).collect()
        };
        let profile = Profile::new(root)?;
        let launcher = programs
            .iter()
            .find_map(|program| command(program, &profile.path, extra).spawn().ok())
            .ok_or("Necesitas Microsoft Edge o Google Chrome instalado para conectar la cuenta automáticamente.")?;
        Ok(Self {
            launcher,
            profile,
            port: None,
        })
    }

    fn connect(&mut self) -> Option<Socket> {
        if self.port.is_none() {
            let active = std::fs::read_to_string(self.profile.path.join("DevToolsActivePort")).ok()?;
            let mut lines = active.lines();
            let port = lines.next()?.parse::<u16>().ok().filter(|port| *port > 0)?;
            if !lines.next()?.starts_with("/devtools/browser/") {
                return None;
            }
            self.port = Some(port);
        }
        Socket::open(self.port?)
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        for _ in 0..10 {
            if let Some(mut connection) = self.connect() {
                let _ = connection.call("Browser.close", json!({}));
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.launcher.kill();
        let _ = self.launcher.wait();
    }
}

struct Liveness {
    started: Instant,
    last_reply: Option<Instant>,
    last_page: Option<Instant>,
}

impl Liveness {
    fn connection_error(&self, now: Instant) -> Option<&'static str> {
        match self.last_reply {
            Some(last) if now.duration_since(last) >= DISCONNECT_GRACE => Some(BROWSER_CLOSED),
            None if now.duration_since(self.started) >= STARTUP_TIMEOUT => Some(BROWSER_UNREACHABLE),
            _ => None,
        }
    }

    fn page_error(&self, now: Instant) -> Option<&'static str> {
        match self.last_page {
            Some(last) if now.duration_since(last) >= DISCONNECT_GRACE => Some(BROWSER_CLOSED),
            None if now.duration_since(self.started) >= STARTUP_TIMEOUT => Some(BROWSER_UNREACHABLE),
            _ => None,
        }
    }
}

fn watch<T>(browser: &mut Browser, cancel: &AtomicBool, mut verify: impl FnMut(&str) -> Result<T, String>) -> Result<(T, String), String> {
    let started = Instant::now();
    let mut live = Liveness {
        started,
        last_reply: None,
        last_page: None,
    };
    let mut socket: Option<Socket> = None;
    let mut previous = String::new();
    let mut stable = 0;
    let mut last_verify: Option<Instant> = None;
    let mut failures = 0;
    loop {
        if cancel.load(Relaxed) {
            return Err(String::new());
        }
        if started.elapsed() >= LOGIN_TIMEOUT {
            return Err("El acceso tardó demasiado. Vuelve a intentarlo cuando estés listo.".into());
        }
        std::thread::sleep(POLL_EVERY);
        if socket.is_none() {
            socket = browser.connect();
        }
        let reply = socket.as_mut().map(|connection| {
            let pages = connection.call("Target.getTargets", json!({}))?;
            let cookies = connection.call("Storage.getCookies", json!({}))?;
            Ok::<_, io::Error>((pages, cookies))
        });
        let (pages, cookies) = match reply {
            Some(Ok(reply)) => reply,
            _ => {
                socket = None;
                if let Some(error) = live.connection_error(Instant::now()) {
                    return Err(error.into());
                }
                continue;
            }
        };
        if !pages["error"].is_null() || !cookies["error"].is_null() {
            return Err(
                "El navegador no permite conectar la sesión. Comprueba sus restricciones o prueba con otro navegador compatible.".into(),
            );
        }
        let now = Instant::now();
        live.last_reply = Some(now);
        if pages["result"]["targetInfos"]
            .as_array()
            .is_some_and(|pages| pages.iter().any(|page| page["type"] == "page"))
        {
            live.last_page = Some(now);
        } else if let Some(error) = live.page_error(now) {
            return Err(error.into());
        }
        let Some(cookie) = youtube_cookie(&cookies) else {
            previous.clear();
            stable = 0;
            continue;
        };
        if cookie == previous {
            stable += 1;
        } else {
            previous = cookie.clone();
            stable = 1;
            last_verify = None;
            failures = 0;
        }
        let music_open = pages["result"]["targetInfos"].as_array().is_some_and(|pages| {
            pages.iter().any(|page| {
                page["url"]
                    .as_str()
                    .and_then(|url| url.strip_prefix("https://music.youtube.com"))
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'))
            })
        });
        if !cookie.contains("LOGIN_INFO=") && !music_open {
            continue;
        }
        if stable < 3 || last_verify.is_some_and(|at| at.elapsed() < VERIFY_EVERY) {
            continue;
        }
        last_verify = Some(Instant::now());
        match verify(&cookie) {
            Ok(account) => {
                if cancel.load(Relaxed) {
                    return Err(String::new());
                }
                return Ok((account, cookie));
            }
            Err(error) => {
                failures += 1;
                if failures >= 3 {
                    return Err(format!("No se pudo conectar la cuenta con YouTube. {error}"));
                }
            }
        }
    }
}

pub fn sign_in<T>(root: &Path, cancel: &AtomicBool, verify: impl FnMut(&str) -> Result<T, String>) -> Result<(T, String), String> {
    if cancel.load(Relaxed) {
        return Err(String::new());
    }
    let mut browser = Browser::launch(root, &["--new-window", "--window-size=1060,820", LOGIN_URL])?;
    watch(&mut browser, cancel, verify)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../browser-login-tests")
    }

    fn fake_cookies(value: &str) -> Value {
        json!({"cookies": [
            {"name":"SAPISID", "value":value, "domain":".youtube.com", "path":"/", "secure":true},
            {"name":"LOGIN_INFO", "value":"synthetic", "domain":".youtube.com", "path":"/", "secure":true},
            {"name":"SAPISID", "value":"never-import-this", "domain":".example.com", "path":"/", "secure":true}
        ]})
    }

    fn ready(browser: &mut Browser) -> Socket {
        (0..150)
            .find_map(|_| {
                let socket = browser.connect();
                if socket.is_none() {
                    std::thread::sleep(Duration::from_millis(100));
                }
                socket
            })
            .expect("the isolated test browser did not open its control channel")
    }

    #[test]
    fn every_attempt_gets_its_own_profile_and_only_that_profile_is_removed() {
        let first = Profile::new(&root()).unwrap();
        let second = Profile::new(&root()).unwrap();
        let first_path = first.path.clone();
        let second_path = second.path.clone();
        assert_ne!(first_path, second_path);
        drop(first);
        assert!(!first_path.exists());
        assert!(second_path.exists());
        drop(second);
        assert!(!second_path.exists());
    }

    #[test]
    fn uses_a_normal_window_with_isolated_extensions_and_no_fixed_port() {
        let command = command(Path::new("chrome"), Path::new("example-profile"), &["--new-window", LOGIN_URL]);
        let args: Vec<_> = command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
        assert!(args.iter().any(|arg| arg == "--new-window"));
        assert!(args.iter().any(|arg| arg == "--disable-extensions"));
        assert!(args.iter().any(|arg| arg == "--remote-debugging-port=0"));
        assert!(!args.iter().any(|arg| arg.starts_with("--app=")));
    }

    #[test]
    fn ignores_cookies_from_unrelated_or_lookalike_domains() {
        let reply = json!({"result":{"cookies":[
            {"name":"SAPISID","value":"real-test","domain":".youtube.com"},
            {"name":"SID","value":"foreign","domain":"youtube.com.example.com"},
            {"name":"SID","value":"foreign","domain":"notyoutube.com"},
            {"name":"SID","value":"foreign","domain":".google.com"}
        ]}});
        assert_eq!(youtube_cookie(&reply).as_deref(), Some("SAPISID=real-test"));
    }

    #[test]
    fn waits_for_startup_and_tolerates_brief_disconnects() {
        let now = Instant::now();
        let mut live = Liveness {
            started: now,
            last_reply: None,
            last_page: None,
        };
        assert_eq!(live.connection_error(now + STARTUP_TIMEOUT - Duration::from_millis(1)), None);
        assert_eq!(live.connection_error(now + STARTUP_TIMEOUT), Some(BROWSER_UNREACHABLE));
        live.last_reply = Some(now);
        assert_eq!(live.connection_error(now + DISCONNECT_GRACE - Duration::from_millis(1)), None);
        assert_eq!(live.connection_error(now + DISCONNECT_GRACE), Some(BROWSER_CLOSED));
        live.last_page = Some(now);
        assert_eq!(live.page_error(now + DISCONNECT_GRACE), Some(BROWSER_CLOSED));
    }

    #[test]
    #[ignore = "needs an installed Edge or Chrome; uses synthetic cookies only"]
    fn reads_verified_session_even_when_the_launcher_process_has_exited() {
        let mut browser = Browser::launch(&root(), &["--headless=new", "about:blank"]).unwrap();
        let mut socket = ready(&mut browser);
        assert!(socket.call("Storage.setCookies", fake_cookies("synthetic")).unwrap()["error"].is_null());
        let mut exited = Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        exited.wait().unwrap();
        browser.launcher = exited;
        let result = watch(&mut browser, &AtomicBool::new(false), |cookie| {
            assert!(cookie.contains("SAPISID=synthetic") && !cookie.contains("never-import-this"));
            Ok::<_, String>("verified-test-account")
        })
        .unwrap();
        assert_eq!(result.0, "verified-test-account");
        let profile = browser.profile.path.clone();
        drop(socket);
        drop(browser);
        assert!(!profile.exists());
    }

    #[test]
    #[ignore = "needs an installed Edge or Chrome; opens about:blank only"]
    fn cancelling_closes_the_owned_browser_and_removes_its_profile() {
        let mut browser = Browser::launch(&root(), &["--headless=new", "about:blank"]).unwrap();
        drop(ready(&mut browser));
        let port = browser.port.unwrap();
        let result = watch(&mut browser, &AtomicBool::new(true), |_| Ok::<_, String>(()));
        assert_eq!(result, Err(String::new()));
        let profile = browser.profile.path.clone();
        drop(browser);
        assert!(!profile.exists());
        assert!(Socket::open(port).is_none());
    }

    #[test]
    #[ignore = "needs an installed Edge or Chrome; uses synthetic cookies only"]
    fn keeps_the_browser_open_until_the_session_is_actually_verified() {
        let mut browser = Browser::launch(&root(), &["--headless=new", "about:blank"]).unwrap();
        let mut socket = ready(&mut browser);
        socket.call("Storage.setCookies", fake_cookies("before-login")).unwrap();
        let tries = Cell::new(0);
        let result = watch(&mut browser, &AtomicBool::new(false), |cookie| {
            tries.set(tries.get() + 1);
            if tries.get() == 1 {
                socket.call("Storage.setCookies", fake_cookies("after-login")).unwrap();
                return Err("not ready".to_owned());
            }
            assert!(cookie.contains("SAPISID=after-login"));
            Ok(())
        });
        assert!(result.is_ok() && tries.get() >= 2);
    }
}
