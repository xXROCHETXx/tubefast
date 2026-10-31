//! Desktop OAuth: system browser, PKCE and a short-lived loopback callback.
//! Credentials never enter browser arguments, logs or the Music cookie header.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use url::Url;

pub const PREFIX: &str = "tubefast-oauth-v1:";
const SCOPE: &str = "https://www.googleapis.com/auth/youtube.readonly";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const WAIT: Duration = Duration::from_secs(5 * 60);
const MAX_RESPONSE: u64 = 64 * 1024;

#[derive(Clone, Deserialize, Serialize)]
pub struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    client_id: String,
    #[serde(default)]
    client_secret: String,
}

#[derive(Deserialize)]
struct Client {
    client_id: String,
    #[serde(default)]
    client_secret: String,
}

impl Client {
    fn parse(source: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Config {
            installed: Client,
        }
        let config: Config = serde_json::from_str(source).map_err(|_| configuration_error())?;
        if !config.installed.client_id.ends_with(".apps.googleusercontent.com") || config.installed.client_id.starts_with("YOUR_") {
            return Err(configuration_error());
        }
        Ok(config.installed)
    }

    fn load() -> Result<Self, String> {
        let path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("oauth-client.json")));
        if let Some(path) = path.filter(|p| p.exists()) {
            let source = std::fs::read_to_string(path).map_err(|_| configuration_error())?;
            return Self::parse(&source);
        }
        Self::parse(include_str!(concat!(env!("OUT_DIR"), "/oauth-client.json")))
    }
}

fn configuration_error() -> String {
    "OAuth needs setup by the fork owner: register a Google Cloud Desktop client and place oauth-client.json beside the app. See OAUTH.es.md.".to_owned()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn random() -> Result<String, String> {
    let mut bytes = [0; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Could not create a secure sign-in request.")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn authorization_url(client: &Client, redirect: &str, state: &str, verifier: &str) -> String {
    let challenge = URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()));
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client.client_id.as_str()),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("state", state),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    url.into()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(15))
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .timeout_write(Duration::from_secs(5))
        .build()
}

pub struct TokenError {
    pub rejected: bool,
    pub message: String,
}

fn token_request(fields: &[(&str, &str)]) -> Result<Value, TokenError> {
    let response = match agent().post(TOKEN_URL).send_form(fields) {
        Ok(response) => response,
        Err(ureq::Error::Status(status, _)) => return Err(TokenError {
            rejected: status == 400 || status == 401,
            message:
                "Google could not renew or complete this authorization. Sign in again; check the fork's OAuth configuration if it persists."
                    .to_owned(),
        }),
        Err(_) => {
            return Err(TokenError {
                rejected: false,
                message: "Could not reach Google. Check your connection and try again.".to_owned(),
            });
        }
    };
    let parsed = serde_json::from_reader(response.into_reader().take(MAX_RESPONSE));
    parsed.map_err(|_| TokenError {
        rejected: false,
        message: "Google returned an invalid authorization response.".to_owned(),
    })
}

fn token_fields(value: &Value) -> Result<(&str, u64), String> {
    let access = value["access_token"].as_str().filter(|v| !v.is_empty() && v.len() < 16384);
    let expiry = value["expires_in"].as_u64().filter(|v| *v > 0 && *v <= 86400);
    let bearer = value["token_type"].as_str().is_some_and(|v| v.eq_ignore_ascii_case("Bearer"));
    let scope_ok = value["scope"].as_str().is_none_or(|s| s.split_whitespace().any(|s| s == SCOPE));
    match (access, expiry, bearer, scope_ok) {
        (Some(access), Some(expiry), true, true) => Ok((access, now().saturating_add(expiry))),
        _ => Err("Google did not grant the requested YouTube read permission.".to_owned()),
    }
}

impl Session {
    pub fn parse(source: &str) -> Option<Self> {
        let session: Self = serde_json::from_str(source.strip_prefix(PREFIX)?).ok()?;
        (!session.access_token.is_empty()
            && !session.refresh_token.is_empty()
            && session.client_id.ends_with(".apps.googleusercontent.com"))
        .then_some(session)
    }

