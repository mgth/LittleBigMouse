//! The engine's view of the cursor/input environment.
//!
//! Abstracting the Win32 calls (`GetCursorPos`/`SetCursorPos`/`ClipCursor`/
//! `GetAsyncKeyState`/freelook signals/`GetTickCount64`) behind a trait keeps the
//! traversal algorithm pure and deterministically testable — the golden-trace
//! defense against silent parity regressions. The real implementation is
//! [`crate::platform::cursor::Win32Cursor`]; tests use a fake.

use crate::geometry::{Point, Rect};

pub trait CursorEnv {
    /// C++ `GetMouseLocation` (`GetCursorPos`).
    fn get_mouse_location(&self) -> Point<i32>;
    /// C++ `SetMouseLocation` (`SetCursorPos`).
    fn set_mouse_location(&mut self, location: Point<i32>);
    /// C++ `GetClip` (`GetClipCursor`).
    fn get_clip(&self) -> Rect<i32>;
    /// C++ `SetClip` (`ClipCursor`).
    fn set_clip(&mut self, r: Rect<i32>);
    /// C++ `GetAsyncKeyState(VK_CONTROL) & 0x8000`.
    fn ctrl_down(&self) -> bool;
    /// Freelook signal 1: cursor hidden (`!(CURSORINFO.flags & CURSOR_SHOWING)`).
    fn cursor_hidden(&self) -> bool;
    /// Freelook signal 2: the cursor clip is a strict sub-rect of the virtual
    /// screen (a game confined the cursor).
    fn clip_is_subrect_of_virtual_screen(&self) -> bool;
    /// C++ `GetTickCount64`.
    fn tick_count(&self) -> u64;
}
