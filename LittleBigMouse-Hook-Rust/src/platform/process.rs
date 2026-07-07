//! Process/window helpers — port of the exe-path resolution in
//! `HookerWinEvents.cpp` (`GetExecutablePathFromProcessId`).

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION,
    PROCESS_VM_READ,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

/// Full Win32 executable path of the process owning `hwnd`, or `None`.
pub fn exe_path_from_window(hwnd: HWND) -> Option<String> {
    let mut pid: u32 = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }

    let handle: HANDLE =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) }.ok()?;

    let mut buf = [0u16; MAX_PATH as usize];
    let mut size = buf.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
    };
    unsafe {
        let _ = CloseHandle(handle);
    }

    result
        .ok()
        .map(|_| String::from_utf16_lossy(&buf[..size as usize]))
}