    pub fn encode(&self) -> String {
        format!("{PREFIX}{}", serde_json::to_string(self).unwrap())
    }
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
    pub fn needs_refresh(&self) -> bool {
        self.expires_at <= now().saturating_add(90)
    }

    pub fn local_account_key(&self) -> String {
        format!(
            "oauth:{}",
            URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, self.refresh_token.as_bytes()))
        )
    }

    pub fn refresh(&mut self) -> Result<(), TokenError> {
        let response = token_request(&[
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("refresh_token", &self.refresh_token),
            ("grant_type", "refresh_token"),
        ])?;
        self.accept_refresh(&response)
    }

    fn accept_refresh(&mut self, response: &Value) -> Result<(), TokenError> {
        let (token, expiry) = token_fields(response).map_err(|message| TokenError { rejected: true, message })?;
        self.access_token = token.to_owned();
        self.expires_at = expiry;
        if let Some(refresh) = response["refresh_token"].as_str().filter(|s| !s.is_empty()) {
            self.refresh_token = refresh.to_owned();
        }
        Ok(())
    }
}

// Validate state before interpreting either Google's code or error.
fn callback(target: &str, state: &str) -> Result<Option<Result<String, String>>, ()> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| ())?;
    if url.path() != "/oauth/callback" {
        return Ok(None);
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let one = |key: &str| {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == key).map(|(_, v)| v.as_ref()).collect();
        (values.len() == 1).then(|| values[0])
    };
    if one("state") != Some(state) {
        return Err(());
    }
    if one("error").is_some() {
        return Ok(Some(Err(
            "Authorization was declined or cancelled in Google. You can try again.".to_owned()
        )));
    }
    let code = one("code").filter(|v| !v.is_empty() && v.len() < 8192).ok_or(())?;
    Ok(Some(Ok(code.to_owned())))
}

fn reply(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
}

fn wait_for_code(listener: &TcpListener, state: &str, cancel: &AtomicBool) -> Result<String, String> {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline && !cancel.load(Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                let mut bytes = Vec::new();
                let mut chunk = [0; 1024];
                let request_deadline = Instant::now() + Duration::from_secs(2);
                while bytes.len() < 16384 && !bytes.ends_with(b"\r\n\r\n") && !cancel.load(Relaxed) && Instant::now() < request_deadline {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bytes.extend_from_slice(&chunk[..n]),
                    }
                }
                let request = String::from_utf8_lossy(&bytes);
                if !bytes.ends_with(b"\r\n\r\n") {
                    reply(&mut stream, "400 Bad Request", "Incomplete request.");
                    continue;
                }
                let mut words = request.lines().next().unwrap_or_default().split_whitespace();
                if words.next() != Some("GET") {
                    reply(&mut stream, "405 Method Not Allowed", "Use Google sign-in.");
                    continue;
                }
                let result = callback(words.next().unwrap_or_default(), state);
                match result {
                    Ok(Some(code)) => {
                        reply(
                            &mut stream,
                            "200 OK",
                            "<!doctype html><meta charset=utf-8><title>Tubefast</title><h1>Return to Tubefast</h1><p>You can close this tab. Tubefast will check the authorization.</p>",
                        );
                        return code;
                    }
                    Ok(None) => reply(&mut stream, "404 Not Found", "Not found."),
                    Err(()) => reply(
                        &mut stream,
                        "400 Bad Request",
                        "Invalid sign-in request. Return to Tubefast and try again.",
                    ),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => return Err("Could not receive Google's authorization. Try again.".to_owned()),
        }
    }
    Err(if cancel.load(Relaxed) {
        "Sign-in cancelled."
    } else {
        "Sign-in timed out. Try again."
    }
    .to_owned())
}

