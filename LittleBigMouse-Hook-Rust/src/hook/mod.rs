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

/// Start the panic-shortcut listener: a global hotkey that frees a cursor trapped
/// where no click can reach the UI. `on_fire` runs on the listener's own thread.
///
/// Linux gets nothing yet. There is no global shortcut without a portal, and reading
/// one from evdev would mean opening the keyboard devices — the keylogging surface
/// this design exists to avoid. Tracked in mgth/LittleBigMouse#526.
pub fn spawn_rescue_key(shared: &'static Shared, on_fire: fn(&'static Shared)) {
    #[cfg(windows)]
    windows::rescue_key::spawn(shared, on_fire);
    #[cfg(not(windows))]
    {
        let _ = (shared, on_fire);
    }
}

/// Tell the listener the desired shortcut changed. Safe to call when it never
/// started — it is a post to a thread id that is still zero.
pub fn rescue_shortcut_changed(shared: &Shared) {
    #[cfg(windows)]
    windows::post_rescue_reconfigure(shared);
    #[cfg(not(windows))]
    {
        let _ = shared;
    }
}

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
    if shared.hooked.load(Ordering::SeqCst) {
        request_unhook(shared);
    }
    shared.broadcast(protocol::DISPLAY_CHANGED);
}

/// The work area changed (C++ `SettingChanged`).
pub(crate) fn on_setting_changed(shared: &Shared) {
    if shared.hooked.load(Ordering::SeqCst) {
        request_unhook(shared);
    }
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
    if shared.hooked.load(Ordering::SeqCst) {
        request_unhook(shared);
    }
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
    if shared.is_excluded(&path) {
        if !shared.paused.load(Ordering::SeqCst) && shared.hooked.load(Ordering::SeqCst) {
            request_unhook(shared);
            shared.paused.store(true, Ordering::SeqCst);
        }
    } else if shared.paused.load(Ordering::SeqCst) {
        if !shared.hooked.load(Ordering::SeqCst) {
            request_hook(shared);
        }
        shared.paused.store(false, Ordering::SeqCst);
    }
    shared.broadcast(&protocol::focus_changed(&path));
}

/// Re-decide the pause against whoever is in the foreground *right now*, and
/// report whether an excluded app is in front.
///
/// [`on_focus_changed`] only ever fires on a *change*, so an excluded app already
/// in front was never announced and the pause never engaged: the engine hooked
/// straight over a game and stayed there until the user alt-tabbed out and back
/// (#541). Starting LBM after the game was always enough to reproduce it; live
/// update (#525) made it ordinary rather than rare, by re-Running on every edit.
///
/// This is the question asked at `Run` time, which is the one moment the daemon
/// decides to hook, and therefore the one place the answer changes anything.
pub(crate) fn adopt_foreground(shared: &Shared) -> bool {
    let path = crate::platform::process::foreground_path_now();
    adopt_foreground_path(shared, path.as_deref())
}

/// See [`adopt_foreground`]; split from the OS query so the decision can be
/// tested without a desktop.
pub(crate) fn adopt_foreground_path(shared: &Shared, path: Option<&str>) -> bool {
    // Unknown foreground (no window, unresolvable owner, a platform that does not
    // answer) leaves the flag exactly as it was: the last thing we actually knew
    // beats a guess, and guessing "not excluded" would unpause over a game.
    let Some(path) = path.filter(|p| !p.is_empty()) else {
        return shared.paused.load(Ordering::SeqCst);
    };

    let excluded = shared.is_excluded(path);
    shared.paused.store(excluded, Ordering::SeqCst);
    // Same announcement a real focus change makes: the UI keeps its own idea of
    // what is in front, and it must not diverge just because nothing moved.
    shared.broadcast(&protocol::focus_changed(path));
    excluded
}

/// Run a callback body catching any panic, so it can never unwind across an
/// `extern "system"` FFI boundary (which would be UB).
pub(crate) fn guard<F: FnOnce()>(body: F) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_excluding(entries: &[&str]) -> Shared {
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = entries.iter().map(|e| e.to_string()).collect();
        shared
    }

    /// The #541 case: the game is already in front when `Run` arrives, so no
    /// focus change ever announced it.
    #[test]
    fn a_foreground_already_excluded_engages_the_pause() {
        let shared = shared_excluding(&[r"\Genshin Impact\"]);
        let excluded = adopt_foreground_path(
            &shared,
            Some(r"C:\Games\Genshin Impact\Genshin Impact game\GenshinImpact.exe"),
        );

        assert!(excluded, "an excluded foreground must stop Run hooking");
        assert!(shared.paused.load(Ordering::SeqCst));
    }

    #[test]
    fn a_foreground_that_is_not_excluded_lifts_a_stale_pause() {
        let shared = shared_excluding(&[r"\Genshin Impact\"]);
        shared.paused.store(true, Ordering::SeqCst);

        let explorer = Some(r"C:\Windows\explorer.exe");
        assert!(!adopt_foreground_path(&shared, explorer));
        assert!(!shared.paused.load(Ordering::SeqCst));
    }

    /// "Unknown" is not "nobody": a locked session, an unresolvable owner, or a
    /// platform that does not answer must not unpause over a game.
    #[test]
    fn an_unknown_foreground_leaves_the_pause_untouched() {
        for path in [None, Some("")] {
            let shared = shared_excluding(&[r"\Genshin Impact\"]);

            shared.paused.store(true, Ordering::SeqCst);
            assert!(adopt_foreground_path(&shared, path), "{path:?} unpaused");
            assert!(shared.paused.load(Ordering::SeqCst));

            shared.paused.store(false, Ordering::SeqCst);
            assert!(!adopt_foreground_path(&shared, path), "{path:?} paused");
            assert!(!shared.paused.load(Ordering::SeqCst));
        }
    }
}
