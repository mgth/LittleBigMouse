//! Touchscreen policy and mouse-resumption adapter for the Windows pump thread.
//! State is stored in Cells; no borrow may span a Win32 call that can re-enter
//! the mouse hook. Pending restoration is cleared only after a successful warp.
use crate::geometry::Point;
use crate::hook::hot_path::{begin_touch, count_event, restore_touch_position};
use crate::hook::touch::{is_pen, is_touch, NativeTouchResume, TouchResume};
use crate::hook::touch_policy::{Modifier, TouchGesture};
use crate::platform::cursor::Win32Cursor;
use crate::shared::SHARED;
use std::cell::Cell;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, LLMHF_INJECTED, MSLLHOOKSTRUCT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};
fn modifier_held(modifier: Modifier) -> bool {
    let held = |key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY|
        unsafe { GetAsyncKeyState(key.0 as i32) } < 0;
    match modifier {
        Modifier::None => false,
        Modifier::Ctrl => held(VK_CONTROL),
        Modifier::Alt => held(VK_MENU),
        Modifier::Shift => held(VK_SHIFT),
        Modifier::Win => held(VK_LWIN) || held(VK_RWIN),
    }
}

thread_local! {
    static TOUCH: Cell<TouchResume> = const { Cell::new(TouchResume::new()) };
    static NATIVE: Cell<NativeTouchResume> = const { Cell::new(NativeTouchResume::new()) };
    static GENERATION: Cell<u32> = const { Cell::new(0) };
    static MOUSE_POSITION: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
    static GESTURE: Cell<TouchGesture> = const { Cell::new(TouchGesture::new()) };
}

fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }
        .ok()
        .map(|_| (point.x, point.y))
}

pub fn reset() {
    super::mouse::reset_movement();
    GESTURE.with(|v| v.set(TouchGesture::new()));
    super::focus_restore::reset();
    TOUCH.with(|state| state.set(TouchResume::new()));
    NATIVE.with(|state| state.set(NativeTouchResume::new()));
    MOUSE_POSITION.with(|position| position.set(cursor_position()));
}

fn sync_generation(shared: &crate::shared::Shared) {
    let generation = shared.touch_generation.load(Ordering::SeqCst);
    GENERATION.with(|old| {
        if old.replace(generation) != generation {
            reset();
        }
    });
}

/// Called on the same pump thread as WH_MOUSE_LL, only after a touchscreen HID
/// report with the cursor suppressed by touch. Never use GetCursorPos as the
/// anchor here: native touch can update it without any mouse-hook notification.
pub fn on_native_touch() {
    let Some(shared) = SHARED.get() else {
        return;
    };
    observe_native(shared);
}

fn observe_native(shared: &crate::shared::Shared) {
    sync_generation(shared);
    if !shared.touch_active() || !shared.hooked.load(Ordering::SeqCst) {
        NATIVE.with(|v| v.set(NativeTouchResume::new()));
        return;
    }
    begin_touch(&shared.engine, &mut Win32Cursor);
    super::mouse::reset_movement();
    let allowed = shared
        .touch_policy
        .try_lock()
        .ok()
        .is_some_and(|policy| !modifier_held(policy.modifier));
    NATIVE.with(|cell| {
        let mut state = cell.get();
        state.observe(MOUSE_POSITION.with(Cell::get), allowed);
        cell.set(state);
    });
}

pub fn on_native_contact(contact: bool) -> bool {
    let Some(shared) = SHARED.get() else {
        return false;
    };
    let allowed = shared
        .touch_policy
        .try_lock()
        .ok()
        .is_some_and(|policy| !modifier_held(policy.modifier));
    NATIVE.with(|cell| {
        let mut state = cell.get();
        state.observe_contact(MOUSE_POSITION.with(Cell::get), allowed, contact);
        cell.set(state);
    });
    if !NATIVE.with(|v| v.get().pending()) {
        super::focus_restore::cancel_native();
        return false;
    }
    shared.restore_keyboard_focus.load(Ordering::SeqCst)
        && super::focus_restore::native_contact(shared, contact)
}

pub fn finish_native_focus() {
    let Some(shared) = SHARED.get() else {
        return;
    };
    sync_generation(shared);
    let point = cursor_position();
    let allowed = shared.touch_active()
        && NATIVE.with(|v| v.get().pending())
        && point.is_some_and(|(x, y)| {
            shared
                .touch_policy
                .try_lock()
                .ok()
                .is_some_and(|policy| policy.includes(x, y))
        });
    if allowed {
        let (x, y) = point.unwrap();
        super::focus_restore::finish_native(shared, POINT { x, y });
    } else {
        super::focus_restore::cancel_native();
    }
}

