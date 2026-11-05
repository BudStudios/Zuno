//! MAC address randomization for incognito windows.
//!
//! A MAC address never leaves the local network segment, so websites cannot see it either way;
//! randomizing it changes what the local network (router, hotspot, network administrator) sees.
//! It needs administrator rights and briefly restarts the adapters, so it runs in a separate
//! elevated PowerShell process the user approves through UAC. That process:
//! 1. restores adapters still randomized by an earlier session that ended uncleanly,
//! 2. gives every active physical adapter a random locally administered unicast address,
//! 3. waits until no incognito window of this Zuno executable is running, then
//! 4. restores the original addresses.
//!
//! One such process serves all incognito windows (a named mutex keeps it unique). The original
//! value is kept next to the adapter's settings in the registry until it has been restored, so a
//! power loss or forced shutdown cannot lose it.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{encode_command, powershell, ps_quote, ARG};
use crate::error::{Error, Result};

const MUTEX: &str = r"Local\ZunoMacRandomizer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Another incognito window already randomized the address; it stays until all are closed.
    AlreadyActive,
    /// The user declined the administrator prompt.
    Declined,
    Started,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AdapterResult {
    pub name: String,
    /// Whether the adapter now reports the new address (some drivers ignore the override).
    pub ok: bool,
}

const SCRIPT: &str = r#"
$created = $false
$mutex = New-Object System.Threading.Mutex($true, '__MUTEX__', [ref]$created)
if (-not $created) { exit 0 }
$class = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}'
$mark = 'ZunoOriginalNetworkAddress'
function Get-AdapterKeys { Get-ChildItem -LiteralPath $class -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -match '^\d{4}$' } }
function Restore-Addresses {
  foreach ($k in Get-AdapterKeys) {
    $p = Get-ItemProperty -LiteralPath $k.PSPath -ErrorAction SilentlyContinue
    if ($null -eq $p -or $null -eq $p.$mark) { continue }
    if ($p.$mark -eq '-') { Remove-ItemProperty -LiteralPath $k.PSPath -Name NetworkAddress -ErrorAction SilentlyContinue }
    else { Set-ItemProperty -LiteralPath $k.PSPath -Name NetworkAddress -Value $p.$mark }
    Remove-ItemProperty -LiteralPath $k.PSPath -Name $mark -ErrorAction SilentlyContinue
    Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceGuid -eq $p.NetCfgInstanceId } | Restart-NetAdapter -Confirm:$false -ErrorAction SilentlyContinue
  }
}
try {
  Restore-Addresses
  $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
  $changed = @()
  foreach ($a in @(Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq 'Up' })) {
    $k = Get-AdapterKeys | Where-Object { (Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue).NetCfgInstanceId -eq $a.InterfaceGuid } | Select-Object -First 1
    if ($null -eq $k) { continue }
    $previous = (Get-ItemProperty -LiteralPath $k.PSPath -ErrorAction SilentlyContinue).NetworkAddress
    if ($null -eq $previous) { $previous = '-' }
    $b = New-Object byte[] 6
    $rng.GetBytes($b)
    $b[0] = ($b[0] -band 0xFC) -bor 0x02
    $mac = -join ($b | ForEach-Object { $_.ToString('X2') })
    Set-ItemProperty -LiteralPath $k.PSPath -Name $mark -Value $previous
    Set-ItemProperty -LiteralPath $k.PSPath -Name NetworkAddress -Value $mac
    Restart-NetAdapter -Name $a.Name -Confirm:$false -ErrorAction SilentlyContinue
    $changed += [pscustomobject]@{ name = $a.Name; mac = $mac }
  }
  Start-Sleep -Seconds 4
  $report = @($changed | ForEach-Object {
    $now = (Get-NetAdapter -Name $_.name -ErrorAction SilentlyContinue).MacAddress -replace '-', ''
    [pscustomobject]@{ name = $_.name; ok = ($now -eq $_.mac) }
  })
  Set-Content -LiteralPath __STATUS__ -Value (ConvertTo-Json -InputObject $report -Compress) -Encoding UTF8
  while (Get-CimInstance Win32_Process -Filter "Name = __EXE_NAME__" -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like '*__ARG__*' }) {
    Start-Sleep -Seconds 2
  }
} finally {
  Restore-Addresses
  $mutex.ReleaseMutex()
}
"#;

