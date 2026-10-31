use crate::app_id;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const ORIGIN: &str = "https://music.youtube.com";
pub const SESSION_KEPT: &str = if cfg!(windows) {
    "The session stays on this computer, encrypted with your Windows account."
} else {
    "The session stays on this computer, in a file only your user account can read."
};

fn sapisid(cookie: &str) -> Option<&str> {
    let value = |name: &str| cookie.split("; ").find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='));
    value("__Secure-3PAPISID").or_else(|| value("SAPISID"))
}

pub fn authorization(cookie: &str, unix_seconds: u64) -> Option<String> {
    let input = format!("{unix_seconds} {} {ORIGIN}", sapisid(cookie)?);
    let digest = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, input.as_bytes());
    let hex: String = digest.as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
    Some(format!("SAPISIDHASH {unix_seconds}_{hex}"))
}

fn session_file() -> Option<PathBuf> {
    Some(eframe::storage_dir(app_id())?.join("session.bin"))
}

pub fn store(cookie: &str) -> Result<(), String> {
    let sealed = seal(cookie.as_bytes(), true).ok_or("This sign-in cannot be kept after a restart on this system.")?;
    let file = session_file().ok_or("There is no folder to keep the sign-in in.")?;
    let mut private = std::fs::OpenOptions::new();
    private.create(true).write(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut private, 0o600);
    std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))
        .and_then(|()| private.open(file)?.write_all(&sealed))
        .map_err(|e| e.to_string())
}

pub fn load() -> Option<String> {
    String::from_utf8(seal(&std::fs::read(session_file()?).ok()?, false)?).ok()
}

pub fn forget() {
    if let Some(file) = session_file() {
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(windows)]
fn seal(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData, CryptUnprotectData};
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };
    unsafe {
        let done = if protect {
            CryptProtectData(&input, null(), null(), null(), null(), 0, &mut output)
        } else {
            CryptUnprotectData(&input, null_mut(), null(), null(), null(), 0, &mut output)
        };
        if done == 0 || output.pbData.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Some(bytes)
    }
}

#[cfg(not(windows))]
fn seal(data: &[u8], _protect: bool) -> Option<Vec<u8>> {
    Some(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_requests_the_way_youtube_expects() {
        let cookie = "SID=a; SAPISID=first; __Secure-3PAPISID=abc/123; LOGIN_INFO=x";
        assert_eq!(sapisid(cookie), Some("abc/123"));
        assert_eq!(sapisid("SID=a; SAPISIDX=no"), None);
        let expected = "SAPISIDHASH 1700000000_149e3c5c81746e107a88a2e0103de8a4b63040a3";
        assert_eq!(authorization(cookie, 1_700_000_000).as_deref(), Some(expected));
        assert_eq!(authorization("SID=a", 1_700_000_000), None);
    }

    #[cfg(windows)]
    #[test]
    fn sealed_sessions_round_trip_and_are_not_plain_text() {
        let sealed = seal(b"SAPISID=secret", true).unwrap();
        assert!(!sealed.windows(6).any(|window| window == b"secret"));
        assert_eq!(seal(&sealed, false).unwrap(), b"SAPISID=secret");
        assert!(seal(b"not a sealed blob", false).is_none());
    }
}