/// An AppBar swipe can change the work area after finger-up. Routing uses full
/// monitor bounds, not the work area. Avoid a teardown/reload that would erase
/// the pending return when those bounds are demonstrably unchanged.
pub fn preserve_on_work_area_change() -> bool {
    SHARED.get().is_some_and(preserve_work_area_return)
}

fn preserve_work_area_return(shared: &crate::shared::Shared) -> bool {
    sync_generation(shared);
    if !shared.touch_active()
        || !shared.hooked.load(Ordering::SeqCst)
        || !(TOUCH.with(|v| v.get().pending()) || NATIVE.with(|v| v.get().pending()))
    {
        return false;
    }
    let Some(monitors) = (shared.monitors_now)() else {
        return false;
    };
    let Ok(engine) = shared.engine.try_lock() else {
        return false;
    };
    let layout = &engine.layout;
    !layout.virtual_layout
        && !monitors.is_empty()
        && layout.main_zones.len() == monitors.len()
        && layout.main_zones.iter().all(|id| {
            let bounds = layout.arena[*id].pixels_bounds();
            monitors.contains(&bounds)
        })
}

/// None continues monitor routing; Some passes through or consumes the event.
pub fn process(message: u32, ms: &MSLLHOOKSTRUCT) -> Option<bool> {
    process_with_shared(SHARED.get()?, message, ms)
}

fn process_with_shared(
    shared: &crate::shared::Shared,
    message: u32,
    ms: &MSLLHOOKSTRUCT,
) -> Option<bool> {
    sync_generation(shared);
    if !shared.touch_active() {
        clear_pending();
        return None;
    }
    if is_touch(ms.dwExtraInfo) {
        on_promoted_touch(shared, message, ms);
        return Some(false);
    }
    if is_pen(ms.dwExtraInfo) {
        if shared
            .touch_policy
            .try_lock()
            .ok()
            .is_some_and(|policy| policy.include_stylus)
        {
            on_promoted_touch(shared, message, ms);
            return Some(false);
        }
        NATIVE.with(|v| v.set(NativeTouchResume::new()));
        GESTURE.with(|v| v.set(TouchGesture::new()));
        super::focus_restore::reset();
        TOUCH.with(|cell| cell.set(TouchResume::new()));
        MOUSE_POSITION.with(|position| position.set(Some((ms.pt.x, ms.pt.y))));
        return Some(false);
    }
    if ms.flags & LLMHF_INJECTED == 0 {
        super::focus_restore::cancel_native();
    }
    // Programmatic cursor motion must not consume the pending restore.
    if ms.flags & LLMHF_INJECTED == 0 && shared.restore_keyboard_focus.load(Ordering::SeqCst) {
        super::focus_restore::on_physical_input(
            shared,
            message == WM_MOUSEMOVE,
            GESTURE.with(|v| v.get().contact),
        );
    }
    if ms.flags & LLMHF_INJECTED != 0
        && (TOUCH.with(|cell| cell.get().pending()) || NATIVE.with(|cell| cell.get().pending()))
    {
        return Some(false);
    }
    if message == WM_MOUSEMOVE {
        if TOUCH.with(|cell| cell.get().in_contact()) {
            count_event();
            return Some(false);
        }
        let mut resumed = TOUCH.with(Cell::get);
        // Native report coordinates may not have reached GetCursorPos
        // when WM_INPUT arrived. At physical resumption read the cursor
        // before applying the physical delta, to select the touched display.
        let native_target = NATIVE.with(|cell| {
            let state = cell.get();
            if !state.pending() {
                return None;
            }
            let touched = cursor_position().unwrap_or((ms.pt.x, ms.pt.y));
            let selected = shared
                .touch_policy
                .try_lock()
                .ok()
                .is_some_and(|policy| policy.includes(touched.0, touched.1));
            state.target(selected)
        });
        if let Some((x, y)) = resumed.resume().or(native_target) {
            // Keep TOUCH pending during the warp: injected re-entry must
            // not feed the absolute return through the crossing engine.
            if let Some(actual) =
                restore_touch_position(&shared.engine, &mut Win32Cursor, Point::new(x, y))
            {
                TOUCH.with(|cell| cell.set(resumed));
                NATIVE.with(|cell| cell.set(NativeTouchResume::new()));
                super::mouse::reset_movement();
                MOUSE_POSITION.with(|position| position.set(Some((actual.x(), actual.y()))));
            }
            count_event();
            return Some(true);
        }
        NATIVE.with(|cell| cell.set(NativeTouchResume::new()));
    } else {
        // A physical click/wheel takes ownership at the current position.
        // Do not relocate an in-progress physical drag on its next move.
        TOUCH.with(|cell| cell.set(TouchResume::new()));
        NATIVE.with(|cell| cell.set(NativeTouchResume::new()));
        MOUSE_POSITION.with(|position| position.set(Some((ms.pt.x, ms.pt.y))));
    }
    None
}

