//! macOS platform helpers — the counterpart of [`crate::platform::windows`] and
//! [`crate::platform::linux`].
//!
//! Coordinates: everything the macOS daemon reads and writes is in CoreGraphics
//! *global display points* — the space `CGDisplayBounds`, `CGEventGetLocation` and
//! `CGWarpMouseCursorPosition` share, origin at the top-left of the main display,
//! y growing down. The macOS layout factory on the C# side builds the zones in that
//! same space, so the engine's "pixels" are points here (two physical pixels each
//! on a Retina panel), the way they are the compositor's logical units on Wayland.

pub mod cg;
pub mod cursor;
pub mod display;
pub mod process;

// The data directory follows the same convention as Linux on purpose: the C# side
// (`LbmPaths.DataDir`) resolves `$XDG_DATA_HOME/LittleBigMouse`, falling back to
// `~/.local/share/LittleBigMouse`, on every non-Windows OS — and the socket the UI
// connects to lives there too when `XDG_RUNTIME_DIR` is unset, as it is on macOS.
#[path = "../linux/paths.rs"]
pub mod paths;

use crate::priority::Priority;

/// Process-wide setup. Warping the cursor normally makes macOS ignore local
/// hardware input for a quarter of a second, which turns every border crossing
/// into a visible stall; a suppression interval of zero on the combined session
/// state removes it for the warps this process makes.
pub fn init() {
    unsafe {
        let source = cg::CGEventSourceCreate(cg::kCGEventSourceStateCombinedSessionState);
        if !source.is_null() {
            cg::CGEventSourceSetLocalEventsSuppressionInterval(source, 0.0);
            // Deliberately leaked: the setting lives as long as the source does,
            // and the source must therefore live as long as the process.
        }
    }
}

/// Same niceness mapping as Linux: both are BSD `setpriority`. Raising priority
/// needs privileges and fails quietly — priority is an optimization only.
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
