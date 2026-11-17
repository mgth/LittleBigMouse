//! macOS input hooking — a CoreGraphics event tap on a CFRunLoop.
//!
//! Same reconcile contract as the other backends: `run` owns the loop on the main
//! thread, installs the tap while `shared.want_hook` holds, removes it otherwise,
//! and returns on `want_quit`. The loop wakes at least every 100 ms to reconcile,
//! and twice a second compares the display list with the last one it saw — a
//! change is reported through `on_display_changed`, exactly as Windows reports
//! `WM_DISPLAYCHANGE`.
//!
//! Permissions: an active event tap needs the Accessibility permission (System
//! Settings → Privacy & Security → Accessibility). macOS grants it to the
//! *responsible* app — the UI bundle, or the terminal it was started from — not
//! to this binary on its own. Without it `CGEventTapCreate` returns null; the
//! loop reports `Stopped`, shows the system prompt once, and retries every few
//! seconds so a grant made while the app is open takes effect without a restart.

mod tap;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::engine::event::MouseEventArg;
use crate::geometry::{Point, Rect};
use crate::ipc::protocol;
use crate::platform;
use crate::platform::macos::cg;
use crate::platform::macos::cursor::MacCursor;
use crate::priority::Priority;
use crate::shared::Shared;

/// Nothing to record: the loop polls the shared flags, like the Linux backends.
pub fn register_main_thread(_shared: &Shared) {}