pub fn record_mouse_position(position: (i32, i32), warped: bool) {
    MOUSE_POSITION.with(|cell| {
        cell.set(if warped {
            cursor_position()
        } else {
            Some(position)
        })
    });
}

fn on_promoted_touch(shared: &crate::shared::Shared, message: u32, ms: &MSLLHOOKSTRUCT) {
    let (selected, overridden) = shared
        .touch_policy
        .try_lock()
        .ok()
        .map(|policy| {
            (
                policy.includes(ms.pt.x, ms.pt.y),
                modifier_held(policy.modifier),
            )
        })
        .unwrap_or((false, false));
    on_promoted_touch_with_policy(shared, message, ms, selected, overridden);
}

fn on_promoted_touch_with_policy(
    shared: &crate::shared::Shared,
    message: u32,
    ms: &MSLLHOOKSTRUCT,
    selected: bool,
    overridden: bool,
) {
    // The override changes restoration policy, not the absolute nature of touch.
    // Otherwise our old zone/clip can pin the first tap to the mouse monitor.
    begin_touch(&shared.engine, &mut Win32Cursor);
    super::mouse::reset_movement();
    super::focus_restore::cancel_native();
    NATIVE.with(|cell| {
        let mut state = cell.get();
        state.promoted();
        cell.set(state);
    });
    let down = message == WM_LBUTTONDOWN;
    let up = message == WM_LBUTTONUP;
    let accepted = GESTURE.with(|cell| {
        let mut gesture = cell.get();
        let accepted = gesture.accepts(selected, overridden, down, up);
        cell.set(gesture);
        accepted
    });
    if !accepted {
        super::focus_restore::reset();
        TOUCH.with(|v| v.set(TouchResume::new()));
        // A temporary override must not replace the physical mouse's anchor.
        // Unselected displays still use normal cursor ownership.
        if !GESTURE.with(|v| v.get().overridden) {
            MOUSE_POSITION.with(|v| v.set(Some((ms.pt.x, ms.pt.y))));
        }
        count_event();
        return;
    }
    // Hover preserves mouse position but must not repeatedly steal typing focus.
    // Contact movement cancels a pending focus attempt until the pen lifts.
    if !is_pen(ms.dwExtraInfo) || message != WM_MOUSEMOVE || GESTURE.with(|v| v.get().contact) {
        super::focus_restore::on_touch(shared, message, ms.pt);
    }
    // Use the last mouse position, including LBM's border warps.
    // The OS cursor may already reflect a preceding touch message.
    let cursor = MOUSE_POSITION.with(Cell::get);
    TOUCH.with(|cell| {
        let mut state = cell.get();
        state.touch(cursor, message == WM_LBUTTONDOWN, message == WM_LBUTTONUP);
        cell.set(state);
    });
    count_event(); // touch activity must not trip the hook watchdog
}

