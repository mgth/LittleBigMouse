//! Capture the editor as well as its top-level window, and restore on a worker.
//! Input queues are attached only for the activation attempt and always detached.
//! Nothing here synthesizes keys or reads/logs typed text.

use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetLastInputInfo, SetFocus, LASTINPUTINFO, VK_CONTROL, VK_LBUTTON, VK_LWIN,
    VK_MBUTTON, VK_MENU, VK_RBUTTON, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    IsChild, IsHungAppWindow, IsIconic, IsWindow, IsWindowVisible, PeekMessageW,
    SetForegroundWindow, WindowFromPoint, GA_ROOT, GUITHREADINFO, MSG, PM_NOREMOVE, WM_LBUTTONDOWN,
    WM_LBUTTONUP,
};

use crate::hook::focus_restore::{before_touch, FocusSnapshot, RestoreRequest, WindowIdentity};
use crate::shared::Shared;

static SENDER: OnceLock<mpsc::SyncSender<RestoreRequest>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static COMPLETED: AtomicU64 = AtomicU64::new(0);
static RESOLVED: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static NATIVE_CAPTURE: Cell<Option<NativeCapture>> = const { Cell::new(None) };
    static ORIGIN: Cell<Option<FocusSnapshot>> = const { Cell::new(None) };
    static PREVIOUS: Cell<Option<FocusSnapshot>> = const { Cell::new(None) };
    static LAST_REQUEST: Cell<u64> = const { Cell::new(0) };
    static RELEASED: Cell<bool> = const { Cell::new(false) };
    static DEFERRED: Cell<Option<RestoreRequest>> = const { Cell::new(None) };
    static DEFERRED_FIRST: Cell<u64> = const { Cell::new(0) };
}

#[derive(Clone, Copy)]
struct NativeCapture {
    current: Option<FocusSnapshot>,
    previous: Option<FocusSnapshot>,
    touched: Option<WindowIdentity>,
    generation: u32,
    input_time: Option<u32>,
    contact: bool,
}

pub fn cancel_native() {
    NATIVE_CAPTURE.with(|capture| capture.set(None));
}

/// Capture before Windows has necessarily updated the cursor or foreground.
/// Returns true only on a down-to-up transition, to schedule position settling.
pub fn native_contact(shared: &Shared, contact: bool) -> bool {
    if !active(shared) {
        cancel_native();
        return false;
    }
    let mut capture = NATIVE_CAPTURE.with(Cell::get);
    if contact && capture.is_none_or(|c| !c.contact) {
        let previous = PREVIOUS.with(Cell::get);
        let current = focus_snapshot().map(|mut current| {
            // A nonactivating panel can clear editor focus while leaving the
            // original foreground window unchanged. Retain its last editor.
            if current.control.is_none() {
                current.control = previous
                    .filter(|p| p.window == current.window)
                    .and_then(|p| p.control);
            }
            current
        });
        capture = Some(NativeCapture {
            current,
            previous,
            touched: None,
            generation: shared.touch_generation.load(Ordering::SeqCst),
            input_time: last_input_time(),
            contact: true,
        });
        SEQUENCE.fetch_add(1, Ordering::SeqCst);
        trace("native touch: captured original focus");
    }
    let Some(mut capture) = capture else {
        return false;
    };
    if !contact && !capture.contact {
        return false;
    }
    let released = capture.contact && !contact;
    // Remember activation during contact, before a release handler can bring
    // another app forward. Never infer the touched app from post-release focus.
    if contact && capture.touched.is_none() {
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_ok() {
            let hit = identity(unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) });
            capture.touched = identity(unsafe { GetForegroundWindow() }).filter(|window| {
                Some(*window) != capture.current.map(|snapshot| snapshot.window)
                    && hit == Some(*window)
            });
        }
    }
    capture.contact = contact;
    capture.input_time = last_input_time();
    NATIVE_CAPTURE.with(|v| v.set(Some(capture)));
    released
}

