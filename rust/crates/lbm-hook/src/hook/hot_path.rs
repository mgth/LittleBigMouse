//! Platform-neutral core of the mouse-hook hot path.
//!
//! Every backend's per-report callback (`hook/windows/mouse.rs`,
//! `hook/linux/x11.rs`, `hook/linux/evdev/router.rs`, `hook/linux/portal.rs`)
//! is built from the same moving parts: **dedup** the report against the last
//! location (where positions repeat at all), take the engine under a
//! **non-blocking `try_lock`** (a contended lock — a `Load` swapping the layout
//! — passes the event straight through), and **dispatch** it to
//! `MouseEngine::on_mouse_move`. That shape runs up to 1000×/s per device with
//! the user's input captured, so it must never block and must not allocate or
//! log on the ordinary (no-crossing) path.
//!
//! Those parts are extracted here, free of any Win32 / evdev / X11 type, for two
//! reasons: the Windows callback that cannot compile off Windows is reduced to a
//! thin `unsafe` shim over code that can, and that same code is then exercised
//! directly by `benches/mouse_hook.rs` and by the tests below on any host — no
//! hook installed, no device opened, the real pointer never touched.
//!
//! The lock-and-dispatch decision ([`route_move`]) is byte-for-byte the same in
//! all four backends. What differs around it is deliberate, and each variant is
//! an explicit choice at the call site, not an accident to be flattened:
//!
//! | backend | dedup                       | after a crossing        | contended report          |
//! |---------|-----------------------------|-------------------------|---------------------------|
//! | Windows | [`MoveDedup`] on OS reports | dedup kept (C++ parity) | counted, passed to the OS |
//! | X11     | [`MoveDedup`] on polled `QueryPointer` | [`MoveDedup::reset`]    | counted, cursor already moved |
//! | evdev   | none — relative deltas, empty frames skipped upstream | nothing to resync | **not counted**; backend commits the clamped candidate itself |
//! | portal  | none — zero-delta frames skipped upstream | capture released at the warp target | counted, treated as not-handled |
//!
//! The crossing column: on Windows the `SetCursorPos` warp re-enters the hook
//! with its own event, so the stale `prev` (the swallowed position) resolves
//! itself — and when the warp lands exactly on the swallowed pixel (adjacent
//! same-DPI border at distance 1) dropping that synthetic event is correct, the
//! cursor is already there. X11 observes the warp by *polling*, not by an event,
//! so it resets the dedup to guarantee the next polled position reaches the
//! engine. The two absolute-position backends own their cursor outright and
//! never see a repeated position to begin with.

use std::sync::Mutex;

use crate::engine::cursor::CursorEnv;
use crate::engine::event::MouseEventArg;
use crate::engine::MouseEngine;
use crate::geometry::Point;
use crate::hook::{CROSSINGS, MOUSE_EVENTS};

/// The C++ `static previousLocation`: the last position the callback actually
/// forwarded. A `WH_MOUSE_LL` hook is re-entered for every OS report, including
/// the ones the OS synthesizes at an unchanged position (and the engine's own
/// `SetCursorPos`), so a report at the same integer pixel is dropped before it
/// reaches the engine — otherwise a stationary pointer would burn a lock and a
/// full traversal pass on every timer tick.
///
/// Not itself thread-safe: the backends that see repeatable positions (Windows,
/// X11) each own one from the single thread their callback runs on (the Windows
/// pump keeps it in a `thread_local`). Splitting it out of that `thread_local`
/// is what makes the dedup decision testable.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoveDedup {
    prev: Option<(i32, i32)>,
}

impl MoveDedup {
    pub const fn new() -> Self {
        MoveDedup { prev: None }
    }

    /// Record `loc` and report whether it differs from the last accepted report.
    /// `false` means "same pixel, drop it"; the caller does no further work.
    #[inline]
    pub fn accept(&mut self, loc: (i32, i32)) -> bool {
        if self.prev != Some(loc) {
            self.prev = Some(loc);
            true
        } else {
            false
        }
    }

    /// Forget the last position so the next report is always accepted. The engine
    /// warps the cursor on a crossing, and the synthetic move that warp produces
    /// must not be swallowed as a duplicate of the position we just left.
    #[inline]
    pub fn reset(&mut self) {
        self.prev = None;
    }
}

