//! Thin Win32 helpers that don't belong to the hook or IPC layers.

pub mod cursor;
pub mod paths;
pub mod process;

use windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS,
    HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, REALTIME_PRIORITY_CLASS,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

use crate::priority::Priority;

/// Must run before any other Win32 call: otherwise every coordinate we read is
/// virtualized and wrong on multi-DPI setups.
pub fn init() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn set_process_priority(priority: Priority) {
    let class = match priority {
        Priority::Idle => IDLE_PRIORITY_CLASS,
        Priority::Below => BELOW_NORMAL_PRIORITY_CLASS,
        Priority::Normal => NORMAL_PRIORITY_CLASS,
        Priority::Above => ABOVE_NORMAL_PRIORITY_CLASS,
        Priority::High => HIGH_PRIORITY_CLASS,
        Priority::Realtime => REALTIME_PRIORITY_CLASS,
    };
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), class);
    }
}
