//! Secrets owned by Zuno (OAuth tokens for accounts the user connects in Settings) are stored in
//! the Windows Credential Manager, which encrypts them with the user's logon credentials (DPAPI).
//! Nothing secret is written to Zuno's own files or logs.

use crate::error::{Error, Result};

const PREFIX: &str = "Zuno/";
/// CRED_MAX_CREDENTIAL_BLOB_SIZE is 2560 bytes; larger secrets are split across entries.
const CHUNK: usize = 2048;

fn target(key: &str, index: usize) -> String {
    format!("{PREFIX}{key}#{index}")
}

#[cfg(windows)]
mod sys {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Security::Credentials::*;

    pub fn write(target: &str, secret: &[u8]) -> windows::core::Result<()> {
        let target = HSTRING::from(target);
        let user = HSTRING::from("Zuno");
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_ptr() as *mut _),
            CredentialBlobSize: secret.len() as u32,
            CredentialBlob: secret.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_ptr() as *mut _),
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }
    }

    pub fn read(target: &str) -> Option<Vec<u8>> {
        let target = HSTRING::from(target);
        let mut ptr: *mut CREDENTIALW = std::ptr::null_mut();
        unsafe {
            CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut ptr).ok()?;
            let cred = &*ptr;
            let bytes = std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize).to_vec();
            CredFree(ptr as *const _);
            Some(bytes)
        }
    }

    pub fn delete(target: &str) {
        let target = HSTRING::from(target);
        unsafe {
            let _ = CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None);
        }
    }
}

/// Stores `secret` under `key`, replacing any previous value.
pub fn store(key: &str, secret: &str) -> Result<()> {
    #[cfg(windows)]
    {
        delete(key);
        let chunks: Vec<&[u8]> = secret.as_bytes().chunks(CHUNK).collect();
        let header = chunks.len().to_string();
        sys::write(&target(key, 0), header.as_bytes())?;
        for (i, chunk) in chunks.iter().enumerate() {
            sys::write(&target(key, i + 1), chunk)?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (key, secret);
        Err(Error::msg("Secure credential storage is only available on Windows"))
    }
}

pub fn load(key: &str) -> Result<Option<String>> {
    #[cfg(windows)]
    {
        let Some(header) = sys::read(&target(key, 0)) else { return Ok(None) };
        let count: usize = String::from_utf8_lossy(&header).parse().map_err(|_| Error::msg("Stored credential is corrupt"))?;
        let mut bytes = Vec::new();
        for i in 1..=count {
            bytes.extend(sys::read(&target(key, i)).ok_or_else(|| Error::msg("Stored credential is incomplete"))?);
        }
        Ok(Some(String::from_utf8(bytes).map_err(|_| Error::msg("Stored credential is corrupt"))?))
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        Ok(None)
    }
}

pub fn delete(key: &str) {
    #[cfg(windows)]
    {
        if let Some(header) = sys::read(&target(key, 0)) {
            let count: usize = String::from_utf8_lossy(&header).parse().unwrap_or(0);
            for i in 1..=count {
                sys::delete(&target(key, i));
            }
        }
        sys::delete(&target(key, 0));
    }
    #[cfg(not(windows))]
    let _ = key;
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn round_trip_including_large_secrets() {
        let key = format!("test/{}", std::process::id());
        let big = "x".repeat(5000) + "end";
        store(&key, &big).unwrap();
        assert_eq!(load(&key).unwrap().as_deref(), Some(big.as_str()));
        store(&key, "short").unwrap();
        assert_eq!(load(&key).unwrap().as_deref(), Some("short"));
        delete(&key);
        assert_eq!(load(&key).unwrap(), None);
    }
}
