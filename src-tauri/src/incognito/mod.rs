//! Incognito windows.
//!
//! An incognito window is a separate Zuno process started with [`ARG`]. Persistence is switched
//! off at the source instead of being cleaned up afterwards:
//! * websites run in WebView2 InPrivate mode inside a throwaway user-data folder ([`Session`])
//!   that is deleted once the process has exited, and swept on the next start if Zuno crashed;
//! * Zuno's database is an in-memory copy holding only the bookmarks, so history, favicons, the
//!   download list and the session are never written; settings are read but never saved, the
//!   window position is not remembered and no log file is written;
//! * downloaded files keep their Mark of the Web (Windows still warns before opening them) but
//!   lose the page and address they came from ([`scrub_download_origin`]);
//! * the Windows DNS cache, which lists recently resolved host names, is flushed on exit;
//! * optionally, active network adapters get a random MAC address while incognito windows are
//!   open ([`mac`]).

pub mod mac;

use std::fs::File;
use std::path::{Path, PathBuf};

use base64::Engine as _;

use crate::error::Result;

/// Command-line flag that starts Zuno as an incognito window.
pub const ARG: &str = "--incognito";

/// Held open (without sharing) for as long as a session's folder is in use.
const LOCK_FILE: &str = "in-use";

pub fn requested() -> bool {
    std::env::args().skip(1).any(|a| a == ARG)
}

/// Starts a new incognito window.
pub fn open_window() -> Result<()> {
    std::process::Command::new(std::env::current_exe()?).arg(ARG).spawn()?;
    Ok(())
}

/// Where incognito sessions keep their throwaway WebView2 folders.
pub fn root(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join("Incognito")
}

/// The scratch folder of one incognito process.
pub struct Session {
    pub dir: PathBuf,
    _lock: File,
}

impl Session {
    pub fn create(root: &Path) -> Result<Self> {
        sweep(root);
        let mut id = [0u8; 8];
        let _ = getrandom::fill(&mut id);
        let dir = root.join(id.iter().map(|b| format!("{b:02x}")).collect::<String>());
        std::fs::create_dir_all(&dir)?;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true);
        #[cfg(windows)]
        std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0);
        let lock = options.open(dir.join(LOCK_FILE))?;
        Ok(Self { dir, _lock: lock })
    }

    pub fn mac_status_file(&self) -> PathBuf {
        self.dir.join("mac-status.json")
    }
}

/// Deletes session folders left behind by incognito processes that are no longer running. A
/// running session's lock file cannot be deleted, which keeps its folder intact.
pub fn sweep(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        match std::fs::remove_file(dir.join(LOCK_FILE)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // A session that is being created right now has no lock file yet.
                let age = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
                if age.is_none_or(|a| a < std::time::Duration::from_secs(60)) {
                    continue;
                }
            }
            Err(_) => continue,
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// After this process exits, deletes the session folder (WebView2's processes release it a moment
/// later, hence the retries) and flushes the DNS cache.
pub fn schedule_cleanup(dir: &Path) {
    let dir = ps_quote(&dir.to_string_lossy());
    let script = format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue\n\
         for ($i = 0; $i -lt 120 -and (Test-Path -LiteralPath {dir}); $i++) {{\n\
           Remove-Item -LiteralPath {dir} -Recurse -Force -ErrorAction SilentlyContinue\n\
           Start-Sleep -Milliseconds 500\n\
         }}\n\
         ipconfig /flushdns | Out-Null",
        pid = std::process::id(),
    );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let spawned = std::process::Command::new(powershell())
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encode_command(&script)])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not schedule incognito cleanup");
        }
    }
}

/// Rewrites a downloaded file's Mark of the Web so it keeps its security zone but no longer
/// records the page (`ReferrerUrl`) and address (`HostUrl`) the file came from.
pub fn scrub_download_origin(path: &Path) {
    let mut stream = path.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    let Ok(current) = std::fs::read_to_string(&stream) else { return };
    let zone = current
        .lines()
        .find_map(|l| l.trim().strip_prefix("ZoneId="))
        .and_then(|z| z.trim().parse::<u32>().ok())
        .unwrap_or(3);
    if let Err(e) = std::fs::write(&stream, format!("[ZoneTransfer]\r\nZoneId={zone}\r\n")) {
        tracing::debug!(error = %e, "could not rewrite download zone information");
    }
}

/// Quotes a value as a PowerShell single-quoted string literal.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Encodes a script for `powershell -EncodedCommand` (Base64 of UTF-16LE), which avoids every
/// command-line quoting issue.
fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Windows PowerShell from the system directory, never whatever `powershell` resolves to on PATH.
#[cfg(windows)]
fn powershell() -> PathBuf {
    let mut buf = [0u16; 260];
    let len = unsafe { windows::Win32::System::SystemInformation::GetSystemDirectoryW(Some(&mut buf)) } as usize;
    let system = if len > 0 && len < buf.len() {
        PathBuf::from(String::from_utf16_lossy(&buf[..len]))
    } else {
        PathBuf::from(r"C:\Windows\System32")
    };
    system.join(r"WindowsPowerShell\v1.0\powershell.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_keeps_sessions_in_use() {
        let root = tempfile::tempdir().unwrap();
        let live = Session::create(root.path()).unwrap();
        let stale = root.path().join("stale");
        std::fs::create_dir_all(stale.join("EBWebView")).unwrap();
        std::fs::write(stale.join(LOCK_FILE), b"").unwrap();
        std::fs::write(stale.join("EBWebView").join("Local State"), b"{}").unwrap();

        sweep(root.path());
        assert!(!stale.exists(), "folders of exited sessions are removed");
        assert!(live.dir.join(LOCK_FILE).exists(), "a running session is left alone");

        let dir = live.dir.clone();
        drop(live);
        sweep(root.path());
        assert!(!dir.exists());
    }

    #[cfg(windows)]
    #[test]
    fn download_origin_is_removed_but_zone_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("setup.exe");
        std::fs::write(&file, b"MZ").unwrap();
        let mut stream = file.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        std::fs::write(&stream, "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://example.com/page\r\nHostUrl=https://cdn.example.com/setup.exe\r\n").unwrap();

        scrub_download_origin(&file);
        let after = std::fs::read_to_string(&stream).unwrap();
        assert_eq!(after, "[ZoneTransfer]\r\nZoneId=3\r\n");
        assert_eq!(std::fs::read(&file).unwrap(), b"MZ");
    }

    #[test]
    fn powershell_helpers() {
        assert_eq!(ps_quote(r"C:\Users\o'neil\x"), r"'C:\Users\o''neil\x'");
        // "a" in UTF-16LE is 61 00.
        assert_eq!(encode_command("a"), "YQA=");
    }
}