pub fn sign_in(cancel: &AtomicBool, open: impl FnOnce(String)) -> Result<String, String> {
    let client = Client::load()?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|_| "Could not open a local sign-in callback.")?;
    listener.set_nonblocking(true).map_err(|_| "Could not prepare sign-in.")?;
    let redirect = format!(
        "http://127.0.0.1:{}/oauth/callback",
        listener.local_addr().map_err(|_| "Could not prepare sign-in.")?.port()
    );
    let state = random()?;
    let verifier = random()?;
    if cancel.load(Relaxed) {
        return Err("Sign-in cancelled.".to_owned());
    }
    open(authorization_url(&client, &redirect, &state, &verifier));
    let code = wait_for_code(&listener, &state, cancel)?;
    if cancel.load(Relaxed) {
        return Err("Sign-in cancelled.".to_owned());
    }
    let response = token_request(&[
        ("client_id", &client.client_id),
        ("client_secret", &client.client_secret),
        ("code", &code),
        ("code_verifier", &verifier),
        ("redirect_uri", &redirect),
        ("grant_type", "authorization_code"),
    ])
    .map_err(|e| e.message)?;
    let (token, expiry) = token_fields(&response)?;
    let refresh = response["refresh_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Google did not provide offline access. Try authorizing again.")?;
    Ok(Session {
        access_token: token.to_owned(),
        refresh_token: refresh.to_owned(),
        expires_at: expiry,
        client_id: client.client_id,
        client_secret: client.client_secret,
    }
    .encode())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_uses_system_browser_flow_with_pkce_and_read_only_scope() {
        // RFC 7636, appendix B.
        let client = Client {
            client_id: "test.apps.googleusercontent.com".to_owned(),
            client_secret: String::new(),
        };
        let url = Url::parse(&authorization_url(
            &client,
            "http://127.0.0.1:1234/oauth/callback",
            "state",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        ))
        .unwrap();
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        assert_eq!(pairs["code_challenge"], "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(pairs["scope"], SCOPE);
        assert!(!pairs.contains_key("client_secret"));
    }

    #[test]
    fn callback_requires_state_even_for_denial_and_rejects_duplicates() {
        assert!(callback("/oauth/callback?state=wrong&code=secret", "expected").is_err());
        assert!(callback("/oauth/callback?error=access_denied", "expected").is_err());
        assert!(callback("/oauth/callback?state=s&state=s&code=x", "s").is_err());
        assert!(callback("/oauth/callback?state=s&code=x&code=y", "s").is_err());
        assert_eq!(
            callback("/oauth/callback?state=s&code=a%2Bb", "s").unwrap().unwrap().unwrap(),
            "a+b"
        );
        assert!(callback("/favicon.ico", "s").unwrap().is_none());
    }

    #[test]
    fn unknown_requests_do_not_consume_the_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || wait_for_code(&listener, "state", &AtomicBool::new(false)));
        for target in [
            "/favicon.ico",
            "/oauth/callback?state=wrong&code=bad",
            "/oauth/callback?state=state&code=good",
        ] {
            let mut stream = TcpStream::connect(addr).unwrap();
            stream
                .write_all(format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes())
                .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(!response.contains("code=good"));
        }
        assert_eq!(worker.join().unwrap().unwrap(), "good");
    }

    #[test]
    fn configuration_is_owned_by_the_fork_and_cancel_is_immediate() {
        assert!(Client::parse("{}").is_err());
        assert!(Client::parse(include_str!("../oauth-client.example.json")).is_err());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(
            wait_for_code(&listener, "state", &AtomicBool::new(true))
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(random().unwrap().len(), 43);
    }

    #[test]
    fn refresh_preserves_offline_access_and_accepts_rotation_only_after_validation() {
        let mut session = Session {
            access_token: "expired".to_owned(),
            refresh_token: "offline".to_owned(),
            expires_at: 0,
            client_id: "test.apps.googleusercontent.com".to_owned(),
            client_secret: String::new(),
        };
        assert!(session.needs_refresh());
        let response = serde_json::json!({"access_token":"renewed","token_type":"Bearer","expires_in":3600});
        assert!(session.accept_refresh(&response).is_ok());
        assert_eq!(session.refresh_token, "offline");
        assert!(!session.needs_refresh());
        assert!(session.accept_refresh(&serde_json::json!({"access_token":"wrong","token_type":"Bearer","expires_in":3600,"scope":"unrequested","refresh_token":"wrong"})).is_err());
        assert_eq!(session.refresh_token, "offline");
        let mut response = response;
        response["refresh_token"] = "rotated".into();
        assert!(session.accept_refresh(&response).is_ok());
        assert_eq!(Session::parse(&session.encode()).unwrap().refresh_token, "rotated");
        assert!(Session::parse("SAPISID=legacy").is_none());
    }
}
