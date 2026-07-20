//! Process/window helpers for foreground exclusions and parent detection.

use windows::core::{HRESULT, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, HWND};
use windows::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// Full Win32 executable path of the process owning `hwnd`, or `None`.
pub fn exe_path_from_window(hwnd: HWND) -> Option<String> {
    let mut pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }
    exe_path_from_pid(pid)
}

/// Full executable path of the process that currently owns the foreground
/// window. Run queries this synchronously so exclusions do not depend on a
/// future WinEvent arriving.
pub fn foreground_process_path() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        None
    } else {
        exe_path_from_window(hwnd)
    }
}

/// Full Win32 executable path of the parent process, used to tell whether the
/// daemon was launched by the UI or as the standalone/autostart process.
pub fn parent_process_path() -> Option<String> {
    let ppid = parent_pid(unsafe { GetCurrentProcessId() })?;
    exe_path_from_pid(ppid)
}

fn parent_pid(pid: u32) -> Option<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut ppid = None;
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ProcessID == pid {
                    ppid = Some(entry.th32ParentProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    ppid
}

fn exe_path_from_pid(pid: u32) -> Option<String> {
    let handle: HANDLE =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let result = query_process_path(handle);
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

fn query_process_path(handle: HANDLE) -> Option<String> {
    const MAX_NT_PATH: usize = 32_768;
    let mut capacity = 260usize;

    loop {
        let mut buf = vec![0u16; capacity];
        let mut size = capacity as u32;
        let query = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
        };
        match query {
            Ok(()) => return Some(String::from_utf16_lossy(&buf[..size as usize])),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) => {}
            Err(_) => return None,
        }

        if capacity >= MAX_NT_PATH {
            return None;
        }
        capacity = (capacity * 2).min(MAX_NT_PATH);
    }
}

/// Windows path fragments use ordinal case-insensitive matching, matching the
/// filesystem/process-name semantics instead of Rust's case-sensitive `contains`.
pub fn path_contains(path: &str, fragment: &str) -> bool {
    let path: Vec<u16> = path.encode_utf16().collect();
    let fragment: Vec<u16> = fragment.encode_utf16().collect();
    if fragment.is_empty() || fragment.len() > path.len() {
        return false;
    }

    path.windows(fragment.len())
        .any(|candidate| unsafe { CompareStringOrdinal(candidate, &fragment, true) == CSTR_EQUAL })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_current_process_with_limited_query_rights() {
        let path = exe_path_from_pid(unsafe { GetCurrentProcessId() }).expect("current exe path");
        assert!(!path.is_empty());
    }

    #[test]
    fn windows_path_fragment_match_is_case_insensitive() {
        assert!(path_contains(
            r"C:\PROGRAM FILES\EPIC GAMES\Launcher.exe",
            r"\Epic Games\"
        ));
    }
}
