//! Linux platform helpers — the counterpart of [`crate::platform::windows`].

pub mod cursor;
pub mod paths;
pub mod process;

use crate::priority::Priority;

/// Nothing to set up process-wide on Linux (the Win32 side sets DPI awareness).
pub fn init() {}

/// Best-effort niceness mapping of the Windows priority classes. Raising
/// priority (negative nice) needs privileges; failures are deliberately ignored
/// — priority is an optimization, never a functional requirement.
pub fn set_process_priority(priority: Priority) {
    let nice = match priority {
        Priority::Idle => 19,
        Priority::Below => 10,
        Priority::Normal => 0,
        Priority::Above => -5,
        Priority::High => -10,
        Priority::Realtime => -15,
    };
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, nice);
    }
}