/// Called after a short cursor-settling interval, on the same message-pump thread.
pub fn finish_native(shared: &Shared, point: POINT) {
    let Some(capture) = NATIVE_CAPTURE.with(|v| v.take()) else {
        return;
    };
    if capture.contact
        || !active(shared)
        || capture.generation != shared.touch_generation.load(Ordering::SeqCst)
        || capture.input_time != last_input_time()
    {
        trace(
            "native touch: cancelled before settled release (contact, state, generation, or input)",
        );
        return;
    }
    let hit = identity(unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) });
    let origin = before_touch(capture.current, capture.previous, hit);
    // A swipe may hide/reposition an AppBar before the settling timer runs.
    // Hit-testing then finds the desktop behind it, not the app that took focus.
    // Use the app observed during contact to recognize its moved panel. An
    // unrelated foreground window may have been opened by the touched button;
    // adopting it here would defeat the worker's explicit-window-switch guard.
    let touched = native_release_target(
        capture.touched,
        hit,
        identity(unsafe { GetForegroundWindow() }),
    );
    ORIGIN.with(|v| v.set(origin));
    LAST_REQUEST.with(|v| v.set(0));
    trace("native touch: scheduling settled release");
    on_touch_window(shared, WM_LBUTTONUP, || touched);
}

fn native_release_target(
    captured: Option<WindowIdentity>,
    hit: Option<WindowIdentity>,
    foreground: Option<WindowIdentity>,
) -> Option<WindowIdentity> {
    match captured {
        Some(touched) => {
            // A panel can move or expose another window in its own process.
            // Prefer the actual hit when it still belongs to the touched app.
            hit.filter(|window| window.process == touched.process)
                .or_else(|| foreground.filter(|window| window.process == touched.process))
                .or(Some(touched))
        }
        // Without evidence of an activated touch target, keep the hit-test.
        // A newly foregrounded app alone is not evidence that it was touched.
        None => hit,
    }
}

fn hwnd(identity: WindowIdentity) -> HWND {
    HWND(identity.handle as *mut std::ffi::c_void)
}

fn identity(window: HWND) -> Option<WindowIdentity> {
    if window == HWND::default() || !unsafe { IsWindow(window) }.as_bool() {
        return None;
    }
    let mut process = 0;
    let thread = unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    (thread != 0 && process != 0).then_some(WindowIdentity {
        handle: window.0 as isize,
        process,
        thread,
    })
}

fn thread_info(thread: u32) -> Option<GUITHREADINFO> {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(thread, &mut info) }
        .ok()
        .map(|_| info)
}

fn focus_snapshot() -> Option<FocusSnapshot> {
    let window = identity(unsafe { GetForegroundWindow() })?;
    let control = thread_info(window.thread).and_then(|info| identity(info.hwndFocus));
    Some(FocusSnapshot { window, control })
}

fn last_input_time() -> Option<u32> {
    let mut input = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe { GetLastInputInfo(&mut input) }
        .as_bool()
        .then_some(input.dwTime)
}

fn active(shared: &Shared) -> bool {
    shared.restore_keyboard_focus.load(Ordering::SeqCst)
        && shared.touch_active()
        && shared.hooked.load(Ordering::SeqCst)
}

fn eligible(shared: &Shared, request: &RestoreRequest) -> bool {
    request.should_restore(
        identity(hwnd(request.origin)),
        identity(unsafe { GetForegroundWindow() }),
        last_input_time(),
        active(shared),
        shared.touch_generation.load(Ordering::SeqCst),
        SEQUENCE.load(Ordering::SeqCst),
    )
}

/// The guard detaches even when activation or focus fails.
struct InputAttachment {
    from: u32,
    to: u32,
}
impl InputAttachment {
    fn new(to: u32) -> Option<Self> {
        let from = unsafe { GetCurrentThreadId() };
        if from == to {
            return None;
        }
        unsafe { AttachThreadInput(from, to, true) }
            .as_bool()
            .then_some(Self { from, to })
    }
}
impl Drop for InputAttachment {
    fn drop(&mut self) {
        unsafe {
            let _ = AttachThreadInput(self.from, self.to, false);
        }
    }
}

