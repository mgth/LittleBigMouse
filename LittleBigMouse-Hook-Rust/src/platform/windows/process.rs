//! Process/window helpers — port of the exe-path resolution in
//! `HookerWinEvents.cpp` (`GetExecutablePathFromProcessId`).

use windows::core::{HRESULT, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, HWND};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// Full Win32 executable path of whatever holds the foreground *right now*, or
/// `None` when there is nothing to ask about (a locked session, a switch in
/// progress) or the owner cannot be resolved.
///
/// The focus hook only ever reports a *change*. This is how the daemon asks the
/// same question at a moment of its own choosing — when it is about to hook, and
/// needs to know whether it would be hooking over an excluded app (#541).
pub fn foreground_path_now() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd == HWND::default() {
        return None;
    }
    exe_path_from_window(hwnd)
}

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

/// Full Win32 executable path of the parent process (C++ `GetParentProcess`),
/// used to tell "launched by the UI" (path contains "LittleBigMouse") from
/// standalone/autostart.
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
    // LIMITED is enough for QueryFullProcessImageNameW and, unlike
    // PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, is granted on elevated or
    // anticheat-protected processes — exactly the games exclusions target.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_current_process_with_limited_query_rights() {
        let path = exe_path_from_pid(unsafe { GetCurrentProcessId() }).expect("current exe path");
        assert!(!path.is_empty());
    }
}
