//! Cursor helpers — Linux side.
//!
//! The per-backend [`crate::engine::cursor::CursorEnv`] implementations live
//! with their event sources (`hook::linux::x11`, `hook::linux::portal`): on
//! Linux the way to read/warp/confine the cursor is inseparable from the way
//! events are captured.

/// Release any cursor confinement. On Linux confinement is owned by the active
/// backend session (X11 grab / portal barriers) and is torn down with it —
/// there is no process-global clip to clear, unlike Win32's `ClipCursor(NULL)`.
pub fn release_clip() {}