/// What [`route_move`] did with a deduped report — the information a backend needs
/// to decide whether to swallow the OS event and whether to resync its dedup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// The lock was contended (a `Load` held it): the event was passed through
    /// untouched, exactly as if there were no hook. Nothing was measured.
    Contended,
    /// The engine ran and left the cursor where it was — an interior move. The
    /// backend forwards the event.
    Passed,
    /// The engine repositioned the cursor across a border. What the backend does
    /// next is its own, documented in the module-level variants table: Windows
    /// swallows the OS event (`LRESULT(1)`) and keeps its dedup, X11 resets its
    /// dedup, portal releases the capture at the warp target, evdev emits the
    /// engine-written position.
    Crossed,
}

impl Routed {
    /// Whether the engine took over and moved the cursor (`MouseEventArg::handled`).
    #[inline]
    pub fn handled(self) -> bool {
        matches!(self, Routed::Crossed)
    }
}

/// Take the engine non-blocking and route one already-deduped report through it.
///
/// This is the shared body of every backend's `try_lock` arm, verbatim in
/// behaviour:
///
/// * **`WouldBlock`** — a `Load` is swapping the layout. Return [`Routed::Contended`]
///   immediately; the callback must never wait here or it risks the
///   `LowLevelHooksTimeout` and being silently torn down by the OS.
/// * **`Poisoned`** — `on_mouse_move` panicked on an earlier event (a debug-build
///   overflow at an extreme corner) and poisoned the mutex. Recover the guard
///   rather than treat the lock as dead forever: without this, every later event
///   fails the lock and crossing stays dead until a restart.
/// * **`Ok`** — dispatch, and count a crossing if the engine handled it.
///
/// Counting (`MOUSE_EVENTS`) belongs to the caller: it happens once per accepted
/// report, before the lock, and a contended pass must still be counted as an
/// event seen. Only [`CROSSINGS`] is bumped here, because only here is the
/// outcome known.
#[inline]
pub fn route_move<E: CursorEnv>(
    engine: &Mutex<MouseEngine>,
    env: &mut E,
    point: Point<i32>,
) -> Routed {
    let mut guard = match engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return Routed::Contended,
    };

    let mut e = MouseEventArg::new(point);
    guard.on_mouse_move(env, &mut e);
    if e.handled {
        CROSSINGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Routed::Crossed
    } else {
        Routed::Passed
    }
}