fn restore(shared: &Shared, request: &RestoreRequest) -> &'static str {
    if !eligible(shared, request) {
        return "skipped: state, input, or foreground changed";
    }
    let target = hwnd(request.origin);
    let policy = shared
        .touch_policy
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if identity(hwnd(request.touched)) != Some(request.touched)
        || !policy.restores_app(
            crate::platform::process::exe_path_from_window(hwnd(request.touched)).as_deref(),
        )
    {
        return "skipped: touched app focus rule or identity";
    }

    let foreground = unsafe { GetForegroundWindow() };
    if !unsafe { IsWindowVisible(target) }.as_bool()
        || unsafe { IsIconic(target) }.as_bool()
        || unsafe { IsHungAppWindow(target) }.as_bool()
        || unsafe { IsHungAppWindow(foreground) }.as_bool()
    {
        return "skipped: window hidden, minimized, or unresponsive";
    }

    // AttachThreadInput resets queue key state. Do not attach with held modifiers
    // or mouse buttons, and never attach while either app has a menu/capture/drag.
    if [
        VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN, VK_LBUTTON, VK_RBUTTON, VK_MBUTTON,
    ]
    .iter()
    .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
    {
        return "skipped: modifier or mouse button held";
    }
    let busy = |thread| {
        thread_info(thread).is_none_or(|info| {
            // GUI_INMOVESIZE | GUI_INMENUMODE | GUI_SYSTEMMENUMODE | GUI_POPUPMENUMODE
            info.flags.0 & 0x1e != 0 || info.hwndCapture != HWND::default()
        })
    };
    if busy(request.origin.thread) || busy(request.touched.thread) {
        return "skipped: app menu, capture, or move loop";
    }

    // Create the worker's message queue before linking it to the target queues.
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let foreground_identity = identity(foreground);
    let _foreground_queue =
        foreground_identity.and_then(|window| InputAttachment::new(window.thread));
    let _target_queue = if foreground_identity.map(|w| w.thread) != Some(request.origin.thread) {
        InputAttachment::new(request.origin.thread)
    } else {
        None
    };

    // Do not act on a request invalidated while making the queue connections.
    if !eligible(shared, request) {
        return "skipped: changed during activation";
    }
    if foreground != target {
        unsafe {
            let _ = SetForegroundWindow(target);
        }
    }
    if unsafe { GetForegroundWindow() } != target {
        return "refused: foreground activation";
    }

    if let Some(control) = request.control {
        let editor = hwnd(control);
        if identity(editor) != Some(control)
            || !(editor == target || unsafe { IsChild(target, editor) }.as_bool())
        {
            return "partial: original editor no longer exists";
        }
        if control.thread != request.origin.thread {
            return "partial: editor belongs to another input thread";
        }
        if !eligible(shared, request) {
            return "skipped: input after activation";
        }
        // SetFocus's return is the previous control, which can legitimately be null.
        // Verify the actual GUI-thread focus instead of treating null as failure.
        unsafe {
            let _ = SetFocus(editor);
        }
        if thread_info(request.origin.thread).map(|info| info.hwndFocus) == Some(editor) {
            return "restored: window and editor";
        }
        return "partial: window active, editor focus refused";
    }
    "restored: window (no native editor handle captured)"
}

/// Worker-only bounded diagnostic file. No titles, text, or keystrokes are logged.
fn trace(message: &str) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("LBM_TOUCH_TRACE").is_some()) {
        return;
    }
    let Some(path) = crate::platform::paths::lbm_data_file("TouchFocus.log") else {
        return;
    };
    let truncate = std::fs::metadata(&path)
        .map(|m| m.len() > 64 * 1024)
        .unwrap_or(false);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate)
        .open(path)
    {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let _ = writeln!(file, "{now} {message}");
    }
}

pub fn start(shared: &'static Shared) {
    let (sender, receiver) = mpsc::sync_channel::<RestoreRequest>(8);
    if SENDER.set(sender).is_err() {
        return;
    }
    std::thread::spawn(move || {
        let tracing = std::env::var_os("LBM_TOUCH_TRACE").is_some();
        if tracing {
            trace("touch-options worker started");
        }
        while let Ok(mut request) = receiver.recv() {
            while let Ok(newer) = receiver.try_recv() {
                request = newer;
            }
            if request.sequence != SEQUENCE.load(Ordering::SeqCst) {
                if tracing {
                    trace("skipped: superseded request before delay");
                }
                COMPLETED.fetch_max(request.sequence, Ordering::SeqCst);
                continue;
            }

            let policy = shared
                .touch_policy
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            // The delay is for touch release. Returning to the mouse is an explicit
            // request to restore focus immediately.
            let delay = if policy.on_mouse_move {
                0
            } else {
                policy.delay_ms
            };
            // Sleep in short intervals so a long configured delay cannot hold up
            // a newer gesture behind stale work.
            let mut remaining = delay;
            while remaining > 0 && request.sequence == SEQUENCE.load(Ordering::SeqCst) {
                let slice = remaining.min(10);
                std::thread::sleep(Duration::from_millis(slice));
                remaining -= slice;
            }

            let result = restore(shared, &request);
            // Only changing input needs a retry on the next physical move.
            // A rule refusal or completed restoration resolves the whole gesture,
            // including newer movement requests queued during the attempt.
            if !matches!(
                result,
                "skipped: state, input, or foreground changed"
                    | "skipped: changed during activation"
                    | "skipped: input after activation"
            ) {
                RESOLVED.fetch_max(request.sequence, Ordering::SeqCst);
            }

            if tracing {
                trace(&format!(
                "request={} result={} origin={} touched={} editor={} input_sample={:?} input_now={:?} foreground={:?} sequence={} active={}",
                request.sequence, result, request.origin.handle, request.touched.handle,
                request.control.map(|c| c.handle).unwrap_or(0), request.input_time, last_input_time(),
                identity(unsafe { GetForegroundWindow() }).map(|w| w.handle),
                SEQUENCE.load(Ordering::SeqCst), active(shared),
            ));
            }
            COMPLETED.fetch_max(request.sequence, Ordering::SeqCst);
        }
    });
}

