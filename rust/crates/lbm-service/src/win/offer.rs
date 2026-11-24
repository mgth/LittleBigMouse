//! Where the service tells the agent of the session that it takes the input stream:
//! `HKLM\SOFTWARE\LittleBigMouse\InputService`, values `Port` (the port it listens on)
//! and `MachineId` (the fingerprint of its certificate). The agent announces them to
//! its masters in `Hello` (`SecureInput`).
//!
//! `HKLM\SOFTWARE` lets users read and only administrators and SYSTEM write: a process
//! of the user cannot announce a service of its own there. Written while the service
//! listens, removed when it stops listening or stops.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegDeleteKeyW, RegSetKeyValueW, HKEY_LOCAL_MACHINE, REG_DWORD, REG_SZ,
};

/// The key, under `HKEY_LOCAL_MACHINE`.
pub const KEY: &str = r"SOFTWARE\LittleBigMouse\InputService";
const KEY_W: PCWSTR = w!(r"SOFTWARE\LittleBigMouse\InputService");

/// Tells the agents the service listens on `port` as `machine_id`.
pub fn publish(port: u16, machine_id: &str) -> Result<(), String> {
    let port = u32::from(port);
    let id: Vec<u16> = machine_id.encode_utf16().chain([0]).collect();
    // SAFETY: a DWORD and a NUL-terminated string, with their sizes in bytes.
    unsafe {
        RegSetKeyValueW(
            HKEY_LOCAL_MACHINE,
            KEY_W,
            w!("Port"),
            REG_DWORD.0,
            Some(&port as *const u32 as *const _),
            4,
        )
        .ok()
        .map_err(|e| format!("{KEY}\\Port: {e}"))?;
        RegSetKeyValueW(
            HKEY_LOCAL_MACHINE,
            KEY_W,
            w!("MachineId"),
            REG_SZ.0,
            Some(id.as_ptr() as *const _),
            (id.len() * 2) as u32,
        )
        .ok()
        .map_err(|e| format!("{KEY}\\MachineId: {e}"))
    }
}

/// The service no longer listens: nothing is announced.
pub fn withdraw() {
    // SAFETY: a constant key name.
    let deleted = unsafe { RegDeleteKeyW(HKEY_LOCAL_MACHINE, KEY_W) };
    if deleted.is_err() && deleted != ERROR_FILE_NOT_FOUND {
        eprintln!("[lbm-service] {KEY} stays: {deleted:?}");
    }
}
