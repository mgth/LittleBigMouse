//! Cursor helpers — Linux side.
//!
//! The per-backend [`crate::engine::cursor::CursorEnv`] implementations live
//! with their event sources (`hook::linux::x11`, `hook::linux::portal`): on
//! Linux the way to read/warp/confine the cursor is inseparable from the way
//! events are captured.

/// Force-release cursor confinement for the explicit rescue shortcut. On Linux
/// confinement is owned by the active backend session (X11 grab / portal barriers)
/// and is torn down with it, so there is no process-global clip to clear.
pub fn force_release_clip() {}

/// Linux confinement belongs to the active input backend rather than a global
/// cursor API, so a hot layout reload has no Win32-style clip to restore here.
pub fn restore_managed_clip(_engine: &mut crate::engine::MouseEngine) {}