pub fn reset() {
    cancel_native();
    SEQUENCE.fetch_add(1, Ordering::SeqCst);
    ORIGIN.with(|origin| origin.set(None));
    LAST_REQUEST.with(|request| request.set(0));
    RELEASED.with(|released| released.set(false));
    DEFERRED.with(|request| request.set(None));
    PREVIOUS.with(|previous| previous.set(focus_snapshot()));
}

pub fn on_physical_input(shared: &Shared, is_move: bool, in_contact: bool) {
    if in_contact {
        return;
    }
    if !is_move {
        reset();
        return;
    }
    let pending = DEFERRED.with(Cell::get);
    let Some(mut request) = pending else {
        reset();
        return;
    };
    if !active(shared)
        || request.generation != shared.touch_generation.load(Ordering::SeqCst)
        || DEFERRED_FIRST
            .with(|first| first.get() != 0 && RESOLVED.load(Ordering::SeqCst) >= first.get())
        || ![Some(request.touched), Some(request.origin)]
            .contains(&identity(unsafe { GetForegroundWindow() }))
    {
        reset();
        return;
    }
    let Some(input_time) = last_input_time() else {
        reset();
        return;
    };
    request.input_time = input_time;
    request.sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    DEFERRED.with(|pending| pending.set(Some(request)));
    LAST_REQUEST.with(|last| last.set(request.sequence));
    if SENDER
        .get()
        .is_none_or(|sender| sender.try_send(request).is_err())
    {
        reset();
    }
}

/// Tagged touch reports only. Capture before promotion when possible, retain the
/// editor throughout a burst, and sample the SAME idle API used by the worker.
pub fn on_touch(shared: &Shared, message: u32, point: POINT) {
    on_touch_window(shared, message, || {
        identity(unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) })
    });
}