/// A tap the system disables is re-enabled from its own callback (see `tap`), so
/// there is no silent-removal failure for a watchdog to heal.
pub fn spawn_watchdog(_shared: &'static Shared) {}

/// Ask the loop to install the event tap.
pub fn request_hook(shared: &Shared) {
    shared.want_hook.store(true, Ordering::SeqCst);
}

/// Ask the loop to remove the event tap.
pub fn request_unhook(shared: &Shared) {
    shared.want_hook.store(false, Ordering::SeqCst);
}

/// Ask the loop to exit.
pub fn request_quit(shared: &Shared) {
    shared.want_quit.store(true, Ordering::SeqCst);
}

const DISPLAY_POLL: Duration = Duration::from_millis(500);
const RETRY_TAP: Duration = Duration::from_secs(3);

struct Tap {
    port: cg::CFMachPortRef,
    source: cg::CFRunLoopSourceRef,
}

/// Run the event loop on this thread until `request_quit`.
pub fn run(shared: &'static Shared) {
    eprintln!("[LittleBigMouse.Hook] macOS event-tap backend running");

    let mut installed: Option<Tap> = None;
    let mut last_failed_attempt: Option<Instant> = None;
    let mut prompted = false;

    let mut displays = displays_signature();
    let mut last_display_poll = Instant::now();
    tap::refresh_displays();

    while !shared.want_quit.load(Ordering::SeqCst) {
        // --- reconcile the tap with the desired state ---------------------------
        let want = shared.want_hook.load(Ordering::SeqCst);
        if want && installed.is_none() {
            let may_retry = last_failed_attempt.is_none_or(|t| t.elapsed() >= RETRY_TAP);
            if may_retry {
                match install() {
                    Some(t) => {
                        installed = Some(t);
                        last_failed_attempt = None;
                        tap::reset();
                        tap::refresh_displays();
                        shared.hooked.store(true, Ordering::SeqCst);
                        platform::set_process_priority(Priority::from_u8(
                            shared.priority.load(Ordering::SeqCst),
                        ));
                        shared.broadcast(protocol::RUNNING);
                    }
                    None => {
                        // Only announce the first failure of a streak, or the UI
                        // would see Stopped every three seconds.
                        if last_failed_attempt.is_none() {
                            eprintln!(
                                "[LittleBigMouse.Hook] cannot create the event tap: grant \
                                 Accessibility to the app that launched LittleBigMouse \
                                 (System Settings > Privacy & Security > Accessibility)"
                            );
                            shared.hooked.store(false, Ordering::SeqCst);
                            shared.broadcast(protocol::STOPPED);
                        }
                        if !prompted {
                            prompted = true;
                            cg::ensure_accessibility_prompt();
                        }
                        last_failed_attempt = Some(Instant::now());
                    }
                }
            }
        } else if !want {
            last_failed_attempt = None;
            if let Some(t) = installed.take() {
                uninstall(t);
                release_engine(shared);
                platform::set_process_priority(Priority::from_u8(
                    shared.priority_unhooked.load(Ordering::SeqCst),
                ));
                shared.broadcast(protocol::STOPPED);
            }
        }

        // --- display changes -------------------------------------------------------
        if last_display_poll.elapsed() >= DISPLAY_POLL {
            last_display_poll = Instant::now();
            let now = displays_signature();
            if now != displays {
                displays = now;
                tap::refresh_displays();
                crate::hook::on_display_changed(shared);
            }
        }

        // --- pump --------------------------------------------------------------------
        // Delivers the tap's events to `tap::callback`. With no source attached
        // CFRunLoopRunInMode returns at once (kCFRunLoopRunFinished): sleep instead
        // of spinning.
        let result = unsafe { cg::CFRunLoopRunInMode(cg::kCFRunLoopDefaultMode, 0.1, 0) };
        if result == cg::kCFRunLoopRunFinished {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    if let Some(t) = installed.take() {
        uninstall(t);
        release_engine(shared);
        shared.broadcast(protocol::STOPPED);
    }
}

fn install() -> Option<Tap> {
    let mask = cg::event_mask_bit(cg::kCGEventMouseMoved)
        | cg::event_mask_bit(cg::kCGEventLeftMouseDragged)
        | cg::event_mask_bit(cg::kCGEventRightMouseDragged)
        | cg::event_mask_bit(cg::kCGEventOtherMouseDragged);

    unsafe {
        let port = cg::CGEventTapCreate(
            cg::kCGSessionEventTap,
            cg::kCGHeadInsertEventTap,
            cg::kCGEventTapOptionDefault,
            mask,
            tap::callback,
            std::ptr::null_mut(),
        );
        if port.is_null() {
            return None;
        }
        let source = cg::CFMachPortCreateRunLoopSource(std::ptr::null(), port, 0);
        if source.is_null() {
            cg::CFMachPortInvalidate(port);
            cg::CFRelease(port as cg::CFTypeRef);
            return None;
        }
        tap::TAP.store(port, Ordering::SeqCst);
        cg::CFRunLoopAddSource(cg::CFRunLoopGetCurrent(), source, cg::kCFRunLoopDefaultMode);
        cg::CGEventTapEnable(port, true);
        Some(Tap { port, source })
    }
}

fn uninstall(t: Tap) {
    unsafe {
        cg::CGEventTapEnable(t.port, false);
        cg::CFRunLoopRemoveSource(
            cg::CFRunLoopGetCurrent(),
            t.source,
            cg::kCFRunLoopDefaultMode,
        );
        tap::TAP.store(std::ptr::null_mut(), Ordering::SeqCst);
        cg::CFMachPortInvalidate(t.port);
        cg::CFRelease(t.source as cg::CFTypeRef);
        cg::CFRelease(t.port as cg::CFTypeRef);
    }
    tap::reset();
}

/// Feed the engine a final `running = false` move so it releases any clip it
/// holds — what the Windows and X11 backends do on unhook. Safe to block-lock:
/// the tap is gone, nothing else routes.
fn release_engine(shared: &Shared) {
    let mut engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
    let mut env = MacCursor::new();
    let mut e = MouseEventArg::new(Point::default());
    e.running = false;
    engine.on_mouse_move(&mut env, &mut e);
    drop(engine);
    platform::cursor::force_release_clip();
    shared.hooked.store(false, Ordering::SeqCst);
}

fn displays_signature() -> Vec<Rect<i32>> {
    let mut rects = platform::display::monitors_now().unwrap_or_default();
    rects.sort_by_key(|r| (r.left(), r.top(), r.width(), r.height()));
    rects
}
