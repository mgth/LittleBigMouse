//! Input hooking, one implementation per OS behind a common facade.
//!
//! Every backend exposes the same control surface: `run(shared)` blocks on the
//! platform's event loop until a `Quit`, and `request_hook` / `request_unhook` /
//! `request_quit` ask that loop to reconcile. The event-action functions
//! (`on_display_changed`, `on_focus_changed`, ...) hold the platform-neutral
//! daemon semantics (unhook + broadcast) and are called by each backend's
//! callbacks.
//!
//! RULE: nothing potentially blocking on a routing thread — the pump loops and
//! the callbacks they invoke (including the `on_*` here, which broadcast to
//! sockets) run while the user's input is captured; any stall freezes the
//! pointer. Full rule and audit notes: hook/linux/evdev.rs module doc.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::ipc::protocol;
use crate::shared::Shared;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::{
    register_main_thread, request_hook, request_quit, request_unhook, run, spawn_watchdog,
};

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::{
    register_main_thread, request_hook, request_quit, request_unhook, run, spawn_watchdog,
};

/// Count of deduped mouse-move events the active backend has processed.
/// Lightweight instrumentation (one relaxed increment) to observe the hook
/// staying alive; also feeds the Windows watchdog.
pub static MOUSE_EVENTS: AtomicU64 = AtomicU64::new(0);

/// Count of events the engine handled (i.e. repositioned the cursor across a
/// border). Non-zero means the engine is actively managing crossings.
pub static CROSSINGS: AtomicU64 = AtomicU64::new(0);

// --- Event actions, shared by the backend callbacks (C++ LittleBigMouseDaemon) --

/// A display was added/removed/reconfigured: stop hooking and tell the UI to
/// recompute and reload the layout (C++ `LittleBigMouseDaemon::DisplayChanged`).
pub(crate) fn on_display_changed(shared: &Shared) {
    set_enabled(shared, false);
    shared.broadcast(protocol::DISPLAY_CHANGED);
}

/// The work area changed (C++ `SettingChanged`).
pub(crate) fn on_setting_changed(shared: &Shared) {
    set_enabled(shared, false);
    shared.broadcast(protocol::SETTING_CHANGED);
}

/// The system switched to/from the secure (UAC) desktop (C++ `DesktopChanged`).
pub(crate) fn on_desktop_changed(shared: &Shared) {
    shared.broadcast(protocol::DESKTOP_CHANGED);
}

/// The desktop stopped being displayed (screen off: sleep, session standby, lock/idle). Like a
/// display change, stop hooking — so the cursor is never left confined when the UI is absent —
/// and tell the UI, which then gates its rebuilds until the display comes back. Deduplicated: the
/// display-state notification re-pushes the current state every time the listener window (and its
/// registration) is recreated, which happens on every hook/unhook cycle.
pub(crate) fn on_suspend(shared: &Shared) {
    if shared.suspended.swap(true, Ordering::SeqCst) {
        return; // already suspended — ignore the repeated current-state push
    }
    reconcile_hook(shared);
    shared.broadcast(protocol::SUSPENDED);
}

/// The desktop is displayed again (wake / unlock / monitor on): tell the UI, which reconciles the
/// layout and re-hooks us once the configuration is stable. We do NOT re-hook ourselves — the UI
/// owns that (exactly like `on_display_changed`), so without a UI we stay safely unhooked.
pub(crate) fn on_resume(shared: &Shared) {
    if !shared.suspended.swap(false, Ordering::SeqCst) {
        return; // was not suspended
    }
    // Never come back from sleep with the cursor still confined: if a clip lingered across the
    // suspend (or the OS set one during the display transition), a leftover sub-virtual-screen clip
    // makes the engine read "freelook" and stop crossing. Release it before the UI re-Starts us.
    crate::platform::cursor::release_clip();
    shared.broadcast(protocol::RESUMED);
}

/// The foreground window changed (C++ `FocusChanged`): pause the hook while an
/// excluded app (e.g. a game) is focused, resume when it loses focus.
pub(crate) fn on_focus_changed(shared: &Shared, path: String) {
    shared
        .paused
        .store(shared.is_excluded(&path), Ordering::SeqCst);
    reconcile_hook(shared);
    shared.broadcast(&protocol::focus_changed(&path));
}

/// Change the user's requested run state, then reconcile the platform hook.
pub(crate) fn set_enabled(shared: &Shared, enabled: bool) {
    shared.enabled.store(enabled, Ordering::SeqCst);
    reconcile_hook(shared);
}

/// Reconcile asynchronous platform state from one authoritative predicate.
pub(crate) fn reconcile_hook(shared: &Shared) {
    let should_hook = shared.should_hook();
    let wants_hook = shared.want_hook.load(Ordering::SeqCst);
    let is_hooked = shared.hooked.load(Ordering::SeqCst);
    if should_hook {
        if !wants_hook || !is_hooked {
            request_hook(shared);
        }
    } else if wants_hook || is_hooked {
        request_unhook(shared);
    }
}

/// Run a callback body catching any panic, so it can never unwind across an
/// `extern "system"` FFI boundary (which would be UB).
pub(crate) fn guard<F: FnOnce()>(body: F) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusion_is_recorded_while_unhook_is_still_pending() {
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = vec!["game.exe".to_string()];
        shared.enabled.store(true, Ordering::SeqCst);
        shared.want_hook.store(true, Ordering::SeqCst);
        shared.hooked.store(false, Ordering::SeqCst);

        on_focus_changed(&shared, r"C:\Games\game.exe".to_string());

        assert!(shared.paused.load(Ordering::SeqCst));
        assert!(!shared.want_hook.load(Ordering::SeqCst));
    }

    #[test]
    fn leaving_exclusion_reasserts_hook_while_unhook_is_pending() {
        let shared = Shared::new();
        shared.enabled.store(true, Ordering::SeqCst);
        shared.paused.store(true, Ordering::SeqCst);
        shared.want_hook.store(false, Ordering::SeqCst);
        shared.hooked.store(true, Ordering::SeqCst);

        on_focus_changed(&shared, r"C:\Windows\explorer.exe".to_string());

        assert!(!shared.paused.load(Ordering::SeqCst));
        assert!(shared.want_hook.load(Ordering::SeqCst));
    }

    #[test]
    fn resume_waits_for_the_ui_to_reload_the_display_layout() {
        let shared = Shared::new();
        shared.enabled.store(true, Ordering::SeqCst);
        shared.want_hook.store(true, Ordering::SeqCst);
        shared.hooked.store(true, Ordering::SeqCst);

        on_suspend(&shared);
        shared.hooked.store(false, Ordering::SeqCst);
        on_resume(&shared);

        assert!(!shared.suspended.load(Ordering::SeqCst));
        assert!(shared.enabled.load(Ordering::SeqCst));
        assert!(
            !shared.want_hook.load(Ordering::SeqCst),
            "the UI must reload/revalidate the topology before re-hooking"
        );
    }
}