fn clear_pending() {
    TOUCH.with(|cell| cell.set(TouchResume::new()));
    NATIVE.with(|cell| cell.set(NativeTouchResume::new()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Shared;

    fn enabled() -> Shared {
        reset();
        GENERATION.with(|v| v.set(0));
        MOUSE_POSITION.with(|v| v.set(Some((1600, 873))));
        let shared = Shared::new();
        shared.touch_mouse_independent.store(true, Ordering::SeqCst);
        shared.want_hook.store(true, Ordering::SeqCst);
        shared.hooked.store(true, Ordering::SeqCst);
        shared
    }

    #[test]
    fn override_then_normal_tap_keeps_original_mouse_anchor() {
        for prior_tap in [false, true] {
            let shared = enabled();
            let touch = MSLLHOOKSTRUCT {
                dwExtraInfo: 0xff51_5780,
                pt: POINT { x: 2600, y: 2500 },
                ..Default::default()
            };
            if prior_tap {
                on_promoted_touch_with_policy(&shared, WM_LBUTTONDOWN, &touch, true, false);
                on_promoted_touch_with_policy(&shared, WM_LBUTTONUP, &touch, true, false);
            }
            on_promoted_touch_with_policy(&shared, WM_LBUTTONDOWN, &touch, true, true);
            // Releasing the key before finger-up must not re-enable this gesture.
            on_promoted_touch_with_policy(&shared, WM_LBUTTONUP, &touch, true, false);
            on_promoted_touch_with_policy(&shared, WM_MOUSEMOVE, &touch, true, false);
            assert!(!TOUCH.with(|v| v.get().pending()));
            assert_eq!(MOUSE_POSITION.with(Cell::get), Some((1600, 873)));
            on_promoted_touch_with_policy(&shared, WM_LBUTTONDOWN, &touch, true, false);
            assert!(TOUCH.with(|v| v.get().in_contact()));
            on_promoted_touch_with_policy(&shared, WM_LBUTTONUP, &touch, true, false);
            assert_eq!(TOUCH.with(|v| v.get().resume()), Some((1600, 873)));
        }
    }

    #[test]
    fn physical_mouse_and_unselected_displays_can_replace_override_anchor() {
        for physical in [false, true] {
            let shared = enabled();
            let touch = MSLLHOOKSTRUCT {
                dwExtraInfo: 0xff51_5780,
                pt: POINT { x: 2600, y: 2500 },
                ..Default::default()
            };
            on_promoted_touch_with_policy(&shared, WM_LBUTTONDOWN, &touch, true, true);
            on_promoted_touch_with_policy(&shared, WM_LBUTTONUP, &touch, true, false);
            if physical {
                record_mouse_position((2600, 2500), false);
            } else {
                on_promoted_touch_with_policy(&shared, WM_LBUTTONDOWN, &touch, false, false);
                on_promoted_touch_with_policy(&shared, WM_LBUTTONUP, &touch, false, false);
            }
            on_promoted_touch_with_policy(&shared, WM_LBUTTONDOWN, &touch, true, false);
            on_promoted_touch_with_policy(&shared, WM_LBUTTONUP, &touch, true, false);
            assert_eq!(TOUCH.with(|v| v.get().resume()), Some((2600, 2500)));
        }
    }

    #[test]
    fn appbar_work_area_change_preserves_return_only_for_unchanged_monitors() {
        use crate::geometry::Rect;
        let mut shared = enabled();
        shared.monitors_now = || Some(vec![Rect::new(0, 0, 1920, 1080)]);
        let layout = crate::zones::ZonesLayout::from_xml(concat!(
            "<ZonesLayout><MainZones><Zone Id=\"0\" Name=\"Main\">",
            "<PixelsBounds><Rect Left=\"0\" Top=\"0\" Width=\"1920\" Height=\"1080\"/></PixelsBounds>",
            "<PhysicalBounds><Rect Left=\"0\" Top=\"0\" Width=\"344\" Height=\"194\"/></PhysicalBounds>",
            "</Zone></MainZones></ZonesLayout>"
        )).unwrap();
        shared.engine.lock().unwrap().load(layout);
        assert!(!preserve_work_area_return(&shared));
        observe_native(&shared);
        assert!(preserve_work_area_return(&shared));
        assert_eq!(NATIVE.with(|v| v.get().target(true)), Some((1600, 873)));

        // A promoted swipe is protected too, including a drag still in contact.
        NATIVE.with(|v| v.set(NativeTouchResume::new()));
        TOUCH.with(|v| {
            let mut touch = TouchResume::new();
            touch.touch(Some((1600, 873)), true, false);
            v.set(touch);
        });
        assert!(preserve_work_area_return(&shared));
        shared.monitors_now = || Some(vec![Rect::new(1, 0, 1920, 1080)]);
        assert!(!preserve_work_area_return(&shared));
        shared.monitors_now = || Some(vec![Rect::new(0, 0, 1920, 1200)]);
        assert!(!preserve_work_area_return(&shared));
        shared.monitors_now = || Some(Vec::new());
        assert!(!preserve_work_area_return(&shared));
        shared.monitors_now = || None;
        assert!(!preserve_work_area_return(&shared));
        shared.monitors_now = || Some(vec![Rect::new(0, 0, 1920, 1080)]);
        shared.paused.store(true, Ordering::SeqCst);
        assert!(!preserve_work_area_return(&shared));
        shared.paused.store(false, Ordering::SeqCst);
        shared.hooked.store(false, Ordering::SeqCst);
        assert!(!preserve_work_area_return(&shared));
        shared.hooked.store(true, Ordering::SeqCst);
        shared.engine.lock().unwrap().layout.virtual_layout = true;
        assert!(!preserve_work_area_return(&shared));
        shared.engine.lock().unwrap().layout.virtual_layout = false;
        {
            let _busy = shared.engine.lock().unwrap();
            assert!(!preserve_work_area_return(&shared));
        }
        assert!(preserve_work_area_return(&shared));
        shared
            .touch_mouse_independent
            .store(false, Ordering::SeqCst);
        assert!(!preserve_work_area_return(&shared));
        shared.touch_mouse_independent.store(true, Ordering::SeqCst);
        shared.touch_generation.fetch_add(1, Ordering::SeqCst);
        assert!(!preserve_work_area_return(&shared));
    }

    #[test]
    fn native_report_injection_and_physical_click_follow_distinct_paths() {
        let shared = enabled();
        observe_native(&shared);
        let injected = MSLLHOOKSTRUCT {
            flags: LLMHF_INJECTED,
            ..Default::default()
        };
        assert_eq!(
            process_with_shared(&shared, WM_MOUSEMOVE, &injected),
            Some(false)
        );
        assert_eq!(NATIVE.with(|v| v.get().target(true)), Some((1600, 873)));
        assert_eq!(
            process_with_shared(&shared, WM_LBUTTONDOWN, &MSLLHOOKSTRUCT::default()),
            None
        );
        assert!(!NATIVE.with(|v| v.get().pending()));
    }

    #[test]
    fn native_return_retries_without_losing_anchor_when_engine_is_busy() {
        let shared = enabled();
        observe_native(&shared);
        let _busy = shared.engine.lock().unwrap();
        let movement = MSLLHOOKSTRUCT {
            pt: POINT { x: 2662, y: 2547 },
            ..Default::default()
        };
        for _ in 0..3 {
            assert_eq!(
                process_with_shared(&shared, WM_MOUSEMOVE, &movement),
                Some(true)
            );
            assert_eq!(NATIVE.with(|v| v.get().target(true)), Some((1600, 873)));
        }
    }

    #[test]
    fn reload_pause_and_pen_discard_native_restoration() {
        for reason in 0..3 {
            let shared = enabled();
            observe_native(&shared);
            let mut message = MSLLHOOKSTRUCT::default();
            match reason {
                0 => {
                    shared.touch_generation.fetch_add(1, Ordering::SeqCst);
                }
                1 => shared.paused.store(true, Ordering::SeqCst),
                _ => message.dwExtraInfo = 0xff51_5700,
            }
            process_with_shared(&shared, WM_MOUSEMOVE, &message);
            assert!(!NATIVE.with(|v| v.get().pending()));
        }
    }

    #[test]
    fn promoted_drag_owns_contact_tracking_when_raw_reports_also_arrive() {
        let shared = enabled();
        observe_native(&shared);
        let touch = MSLLHOOKSTRUCT {
            dwExtraInfo: 0xff51_5780,
            ..Default::default()
        };
        assert_eq!(
            process_with_shared(&shared, WM_LBUTTONDOWN, &touch),
            Some(false)
        );
        observe_native(&shared);
        assert!(!NATIVE.with(|v| v.get().pending()));
        assert_eq!(
            process_with_shared(&shared, WM_MOUSEMOVE, &MSLLHOOKSTRUCT::default()),
            Some(false)
        );
        assert!(TOUCH.with(|v| v.get().in_contact()));
        process_with_shared(&shared, WM_LBUTTONUP, &touch);
        assert_eq!(TOUCH.with(|v| v.get().resume()), Some((1600, 873)));
    }

    #[test]
    fn stylus_is_opt_in_and_preserves_the_mouse_anchor_through_hover_and_contact() {
        for include_stylus in [false, true] {
            let shared = enabled();
            let mut layout = crate::zones::ZonesLayout::default();
            layout.stylus_mouse_independent = include_stylus;
            *shared.touch_policy.lock().unwrap() =
                std::sync::Arc::new(crate::hook::touch_policy::TouchPolicy::from_layout(&layout));
            let pen = MSLLHOOKSTRUCT {
                dwExtraInfo: 0xff51_5700,
                pt: POINT { x: 2600, y: 2500 },
                ..Default::default()
            };
            for message in [WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE] {
                assert_eq!(process_with_shared(&shared, message, &pen), Some(false));
            }
            assert_eq!(
                TOUCH.with(|v| v.get().resume()),
                include_stylus.then_some((1600, 873))
            );
            shared
                .touch_mouse_independent
                .store(false, Ordering::SeqCst);
            process_with_shared(&shared, WM_MOUSEMOVE, &pen);
            assert!(!TOUCH.with(|v| v.get().pending()));
        }
    }
}