fn script(status: &Path, exe_name: &str) -> String {
    // The executable name goes inside a WQL string, which is itself inside a double-quoted
    // PowerShell string.
    let wql_name = format!("'{}'", exe_name.replace('\\', "\\\\").replace('\'', "\\'"));
    let ps_escaped = wql_name.replace('`', "``").replace('$', "`$").replace('"', "`\"");
    SCRIPT
        .replace("__MUTEX__", MUTEX)
        .replace("__STATUS__", &ps_quote(&status.to_string_lossy()))
        .replace("__EXE_NAME__", &ps_escaped)
        .replace("__ARG__", ARG)
}

/// Whether a randomizer process is already running. Its mutex belongs to an elevated process, so
/// opening it from here may be refused, which still means it exists.
fn helper_running() -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND};
    use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &HSTRING::from(MUTEX)) } {
        Ok(handle) => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            true
        }
        Err(e) => e.code() != ERROR_FILE_NOT_FOUND.to_hresult(),
    }
}

/// Asks for administrator approval and starts the randomizer. Blocks until the UAC prompt has
/// been answered, so call it from a worker thread. `status` receives the per-adapter results.
pub fn start(status: &Path, owner: Option<isize>) -> Result<Outcome> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    if helper_running() {
        return Ok(Outcome::AlreadyActive);
    }
    let exe = std::env::current_exe()?;
    let exe_name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or(Error::NotFound)?;
    let _ = std::fs::remove_file(status);
    let params = format!(
        "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand {}",
        encode_command(&script(status, &exe_name))
    );
    let result = unsafe {
        // ShellExecute may use COM for the elevation prompt.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        ShellExecuteW(
            owner.map(|h| HWND(h as *mut _)),
            &HSTRING::from("runas"),
            &HSTRING::from(powershell().as_os_str()),
            &HSTRING::from(params),
            PCWSTR::null(),
            SW_HIDE,
        )
    };
    // ShellExecute returns a value greater than 32 on success; SE_ERR_ACCESSDENIED (5) when the
    // UAC prompt was declined.
    match result.0 as isize {
        n if n > 32 => Ok(Outcome::Started),
        5 => Ok(Outcome::Declined),
        n => Err(Error::msg(format!("Windows could not start the MAC address helper (code {n})"))),
    }
}

/// Waits for the randomizer's report. `None` if it did not arrive within `timeout`.
pub fn wait_for_status(status: &Path, timeout: Duration) -> Option<Vec<AdapterResult>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(results) = read_status(status) {
            return Some(results);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    None
}

fn read_status(status: &Path) -> Option<Vec<AdapterResult>> {
    let text = std::fs::read_to_string(status).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}').trim()).ok()
}

/// A one-line summary of the randomizer's report for the interface.
pub fn describe(results: &[AdapterResult]) -> (&'static str, String) {
    let (ok, failed): (Vec<_>, Vec<_>) = results.iter().partition(|r| r.ok);
    let names = |v: &[&AdapterResult]| v.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ");
    match (ok.is_empty(), failed.is_empty()) {
        (true, true) => ("error", "MAC address not changed: no active network adapter was found".into()),
        (false, true) => ("info", format!("MAC address randomized ({}) until incognito windows are closed", names(&ok))),
        (true, false) => ("error", format!("MAC address not changed: the driver for {} does not allow it", names(&failed))),
        (false, false) => (
            "info",
            format!("MAC address randomized ({}); the driver for {} does not allow it", names(&ok), names(&failed)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_substitutions() {
        let s = script(Path::new(r"C:\Users\o'neil\status.json"), "Zuno.exe");
        assert!(s.contains(r"-LiteralPath 'C:\Users\o''neil\status.json'"));
        assert!(s.contains(r#"-Filter "Name = 'Zuno.exe'""#));
        assert!(s.contains("'*--incognito*'"));
        assert!(s.contains(r"'Local\ZunoMacRandomizer'"));
        assert!(!s.contains("__"));
    }

    #[test]
    fn status_parsing_and_summary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        std::fs::write(&path, "\u{feff}[{\"name\":\"Wi-Fi\",\"ok\":true},{\"name\":\"Ethernet\",\"ok\":false}]\r\n").unwrap();
        let results = read_status(&path).unwrap();
        assert_eq!(results.len(), 2);
        let (level, message) = describe(&results);
        assert_eq!(level, "info");
        assert!(message.contains("Wi-Fi") && message.contains("Ethernet"));
        assert_eq!(describe(&[]).0, "error");
    }
}
