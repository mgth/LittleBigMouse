//! The update check's Windows side: GitHub over HTTPS (reqwest on SChannel, through the
//! system proxy), the installer's Authenticode signature (`WinVerifyTrust`), and the
//! installer run elevated (`ShellExecuteExW`, verb `runas`) — what C# did with `runas`.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_CANCELLED, HANDLE, HWND, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Cryptography::{CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE};
use windows::Win32::Security::WinTrust::{
    WTHelperGetProvCertFromChain, WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData,
    WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
    WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_WHOLECHAIN, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};
use windows::Win32::UI::Shell::{ShellExecuteExW, ShellExecuteW, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::{Platform, Release, PUBLISHER, RELEASES};

/// No installer is anywhere near this; a bigger answer is not one.
const MAX_INSTALLER: u64 = 200 * 1024 * 1024;
/// `TRUST_E_NOSIGNATURE`: the file carries no signature at all.
const NO_SIGNATURE: i32 = 0x800B_0100_u32 as i32;

/// The platform of [`super::run`] under Windows.
pub struct Windows {
    client: reqwest::Client,
}

impl Windows {
    pub fn new() -> Result<Windows, String> {
        let client = reqwest::Client::builder()
            // GitHub's API refuses requests without one (PRIVACY.md names it).
            .user_agent("LittleBigMouse")
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Windows { client })
    }

    /// Downloads the installer of `release` into its own folder under %TEMP%, checking
    /// that it comes to the size GitHub announced. Neither checked nor run.
    pub async fn download(&self, release: &Release) -> Result<PathBuf, String> {
        if release.size > MAX_INSTALLER {
            return Err(format!(
                "The installer is too large ({} bytes)",
                release.size
            ));
        }
        let mut response = self
            .client
            .get(&release.installer)
            .timeout(Duration::from_secs(600))
            .send()
            .await
            .map_err(|e| format!("The download failed ({e})"))?;
        if !response.status().is_success() {
            return Err(format!(
                "The download failed (GitHub answered {})",
                response.status()
            ));
        }
        let mut bytes = Vec::with_capacity(release.size as usize);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("The download failed ({e})"))?
        {
            bytes.extend_from_slice(&chunk);
            if bytes.len() as u64 > release.size {
                break;
            }
        }
        if bytes.len() as u64 != release.size {
            return Err(format!(
                "The download is {} bytes, GitHub announced {}",
                bytes.len(),
                release.size
            ));
        }
        let file_name = release.file_name.clone();
        tokio::task::spawn_blocking(move || save(&file_name, &bytes))
            .await
            .map_err(|e| e.to_string())?
    }
}

impl Platform for Windows {
    async fn releases(&self) -> Result<Value, String> {
        let response = self
            .client
            .get(RELEASES)
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("GitHub could not be reached ({e})"))?;
        if !response.status().is_success() {
            return Err(format!("GitHub answered {}", response.status()));
        }
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        serde_json::from_slice(&body).map_err(|e| format!("GitHub's answer is not JSON ({e})"))
    }

    async fn install(&self, release: &Release) -> Result<(), String> {
        let path = self.download(release).await?;
        tokio::task::spawn_blocking(move || {
            if let Err(why) = verify(&path) {
                let _ = std::fs::remove_file(&path);
                return Err(why);
            }
            launch(&path)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    fn open(&self, url: &str) {
        let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        // SAFETY: two NUL-terminated strings that outlive the call.
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(url.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
}

/// Writes the installer to a directory of its own under %TEMP%, emptied first: an
/// earlier download, run or not, is not kept.
fn save(file_name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("LittleBigMouse-update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("The installer could not be saved ({e})"))?;
    let path = dir.join(file_name);
    std::fs::write(&path, bytes).map_err(|e| format!("The installer could not be saved ({e})"))?;
    Ok(path)
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// The installer's signature is valid, its chain trusted and unrevoked, and its signer
/// is [`PUBLISHER`]. The error is the reason, for the user.
pub fn verify(path: &Path) -> Result<(), String> {
    let path = wide(path);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(path.as_ptr()),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_WHOLECHAIN,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // No window: nothing is to be shown, and nobody is asked.
    let no_ui = HWND(INVALID_HANDLE_VALUE.0);
    // SAFETY: `data` points at `file`, which points at `path`; all outlive both calls.
    let status = unsafe { WinVerifyTrust(no_ui, &mut action, &mut data as *mut _ as *mut c_void) };
    let verdict = match status {
        0 => match signer(data.hWVTStateData) {
            Some(name) if name == PUBLISHER => Ok(()),
            Some(name) => Err(format!(
                "The installer is signed by {name}, not by {PUBLISHER}"
            )),
            None => Err("The installer's signer could not be read".to_owned()),
        },
        NO_SIGNATURE => Err(format!("The installer is not signed by {PUBLISHER}")),
        other => Err(format!(
            "The installer's signature is not valid (0x{:08X})",
            other as u32
        )),
    };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    // SAFETY: releases the state the first call kept.
    unsafe {
        WinVerifyTrust(no_ui, &mut action, &mut data as *mut _ as *mut c_void);
    }
    verdict
}

/// The simple name of the certificate that signed, from a verification's state.
fn signer(state: HANDLE) -> Option<String> {
    // SAFETY: pointers into the state WinVerifyTrust keeps until it is closed, each
    // checked before use.
    unsafe {
        let provider = WTHelperProvDataFromStateData(state);
        if provider.is_null() {
            return None;
        }
        let signer = WTHelperGetProvSignerFromChain(provider, 0, false, 0);
        if signer.is_null() {
            return None;
        }
        let cert = WTHelperGetProvCertFromChain(signer, 0);
        if cert.is_null() || (*cert).pCert.is_null() {
            return None;
        }
        let context = (*cert).pCert;
        let len = CertGetNameStringW(context, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, None);
        if len <= 1 {
            return None;
        }
        let mut name = vec![0u16; len as usize];
        CertGetNameStringW(
            context,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(&mut name),
        );
        let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        Some(String::from_utf16_lossy(&name[..end]))
    }
}

/// Runs the installer elevated: its wizard takes over, and closes this agent itself
/// (`CloseApplications=force`).
fn launch(path: &Path) -> Result<(), String> {
    let file = wide(path);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` points at strings that outlive the call.
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => {
            let _ = std::fs::remove_file(path);
            Err("The administrator prompt was declined".to_owned())
        }
        Err(e) => Err(format!("The installer could not be started ({e})")),
    }
}
