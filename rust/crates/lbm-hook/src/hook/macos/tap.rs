//! The event-tap callback — the macOS counterpart of `hook/windows/mouse.rs`.
//!
//! An active (`kCGEventTapOptionDefault`) session tap sees every mouse-move and
//! drag event before applications do, and may rewrite it. Unlike `WH_MOUSE_LL`,
//! by the time the tap runs the window server has *already* moved the cursor: the
//! event's location is where it is now. So where the Windows hook swallows the
//! event and lets `SetCursorPos` win, this one warps and then stamps the event it
//! hands on with the warped position, so the app under the cursor sees the same
//! place the cursor is drawn.
//!
//! Two more differences the engine must not notice:
//!
//! * **Free edges.** macOS keeps the cursor on the displays: pushing into an edge
//!   that has no neighbouring display *in the system arrangement* leaves the
//!   location pinned to the edge. Win32 would have reported the position past the
//!   border, and that overshoot is what the engine reads to cross (and to drain
//!   border resistance). It is rebuilt here from the event's own motion delta:
//!   last location + delta, used whenever that lands outside every display.
//! * **No ClipCursor.** The engine's confinement is emulated (`platform::macos::
//!   cursor`); after routing, a cursor the system moved outside the clip is
//!   pulled back in and the event stamped accordingly.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::geometry::Point;
use crate::hook::hot_path::{count_event, route_move, MoveDedup, Routed};
use crate::platform::macos::cg;
use crate::platform::macos::cursor::{self, MacCursor};
use crate::shared::SHARED;

/// The installed tap, for re-enabling it from inside the callback when the
/// system turns it off (`kCGEventTapDisabledByTimeout`). Null when unhooked.
pub(super) static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

thread_local! {
    /// Last location this callback saw or set (global points). The base the
    /// free-edge overshoot is rebuilt from. Only ever touched on the run-loop
    /// thread the tap's source is attached to.
    static PREV: Cell<Option<cg::CGPoint>> = const { Cell::new(None) };
    static DEDUP: Cell<MoveDedup> = const { Cell::new(MoveDedup::new()) };
    /// Display bounds cache, refreshed by the run loop (`refresh_displays`), so
    /// the hot path never asks the window server for the display list.
    static DISPLAYS: RefCell<Vec<cg::CGRect>> = const { RefCell::new(Vec::new()) };
}

/// Forget per-session state (called on hook and unhook).
pub(super) fn reset() {
    PREV.with(|p| p.set(None));
    DEDUP.with(|d| d.set(MoveDedup::new()));
}

/// Re-read the display bounds the overshoot test uses. Run-loop thread only.
pub(super) fn refresh_displays() {
    let displays = cg::active_display_bounds();
    DISPLAYS.with(|d| *d.borrow_mut() = displays);
}

fn on_some_display(p: cg::CGPoint) -> bool {
    DISPLAYS.with(|d| {
        d.borrow().iter().any(|r| {
            p.x >= r.origin.x
                && p.x < r.origin.x + r.size.width
                && p.y >= r.origin.y
                && p.y < r.origin.y + r.size.height
        })
    })
}

/// # Safety
/// Registered with `CGEventTapCreate`; only the window server calls it, with a
/// valid event for the duration of the call.
pub(super) unsafe extern "C" fn callback(
    _proxy: cg::CGEventTapProxy,
    event_type: cg::CGEventType,
    event: cg::CGEventRef,
    _user_info: *mut c_void,
) -> cg::CGEventRef {
    if event_type == cg::kCGEventTapDisabledByTimeout
        || event_type == cg::kCGEventTapDisabledByUserInput
    {
        // The system stops a tap whose callback was too slow (or on secure input).
        // Turn it back on: the Windows watchdog's job, done in place.
        let tap = TAP.load(Ordering::SeqCst);
        if !tap.is_null() {
            eprintln!("[LittleBigMouse.Hook] event tap was disabled by the system, re-enabling");
            unsafe { cg::CGEventTapEnable(tap, true) };
        }
        return event;
    }

    if event.is_null() || !is_motion(event_type) {
        return event;
    }

    // Never unwind into the window server.
    let _ = catch_unwind(AssertUnwindSafe(|| process(event)));
    event
}

