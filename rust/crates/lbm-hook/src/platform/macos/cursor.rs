//! CoreGraphics implementation of [`CursorEnv`].
//!
//! macOS has no `ClipCursor`. The confinement the engine relies on (hold the
//! cursor inside the current zone while a border resists, or where no zone
//! continues) is emulated in software, like the X11 backend does: the clip is a
//! rectangle this process keeps, and the event tap pulls the real cursor back
//! inside it whenever the OS moved it out (`hook::macos::tap`).

use std::sync::Mutex;
use std::time::Instant;

use super::cg;
use super::display::virtual_screen;
use crate::engine::cursor::CursorEnv;
use crate::geometry::{Point, Rect};

/// The emulated clip; `None` = unclipped (the whole virtual screen). Process-wide
/// because the rescue shortcut and a hot layout reload reach it from the IPC
/// threads, while the tap reads it on the run-loop thread.
static CLIP: Mutex<Option<Rect<i32>>> = Mutex::new(None);

fn clip() -> Option<Rect<i32>> {
    *CLIP.lock().unwrap_or_else(|p| p.into_inner())
}

fn store_clip(r: Option<Rect<i32>>) {
    *CLIP.lock().unwrap_or_else(|p| p.into_inner()) = r;
}

/// Force-release the emulated clip, for the explicit rescue shortcut.
pub fn force_release_clip() {
    store_clip(None);
}

/// Restore the confinement saved by the engine when it is still the active LBM
/// rect (hot layout reload).
pub fn restore_managed_clip(engine: &mut crate::engine::MouseEngine) {
    let mut env = MacCursor::new();
    engine.restore_managed_clip(&mut env);
}

/// The active clip, for the tap's enforcement step.
pub(crate) fn active_clip() -> Option<Rect<i32>> {
    clip()
}

pub(crate) fn contains(r: &Rect<i32>, p: Point<i32>) -> bool {
    p.x() >= r.left() && p.x() < r.right() && p.y() >= r.top() && p.y() < r.bottom()
}

pub(crate) fn clamp(r: &Rect<i32>, p: Point<i32>) -> Point<i32> {
    Point::new(
        p.x().clamp(r.left(), r.right() - 1),
        p.y().clamp(r.top(), r.bottom() - 1),
    )
}

/// A global point to the engine's integer grid. `floor`, not `round`: the point
/// 1919.6 is still inside the column that starts at 1919.
pub(crate) fn to_grid(p: cg::CGPoint) -> Point<i32> {
    Point::new(p.x.floor() as i32, p.y.floor() as i32)
}

/// Move the real cursor. The warp itself generates no event; the next hardware
/// report starts from the new position.
pub(crate) fn warp(p: Point<i32>) {
    unsafe {
        let _ = cg::CGWarpMouseCursorPosition(cg::CGPoint {
            x: p.x() as f64,
            y: p.y() as f64,
        });
        // Re-associate explicitly: some macOS versions leave the hardware detached
        // from the cursor for a moment after a warp, which feels like a hitch.
        let _ = cg::CGAssociateMouseAndMouseCursorPosition(1);
    }
}

fn started() -> Instant {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

pub struct MacCursor {
    /// Where `set_mouse_location` sent the cursor during this event, so the tap
    /// can stamp the event it hands on with the same position.
    pub warped: Option<Point<i32>>,
}

impl MacCursor {
    pub fn new() -> Self {
        MacCursor { warped: None }
    }
}

impl Default for MacCursor {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorEnv for MacCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        let pos = to_grid(cg::cursor_location());
        // Under a real clip Win32 would never report an outside position.
        match clip() {
            Some(c) if !contains(&c, pos) => clamp(&c, pos),
            _ => pos,
        }
    }

    fn set_mouse_location(&mut self, location: Point<i32>) {
        warp(location);
        self.warped = Some(location);
    }

    fn get_clip(&self) -> Rect<i32> {
        // Win32 GetClipCursor returns the virtual screen when unclipped.
        clip().unwrap_or_else(virtual_screen)
    }

    fn set_clip(&mut self, r: Rect<i32>) {
        if r.is_empty() || r == virtual_screen() {
            store_clip(None);
            return;
        }
        store_clip(Some(r));
        // Win32 ClipCursor pulls the cursor into the new clip at once — the
        // engine's travel path relies on exactly that (set_clip, then read).
        let pos = to_grid(cg::cursor_location());
        if !contains(&r, pos) {
            warp(clamp(&r, pos));
        }
    }

    fn ctrl_down(&self) -> bool {
        let flags = unsafe { cg::CGEventSourceFlagsState(cg::kCGEventSourceStateHIDSystemState) };
        flags & cg::kCGEventFlagMaskControl != 0
    }

    fn buttons_down(&self) -> bool {
        // Left, right, middle and the two side buttons, like the Win32 side.
        (0..5).any(|button| unsafe {
            cg::CGEventSourceButtonState(cg::kCGEventSourceStateHIDSystemState, button)
        })
    }

    fn cursor_hidden(&self) -> bool {
        // Not detected yet: the public query (CGCursorIsVisible) is deprecated and
        // unreliable across processes. Fail-open, as the Win32 side does on error.
        // Games that detach the cursor are caught in the tap instead (a frozen
        // location under moving deltas is passed through untouched).
        false
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        // No other process can clip the cursor on macOS.
        false
    }

    fn tick_count(&self) -> u64 {
        started().elapsed().as_millis() as u64
    }
}