fn on_touch_window(
    shared: &Shared,
    message: u32,
    resolve_touched: impl FnOnce() -> Option<WindowIdentity>,
) {
    if !active(shared) {
        reset();
        return;
    }
    // Resolve lazily: focus restoration is optional, and disabled touch events
    // should not hit-test windows or query their owning processes.
    let touched = resolve_touched();
    let last = LAST_REQUEST.with(Cell::get);
    if last != 0 && COMPLETED.load(Ordering::SeqCst) >= last {
        ORIGIN.with(|origin| origin.set(None));
    }
    LAST_REQUEST.with(|request| request.set(0));
    if message == WM_LBUTTONDOWN {
        RELEASED.with(|released| released.set(false));
        DEFERRED.with(|request| request.set(None));
    } else if message == WM_LBUTTONUP {
        RELEASED.with(|released| released.set(true));
    }
    ORIGIN.with(|origin| {
        if origin.get().is_none() {
            origin.set(before_touch(
                focus_snapshot(),
                PREVIOUS.with(Cell::get),
                touched,
            ));
        }
    });
    let sequence = SEQUENCE.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    if !RELEASED.with(Cell::get) {
        return;
    }
    let origin = ORIGIN.with(Cell::get);
    if let (Some(origin), Some(touched), Some(input_time), Some(sender)) =
        (origin, touched, last_input_time(), SENDER.get())
    {
        let request = RestoreRequest {
            origin: origin.window,
            control: origin.control,
            touched,
            input_time,
            generation: shared.touch_generation.load(Ordering::SeqCst),
            sequence,
        };

        let Ok(policy) = shared.touch_policy.try_lock() else {
            reset();
            return;
        };
        if policy.on_mouse_move {
            DEFERRED_FIRST.with(|first| first.set(sequence));
            DEFERRED.with(|pending| pending.set(Some(request)));
            return;
        }
        LAST_REQUEST.with(|last| last.set(sequence));
        if sender.try_send(request).is_err() {
            reset();
        }
    } else {
        reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::w;
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, TranslateMessage, PM_REMOVE,
        WINDOW_EX_STYLE, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
    };

    #[test]
    fn disabled_focus_does_not_resolve_the_touched_window() {
        on_touch_window(&Shared::new(), WM_LBUTTONUP, || {
            panic!("disabled focus must not query the touched window")
        });
    }

    #[test]
    fn native_swipe_uses_activated_panel_after_it_moves_away_from_release_point() {
        let window = |handle| WindowIdentity {
            handle,
            process: handle as u32,
            thread: 20,
        };
        let origin = Some(window(1));
        let desktop = Some(window(2));
        let panel = Some(window(3));
        assert_eq!(native_release_target(panel, desktop, panel), panel);
        assert_eq!(native_release_target(panel, None, panel), panel);
        // A nonactivating touch still uses hit-testing, never the original editor.
        assert_eq!(native_release_target(None, panel, origin), panel);
        assert_eq!(native_release_target(None, panel, None), panel);
        assert_eq!(native_release_target(None, desktop, panel), desktop);
    }

    #[test]
    fn widget_opening_media_player_does_not_adopt_player_as_touch_target() {
        let origin = WindowIdentity {
            handle: 1,
            process: 1,
            thread: 1,
        };
        let widget = WindowIdentity {
            handle: 2,
            process: 2,
            thread: 2,
        };
        let player = WindowIdentity {
            handle: 3,
            process: 3,
            thread: 3,
        };
        let desktop = WindowIdentity {
            handle: 4,
            process: 4,
            thread: 4,
        };
        for captured in [None, Some(widget)] {
            let touched = native_release_target(captured, Some(widget), Some(player)).unwrap();
            assert_eq!(touched, widget);
            let request = RestoreRequest {
                origin,
                control: None,
                touched,
                input_time: 10,
                generation: 1,
                sequence: 1,
            };
            // Applies both after the release delay and when restoration is
            // deferred until physical mouse movement.
            assert!(!request.should_restore(Some(origin), Some(player), Some(10), true, 1, 1));
            assert!(request.should_restore(Some(origin), Some(widget), Some(10), true, 1, 1));
        }
        // Hiding the widget or covering its release point must not adopt the
        // newly opened app either when the contact target was captured.
        for hit in [None, Some(desktop), Some(player)] {
            assert_eq!(
                native_release_target(Some(widget), hit, Some(player)),
                Some(widget)
            );
        }
        let panel = WindowIdentity {
            handle: 5,
            ..widget
        };
        assert_eq!(
            native_release_target(Some(widget), Some(desktop), Some(panel)),
            Some(panel)
        );
        // A different window in the widget process must also be respected when
        // the original widget still occupies the release point.
        assert_eq!(
            native_release_target(Some(widget), Some(widget), Some(panel)),
            Some(widget)
        );
    }

    struct TestWindows {
        original: HWND,
        other: HWND,
        previous: HWND,
    }
    impl Drop for TestWindows {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.other);
                let _ = DestroyWindow(self.original);
                let _ = SetForegroundWindow(self.previous);
            }
        }
    }

    #[test]
    #[ignore = "Interactive Windows test: briefly opens two test windows and changes focus"]
    fn restores_native_editor_from_another_process() {
        let previous = unsafe { GetForegroundWindow() };
        let create = |title| unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                title,
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                20,
                20,
                330,
                160,
                None,
                None,
                HINSTANCE::default(),
                None,
            )
            .unwrap()
        };
        let windows = TestWindows {
            original: create(w!("LBM focus regression - original editor")),
            other: create(w!("LBM focus regression - touch target")),
            previous,
        };
        let editor = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!("Temporary focus test"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                10,
                10,
                280,
                35,
                windows.original,
                None,
                HINSTANCE::default(),
                None,
            )
            .unwrap()
        };
        // A test launched by a background shell is not entitled to foreground
        // activation either. Connect the fixture's queue while setting it up.
        let fixture_queue = identity(unsafe { GetForegroundWindow() })
            .and_then(|window| InputAttachment::new(window.thread));
        unsafe {
            let _ = SetForegroundWindow(windows.original);
            let _ = SetFocus(editor);
        }
        let origin = identity(windows.original).unwrap();
        assert_eq!(thread_info(origin.thread).unwrap().hwndFocus, editor);
        unsafe {
            let _ = SetForegroundWindow(windows.other);
            let _ = SetFocus(windows.other);
        }
        assert_eq!(
            unsafe { GetForegroundWindow() },
            windows.other,
            "Windows must allow the fixture to activate its test window"
        );
        drop(fixture_queue);
        let data = format!(
            "{},{},{},{}",
            origin.handle,
            editor.0 as isize,
            windows.other.0 as isize,
            last_input_time().unwrap()
        );
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "hook::windows::focus_restore::tests::activation_worker_process",
                    "--ignored",
                    "--nocapture",
                ])
                .env("LBM_FOCUS_TEST_WINDOWS", data)
                .output()
                .unwrap();
            sender.send(output).unwrap();
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Ok(status) = receiver.try_recv() {
                break status;
            }
            assert!(std::time::Instant::now() < deadline, "focus worker stalled");
            let mut msg = MSG::default();
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        worker.join().unwrap();
        println!("{}", String::from_utf8_lossy(&status.stdout));
        assert!(
            status.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        );
        assert_eq!(unsafe { GetForegroundWindow() }, windows.original);
        assert_eq!(thread_info(origin.thread).unwrap().hwndFocus, editor);
    }

    #[test]
    #[ignore = "Child process used by the interactive focus test"]
    fn activation_worker_process() {
        let Ok(data) = std::env::var("LBM_FOCUS_TEST_WINDOWS") else {
            return;
        };
        let values: Vec<isize> = data
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(values.len(), 4);
        let window = |index| identity(HWND(values[index] as *mut std::ffi::c_void)).unwrap();
        let shared = Box::leak(Box::new(Shared::new()));
        shared.hooked.store(true, Ordering::SeqCst);
        shared.want_hook.store(true, Ordering::SeqCst);
        shared.touch_mouse_independent.store(true, Ordering::SeqCst);
        shared.restore_keyboard_focus.store(true, Ordering::SeqCst);
        let request = RestoreRequest {
            origin: window(0),
            control: Some(window(1)),
            touched: window(2),
            input_time: values[3] as u32,
            generation: 0,
            sequence: 0,
        };

        let mut layout = crate::zones::ZonesLayout::default();
        // App rules run in the real worker's restoration path and do not activate.
        layout.focus_keep_apps = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        *shared.touch_policy.lock().unwrap() =
            std::sync::Arc::new(crate::hook::touch_policy::TouchPolicy::from_layout(&layout));
        assert!(
            eligible(shared, &request),
            "fixture changed before restoration: active={}, input={:?}/{}, foreground={:?}/{:?}, generation={}, sequence={}",
            active(shared),
            last_input_time(),
            request.input_time,
            identity(unsafe { GetForegroundWindow() }),
            request.touched,
            shared.touch_generation.load(Ordering::SeqCst),
            SEQUENCE.load(Ordering::SeqCst)
        );
        assert_eq!(
            restore(shared, &request),
            "skipped: touched app focus rule or identity"
        );
        assert_eq!(
            identity(unsafe { GetForegroundWindow() }),
            Some(request.touched)
        );

        layout.focus_keep_apps.clear();
        layout.focus_restore_delay = 250;
        *shared.touch_policy.lock().unwrap() =
            std::sync::Arc::new(crate::hook::touch_policy::TouchPolicy::from_layout(&layout));
        start(shared);
        SENDER.get().unwrap().try_send(request).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            identity(unsafe { GetForegroundWindow() }),
            Some(request.touched),
            "configured delay must not restore early"
        );
        let wait_for_editor = || {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while thread_info(request.origin.thread).map(|info| info.hwndFocus)
                != Some(hwnd(request.control.unwrap()))
                || unsafe { GetForegroundWindow() } != hwnd(request.origin)
            {
                assert!(
                    std::time::Instant::now() < deadline,
                    "worker did not restore editor"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        wait_for_editor();

        // Put the touched window back in front, then exercise the actual
        // movement trigger. A physical click cancels it; a movement restores it.
        let _queue = InputAttachment::new(request.touched.thread);
        unsafe {
            let _ = SetForegroundWindow(hwnd(request.touched));
        }
        drop(_queue);
        assert_eq!(
            identity(unsafe { GetForegroundWindow() }),
            Some(request.touched)
        );
        reset();
        layout.focus_restore_on_mouse_move = true;
        *shared.touch_policy.lock().unwrap() =
            std::sync::Arc::new(crate::hook::touch_policy::TouchPolicy::from_layout(&layout));
        let mut deferred = request;
        deferred.sequence = SEQUENCE.load(Ordering::SeqCst);
        DEFERRED_FIRST.with(|first| first.set(deferred.sequence));
        DEFERRED.with(|pending| pending.set(Some(deferred)));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            identity(unsafe { GetForegroundWindow() }),
            Some(request.touched)
        );
        on_physical_input(shared, false, false);
        assert!(DEFERRED.with(Cell::get).is_none());
        deferred.sequence = SEQUENCE.load(Ordering::SeqCst);
        DEFERRED_FIRST.with(|first| first.set(deferred.sequence));
        DEFERRED.with(|pending| pending.set(Some(deferred)));
        on_physical_input(shared, true, false);
        wait_for_editor();

        // Native touch captures the original editor before cursor relocation;
        // release resolves the touched window and uses the same guarded worker.
        reset();
        layout.focus_restore_on_mouse_move = false;
        layout.focus_restore_delay = 80;
        *shared.touch_policy.lock().unwrap() =
            std::sync::Arc::new(crate::hook::touch_policy::TouchPolicy::from_layout(&layout));
        assert!(!native_contact(shared, true));
        let queue = InputAttachment::new(request.touched.thread);
        unsafe {
            let _ = SetForegroundWindow(hwnd(request.touched));
        }
        drop(queue);
        assert!(native_contact(shared, false));
        assert!(!native_contact(shared, false)); // duplicate release cannot re-arm
        let mut rect = windows::Win32::Foundation::RECT::default();
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowRect(
                hwnd(request.touched),
                &mut rect,
            )
            .unwrap();
        }
        finish_native(
            shared,
            POINT {
                x: rect.left + 40,
                y: rect.top + 60,
            },
        );
        wait_for_editor();
        println!("Native touch release restored the original editor");

        // AppBar swipes can leave no panel beneath the release point while a
        // different panel window remains foreground. Exercise real activation
        // with deliberately stale hit-testing, not just the target selector.
        reset();
        assert!(!native_contact(shared, true));
        let queue = InputAttachment::new(request.touched.thread);
        unsafe {
            let _ = SetForegroundWindow(hwnd(request.touched));
        }
        drop(queue);
        assert!(native_contact(shared, false));
        let stale_point = POINT {
            x: -30000,
            y: -30000,
        };
        assert_ne!(
            identity(unsafe { GetAncestor(WindowFromPoint(stale_point), GA_ROOT) }),
            Some(request.touched)
        );
        // Model the panel observed while the finger was still in contact,
        // before the AppBar moved away from the release point.
        NATIVE_CAPTURE.with(|capture| {
            let mut value = capture.get().unwrap();
            value.touched = Some(request.touched);
            capture.set(Some(value));
        });
        finish_native(shared, stale_point);
        wait_for_editor();
        println!("Native swipe with stale hit-testing restored the original editor");

        // Exercise queue attachment/detachment and process-handle ownership in
        // the production restoration path after the worker has warmed up.
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
        let handle_count = || {
            let mut count = 0;
            unsafe {
                GetProcessHandleCount(GetCurrentProcess(), &mut count).unwrap();
            }
            count
        };
        let mut repeated = request;
        repeated.sequence = SEQUENCE.load(Ordering::SeqCst);
        repeated.input_time = last_input_time().unwrap();
        assert_eq!(restore(shared, &repeated), "restored: window and editor");
        let before = handle_count();
        for _ in 0..500 {
            assert_eq!(restore(shared, &repeated), "restored: window and editor");
        }
        let after = handle_count();
        assert!(
            after <= before + 2,
            "native handles grew from {before} to {after}"
        );
        println!("500 focus restorations: native handles {before} -> {after}");
    }
}
