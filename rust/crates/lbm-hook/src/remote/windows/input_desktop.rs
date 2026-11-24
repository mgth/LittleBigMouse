//! Following the input desktop, for `lbm-inject` (docs/multi-pc-service-windows.md).
//!
//! `SendInput` puts input on the desktop the calling thread is attached to, and only
//! while that desktop is the input desktop. When a UAC prompt or the lock screen comes
//! up, the input desktop becomes `winsta0\winlogon`, which only SYSTEM may open: the
//! injector, launched by the service with a SYSTEM token in the user's session, moves
//! its thread there, and back to `Default` when it goes away.
//!
//! No polling: the thread is attached when a session starts, and again when a
//! `SendInput` falls short (`super::super::stream::send_following`).
//!
//! `SetThreadDesktop` refuses a thread that owns a window or a hook: the injecting
//! thread owns neither (the pointer's hiding window lives on a thread of its own).

use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, SetThreadDesktop,
    DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS, HDESK, UOI_NAME,
};

/// `MAXIMUM_ALLOWED`: whatever the token may have on the desktop, which for SYSTEM is
/// everything `SendInput` needs.
const MAXIMUM_ALLOWED: DESKTOP_ACCESS_FLAGS = DESKTOP_ACCESS_FLAGS(0x0200_0000);

/// The desktop this thread was last attached to by [`InputDesktop::attach`].
#[derive(Default)]
pub struct InputDesktop {
    current: Option<(HDESK, String)>,
    /// Said once: a thread that cannot be moved stays where it is for good.
    refused: bool,
}

// SAFETY: a desktop handle is a plain kernel handle, usable from any thread; it is only
// ever used by the thread that owns the injector.
unsafe impl Send for InputDesktop {}

impl InputDesktop {
    /// Attaches the calling thread to the input desktop. `true` when that changed the
    /// desktop it is on.
    pub fn attach(&mut self) -> bool {
        // SAFETY: plain calls; the handle is closed below unless kept.
        let desktop =
            match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, MAXIMUM_ALLOWED) } {
                Ok(desktop) => desktop,
                Err(error) => {
                    eprintln!("[lbm-inject] no input desktop: {error}");
                    return false;
                }
            };
        let name = name(desktop).unwrap_or_default();
        if self
            .current
            .as_ref()
            .is_some_and(|(_, current)| *current == name)
        {
            // SAFETY: the handle just opened, not kept.
            let _ = unsafe { CloseDesktop(desktop) };
            return false;
        }
        // SAFETY: the handle is valid; the call fails on a thread with windows or hooks.
        if let Err(error) = unsafe { SetThreadDesktop(desktop) } {
            if !self.refused {
                eprintln!("[lbm-inject] cannot move to desktop {name}: {error}");
                self.refused = true;
            }
            // SAFETY: as above.
            let _ = unsafe { CloseDesktop(desktop) };
            return false;
        }
        eprintln!("[lbm-inject] on desktop {name}");
        if let Some((previous, _)) = self.current.replace((desktop, name)) {
            // SAFETY: the thread is no longer attached to it.
            let _ = unsafe { CloseDesktop(previous) };
        }
        true
    }
}

impl Drop for InputDesktop {
    fn drop(&mut self) {
        // Kept open while the thread is attached; closing it at the end is harmless,
        // the system keeps the thread's desktop alive until the thread ends.
        if let Some((desktop, _)) = self.current.take() {
            // SAFETY: a handle this value opened.
            let _ = unsafe { CloseDesktop(desktop) };
        }
    }
}

/// The desktop's name: `Default`, `Winlogon`…
fn name(desktop: HDESK) -> Option<String> {
    let mut buffer = [0u16; 64];
    let mut needed = 0u32;
    // SAFETY: the buffer and its size in bytes; the name is NUL-terminated.
    unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(buffer.as_mut_ptr().cast()),
            std::mem::size_of_val(&buffer) as u32,
            Some(&mut needed),
        )
    }
    .ok()?;
    // SAFETY: NUL-terminated within the buffer, as just written.
    unsafe { PCWSTR(buffer.as_ptr()).to_string() }.ok()
}