/// Count one accepted (deduped) report. Kept next to [`route_move`] so a backend
/// pairs the two and the "event seen" counter means the same thing everywhere:
/// every report that survived dedup, contended or not. (evdev is the documented
/// exception — see the module-level variants table: its contended arm commits a
/// position itself and has always left that frame uncounted.)
#[inline]
pub fn count_event() {
    MOUSE_EVENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Release mouse routing before absolute touch input, even when restoration is
/// overridden. Never wait for a layout reload from the hook thread.
pub fn begin_touch<E: CursorEnv>(engine: &Mutex<MouseEngine>, env: &mut E) {
    let mut guard = match engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    guard.begin_touch(env);
}

/// Do not block the hook while a layout reload owns the engine. The caller retains
/// its saved touch position on contention and retries on the next physical move.
pub fn restore_touch_position<E: CursorEnv>(
    engine: &Mutex<MouseEngine>,
    env: &mut E,
    target: Point<i32>,
) -> Option<Point<i32>> {
    let mut guard = match engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    Some(guard.resume_after_touch(env, target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Point, Rect};
    use crate::zones::ZonesLayout;

    // A fake cursor identical in spirit to the engine's own test double: every
    // method is a field read/write, so nothing here touches the OS.
    struct FakeCursor {
        pos: Point<i32>,
        clip: Rect<i32>,
    }

    impl FakeCursor {
        fn new() -> Self {
            FakeCursor {
                pos: Point::new(0, 0),
                clip: Rect::new(-10000, -10000, 30000, 20000),
            }
        }
    }

    impl CursorEnv for FakeCursor {
        fn get_mouse_location(&self) -> Point<i32> {
            self.pos
        }
        fn set_mouse_location(&mut self, location: Point<i32>) {
            // Match Windows: SetCursorPos is clamped to the active clip.
            self.pos = Point::new(
                location.x().clamp(self.clip.left(), self.clip.right() - 1),
                location.y().clamp(self.clip.top(), self.clip.bottom() - 1),
            );
        }
        fn get_clip(&self) -> Rect<i32> {
            self.clip
        }
        fn set_clip(&mut self, r: Rect<i32>) {
            self.clip = r;
        }
        fn ctrl_down(&self) -> bool {
            false
        }
        fn buttons_down(&self) -> bool {
            false
        }
        fn cursor_hidden(&self) -> bool {
            false
        }
        fn clip_is_subrect_of_virtual_screen(&self) -> bool {
            false
        }
        fn tick_count(&self) -> u64 {
            0
        }
    }

    // Left (pixels -3840..0) adjacent to Right (0..3840), Strait algorithm — the
    // same two-zone fixture the engine's own tests use.
    const FIXTURE: &str = concat!(
        r#"<ZonesLayout Priority="Normal" PriorityUnhooked="Below" Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="Left"><PixelsBounds><Rect Left="-3840" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="-567" Top="30.920075223319227" Width="527" Height="296"></Rect></PhysicalBounds>"#,
        r#"<RightLinks><ZoneLink From="0" To="393" SourceFromPixel="-225" SourceToPixel="2642" TargetFromPixel="0" TargetToPixel="2160" BorderResistance="0" TargetId="1"></ZoneLink>"#,
        r#"<ZoneLink From="393" To="1.7976931348623157E+308" SourceFromPixel="2642" SourceToPixel="2147483647" TargetFromPixel="-2147483648" TargetToPixel="2147483647" BorderResistance="0" TargetId="-1"></ZoneLink></RightLinks></Zone>"#,
        r#"<Zone Id="1" Name="Right"><PixelsBounds><Rect Left="0" Top="0" Width="3840" Height="2160"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="698" Height="393"></Rect></PhysicalBounds>"#,
        r#"<LeftLinks><ZoneLink From="0" To="393" SourceFromPixel="0" SourceToPixel="2160" TargetFromPixel="-225" TargetToPixel="2642" BorderResistance="0" TargetId="0"></ZoneLink></LeftLinks></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    fn engine_mutex() -> Mutex<MouseEngine> {
        let mut e = MouseEngine::new();
        e.load(ZonesLayout::from_xml(FIXTURE).unwrap());
        Mutex::new(e)
    }

    #[test]
    fn dedup_drops_a_repeat_and_accepts_a_move() {
        let mut d = MoveDedup::new();
        assert!(
            d.accept((100, 200)),
            "first report of a position is accepted"
        );
        assert!(!d.accept((100, 200)), "the same pixel is dropped");
        assert!(d.accept((100, 201)), "a one-pixel move is accepted");
        assert!(!d.accept((100, 201)));
        d.reset();
        assert!(
            d.accept((100, 201)),
            "after reset the same pixel is accepted again"
        );
    }

    #[test]
    fn interior_move_passes_and_leaves_the_cursor_alone() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        // Prime tracking (ExtFirst resolves the starting zone).
        assert_eq!(
            route_move(&engine, &mut env, Point::new(-100, 1000)),
            Routed::Passed
        );
        let before = env.pos;
        let routed = route_move(&engine, &mut env, Point::new(-200, 1000));
        assert_eq!(routed, Routed::Passed);
        assert!(!routed.handled());
        assert_eq!(env.pos, before, "an interior move must not warp the cursor");
    }

    #[test]
    fn overridden_touch_reaches_target_on_first_tap_without_a_return_warp() {
        for algorithm in [
            crate::zones::Algorithm::Strait,
            crate::zones::Algorithm::CornerCrossing,
        ] {
            let engine = engine_mutex();
            engine.lock().unwrap().layout.algorithm = algorithm;
            let mut env = FakeCursor::new();
            let desktop = env.clip;
            route_move(&engine, &mut env, Point::new(-500, 700));
            route_move(&engine, &mut env, Point::new(0, 1000));
            assert_ne!(env.clip, desktop);

            begin_touch(&engine, &mut env);
            let touched = Point::new(-1500, 1200);
            env.set_mouse_location(touched); // Windows applies the first tap.
            assert_eq!(env.pos, touched);
            assert_eq!(env.clip, desktop);
            // With override held, there is no saved-position return. The next
            // mouse report must adopt the touched monitor without remapping it.
            assert_eq!(route_move(&engine, &mut env, touched), Routed::Passed);
            assert_eq!(env.pos, touched);
            assert_eq!(
                route_move(&engine, &mut env, Point::new(-1499, 1200)),
                Routed::Passed
            );
        }
    }

    #[test]
    fn touch_begin_preserves_external_clip_and_never_waits_for_engine() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        route_move(&engine, &mut env, Point::new(-500, 700));
        route_move(&engine, &mut env, Point::new(0, 1000));
        let app_clip = Rect::new(100, 100, 200, 200);
        env.clip = app_clip;
        let before = env.pos;
        {
            let _busy = engine.lock().unwrap();
            begin_touch(&engine, &mut env);
        }
        begin_touch(&engine, &mut env);
        assert_eq!(env.clip, app_clip);
        assert_eq!(env.pos, before);
    }

    #[test]
    fn touch_return_releases_owned_clip_and_forgets_the_touched_monitor() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        let desktop = env.clip;
        let saved = Point::new(-500, 700);
        route_move(&engine, &mut env, saved);
        assert_eq!(
            route_move(&engine, &mut env, Point::new(0, 1000)),
            Routed::Crossed
        );
        assert_ne!(env.clip, desktop);
        // The previous direct warp clamps to the other monitor's edge.
        env.set_mouse_location(saved);
        assert_ne!(env.pos, saved);
        assert_eq!(
            restore_touch_position(&engine, &mut env, saved),
            Some(saved)
        );
        assert_eq!(env.clip, desktop);
        // A tiny subsequent move must not cross back from stale zone history.
        assert_eq!(
            route_move(&engine, &mut env, Point::new(-499, 700)),
            Routed::Passed
        );
        assert_eq!(env.clip, desktop);
    }

    #[test]
    fn touch_return_preserves_another_apps_clip() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        route_move(&engine, &mut env, Point::new(-500, 700));
        route_move(&engine, &mut env, Point::new(0, 1000));
        let app_clip = Rect::new(100, 100, 200, 200);
        env.clip = app_clip;
        assert_eq!(
            restore_touch_position(&engine, &mut env, Point::new(-500, 700)),
            Some(Point::new(100, 299))
        );
        assert_eq!(env.clip, app_clip);
    }

    #[test]
    fn touch_return_does_not_move_or_wait_when_engine_is_busy() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        let held = engine.lock().unwrap();
        assert_eq!(
            restore_touch_position(&engine, &mut env, Point::new(-500, 700)),
            None
        );
        assert_eq!(env.pos, Point::new(0, 0));
        drop(held);
        assert_eq!(
            restore_touch_position(&engine, &mut env, Point::new(-500, 700)),
            Some(Point::new(-500, 700))
        );
    }

    #[test]
    fn crossing_a_border_reports_crossed_and_warps() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        route_move(&engine, &mut env, Point::new(-100, 1000)); // init in Left
        let routed = route_move(&engine, &mut env, Point::new(0, 1000)); // cross into Right
        assert_eq!(routed, Routed::Crossed);
        assert!(routed.handled());
        assert_eq!(env.pos, Point::new(0, 922), "cursor remapped into Right");
    }

    #[test]
    fn a_contended_lock_passes_the_event_through() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        // Hold the lock as a `Load` would while swapping the layout.
        let held = engine.lock().unwrap();
        let routed = route_move(&engine, &mut env, Point::new(0, 1000));
        assert_eq!(
            routed,
            Routed::Contended,
            "a held engine lock must pass the event straight through"
        );
        assert!(!routed.handled());
        drop(held);
    }

    #[test]
    fn a_poisoned_lock_is_recovered_not_abandoned() {
        let engine = engine_mutex();
        let mut env = FakeCursor::new();
        route_move(&engine, &mut env, Point::new(-100, 1000)); // init

        // Poison the mutex the way a panic in on_mouse_move would.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = engine.lock().unwrap();
            panic!("simulated on_mouse_move panic");
        }));
        assert!(
            engine.is_poisoned(),
            "the mutex must be poisoned for this test"
        );

        // The very next report must still route: crossing stays alive.
        let routed = route_move(&engine, &mut env, Point::new(0, 1000));
        assert_eq!(
            routed,
            Routed::Crossed,
            "a poisoned lock must be recovered, not fail forever"
        );
    }
}