fn is_motion(event_type: cg::CGEventType) -> bool {
    matches!(
        event_type,
        cg::kCGEventMouseMoved
            | cg::kCGEventLeftMouseDragged
            | cg::kCGEventRightMouseDragged
            | cg::kCGEventOtherMouseDragged
    )
}

fn process(event: cg::CGEventRef) {
    let Some(shared) = SHARED.get() else {
        return;
    };

    let loc = unsafe { cg::CGEventGetLocation(event) };
    let dx = unsafe { cg::CGEventGetDoubleValueField(event, cg::kCGMouseEventDeltaX) };
    let dy = unsafe { cg::CGEventGetDoubleValueField(event, cg::kCGMouseEventDeltaY) };
    let prev = PREV.with(|p| p.get());

    // A cursor detached from the mouse (a game called
    // CGAssociateMouseAndMouseCursorPosition(false)) reports motion while its
    // location stays put. That is the game's pointer, not ours: hands off.
    if prev == Some(loc) && (dx != 0.0 || dy != 0.0) {
        let candidate = cg::CGPoint {
            x: loc.x + dx,
            y: loc.y + dy,
        };
        if on_some_display(candidate) {
            return;
        }
    }

    // The position the engine is told about: where the system put the cursor, or
    // — when the system pinned it to an edge with no display beyond — where it
    // would have gone (see the module doc).
    let mut overshoot = false;
    let mut raw = loc;
    if let Some(prev) = prev {
        let candidate = cg::CGPoint {
            x: prev.x + dx,
            y: prev.y + dy,
        };
        if (dx != 0.0 || dy != 0.0) && !on_some_display(candidate) {
            raw = candidate;
            overshoot = true;
        }
    }
    PREV.with(|p| p.set(Some(loc)));

    let point = cursor::to_grid(raw);

    // Pushing into a free edge repeats the same overshoot on every report, and
    // each one must reach the engine: it is what drains border resistance. Only
    // real positions are deduplicated.
    if !overshoot {
        let accepted = DEDUP.with(|d| {
            let mut dedup = d.get();
            let accepted = dedup.accept((point.x(), point.y()));
            d.set(dedup);
            accepted
        });
        if !accepted {
            return;
        }
    }
    count_event();

    let mut env = MacCursor::new();
    match route_move(&shared.engine, &mut env, point) {
        Routed::Crossed => {
            if let Some(target) = env.warped {
                stamp(event, target);
            }
            // The warp produces no event of its own: resync so the next report,
            // whatever it is, reaches the engine.
            DEDUP.with(|d| {
                let mut dedup = d.get();
                dedup.reset();
                d.set(dedup);
            });
        }
        Routed::Passed => {
            // Enforce the emulated clip: the system may have carried the cursor
            // across an edge it shares with another display, into a place the
            // layout does not lead to (no link, or resistance not yet drained).
            if let Some(clip) = cursor::active_clip() {
                let here = cursor::to_grid(loc);
                if !cursor::contains(&clip, here) {
                    let inside = cursor::clamp(&clip, here);
                    cursor::warp(inside);
                    stamp(event, inside);
                }
            }
        }
        Routed::Contended => {}
    }
}

/// Make the event say the cursor is at `p`, and remember it as the base for the
/// next report's delta.
fn stamp(event: cg::CGEventRef, p: Point<i32>) {
    let location = cg::CGPoint {
        x: p.x() as f64,
        y: p.y() as f64,
    };
    unsafe { cg::CGEventSetLocation(event, location) };
    PREV.with(|prev| prev.set(Some(location)));
}
