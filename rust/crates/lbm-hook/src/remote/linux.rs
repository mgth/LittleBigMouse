//! uinput: how a Linux remote injects the stream its agent relays.
//!
//! A pair of virtual devices of its own, built by the same code as the evdev
//! backend's (`hook::linux::evdev::uinput`) and kept apart from them: an absolute
//! pointer whose range is the desktop, and a full keyboard. Their names start like
//! every device this process creates, which is how the evdev backend's device scan
//! knows never to grab the pointer nor read the keyboard's ctrl.
//!
//! Known limits, both outside this lot:
//! - when this machine's own evdev backend is routing, it owns the cursor position
//!   (`EvdevCursor`): a local mouse moved after the remote pointer resumes from where
//!   the backend last put the cursor, not from where the master left it;
//! - a freshly created device is not seen by the compositor at once, so the very
//!   first events after it is built can be lost. The devices are therefore built at
//!   the first injection and then kept for the life of the hook.

use std::sync::TryLockError;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AbsoluteAxisCode, EventType, InputEvent, RelativeAxisCode};

use crate::geometry::Rect;
use crate::hook::linux::evdev::{probe, uinput};
use crate::shared::Shared;

use super::coordinates::desktop_relative;
use super::{Injection, Injector};

/// How long a device that could not be created (no write access to /dev/uinput) is
/// left alone before trying again: not at every report.
const RETRY: Duration = Duration::from_secs(5);

/// Product ids beside the evdev backend's 0x4d55 (pointer) and 0x4d56 (keyboard).
const POINTER_PRODUCT: u16 = 0x4d57;
const KEYBOARD_PRODUCT: u16 = 0x4d58;

struct Pointer {
    device: VirtualDevice,
    /// The desktop the device's ABS range was declared for.
    desktop: Rect<i32>,
}

pub struct UinputInjector {
    shared: &'static Shared,
    pointer: Option<Pointer>,
    keyboard: Option<VirtualDevice>,
    pointer_failed: Option<Instant>,
    keyboard_failed: Option<Instant>,
    no_desktop_said: bool,
    /// Buttons this injector pressed and has not released: the pointer device is not
    /// rebuilt for a new desktop while one is held, which would drop it mid-drag.
    buttons_held: usize,
    /// Wheel travel not yet worth a whole notch, per axis, for the low-resolution
    /// codes; the high-resolution ones carry every 1/120th as it comes.
    wheel_rest: (i32, i32),
    pointer_frame: Vec<InputEvent>,
    keyboard_frame: Vec<InputEvent>,
}

impl UinputInjector {
    pub fn new(shared: &'static Shared) -> Self {
        Self {
            shared,
            pointer: None,
            keyboard: None,
            pointer_failed: None,
            keyboard_failed: None,
            no_desktop_said: false,
            buttons_held: 0,
            wheel_rest: (0, 0),
            pointer_frame: Vec::with_capacity(16),
            keyboard_frame: Vec::with_capacity(16),
        }
    }

    /// The desktop the pointer must span: the one the agent named with its last
    /// layout, else — for an agent that names none — the layout's own extent, else
    /// nothing. With nothing there is no pointer: a guessed desktop would put every
    /// position somewhere else, while the keys still work without one.
    fn desktop(&self) -> Option<Rect<i32>> {
        let named = match self.shared.desktop.try_lock() {
            Ok(desktop) => *desktop,
            Err(TryLockError::Poisoned(poisoned)) => *poisoned.into_inner(),
            // A `Load` is recording a new one: keep what there is for this batch.
            Err(TryLockError::WouldBlock) => return self.pointer.as_ref().map(|p| p.desktop),
        };
        if let Some(desktop) = named {
            return (desktop.width() > 0 && desktop.height() > 0).then_some(desktop);
        }
        if let Some(pointer) = &self.pointer {
            return Some(pointer.desktop);
        }
        let has_layout = !self
            .shared
            .engine
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .layout
            .main_zones
            .is_empty();
        // This machine's own zones: the desktop the injected pointer lands on.
        has_layout.then(|| probe::layout_bounds_blocking(self.shared).1)
    }

    /// A pointer device spanning the current desktop, built or rebuilt as needed.
    fn ensure_pointer(&mut self) {
        let Some(desktop) = self.desktop() else {
            if !self.no_desktop_said {
                self.no_desktop_said = true;
                eprintln!(
                    "[LittleBigMouse.Hook] remote input: no desktop known yet, \
                     the pointer is not injected (keys are)"
                );
            }
            return;
        };
        match &self.pointer {
            Some(pointer) if pointer.desktop == desktop => return,
            Some(_) if self.buttons_held > 0 => return,
            _ => {}
        }
        if self.pointer_failed.is_some_and(|at| at.elapsed() < RETRY) {
            return;
        }
        flush(
            &mut self.pointer_frame,
            self.pointer.as_mut().map(|p| &mut p.device),
        );
        match uinput::build_pointer(uinput::REMOTE_POINTER_NAME, POINTER_PRODUCT, desktop) {
            Ok(device) => {
                self.pointer = Some(Pointer { device, desktop });
                self.pointer_failed = None;
            }
            Err(error) => {
                eprintln!("[LittleBigMouse.Hook] remote input: no virtual pointer: {error}");
                self.pointer_failed = Some(Instant::now());
            }
        }
    }

    fn ensure_keyboard(&mut self) {
        if self.keyboard.is_some() || self.keyboard_failed.is_some_and(|at| at.elapsed() < RETRY) {
            return;
        }
        match uinput::build_keyboard(uinput::REMOTE_KEYBOARD_NAME, KEYBOARD_PRODUCT) {
            Ok(device) => {
                self.keyboard = Some(device);
                self.keyboard_failed = None;
            }
            Err(error) => {
                eprintln!("[LittleBigMouse.Hook] remote input: no virtual keyboard: {error}");
                self.keyboard_failed = Some(Instant::now());
            }
        }
    }

    /// Adds an event to the pointer's frame. Whatever the keyboard has pending goes
    /// out first, so that Ctrl pressed before a click is down when the click lands.
    fn queue_pointer(&mut self, event: InputEvent) {
        flush(&mut self.keyboard_frame, self.keyboard.as_mut());
        if conflicts(&self.pointer_frame, &event) {
            flush(
                &mut self.pointer_frame,
                self.pointer.as_mut().map(|p| &mut p.device),
            );
        }
        self.pointer_frame.push(event);
    }

    fn queue_keyboard(&mut self, event: InputEvent) {
        flush(
            &mut self.pointer_frame,
            self.pointer.as_mut().map(|p| &mut p.device),
        );
        if conflicts(&self.keyboard_frame, &event) {
            flush(&mut self.keyboard_frame, self.keyboard.as_mut());
        }
        self.keyboard_frame.push(event);
    }

    /// One axis of wheel travel: every 1/120th on the high-resolution code, and a
    /// notch on the legacy one each time a whole one has accumulated — libinput only
    /// reads the high-resolution codes from a device that declares them, the legacy
    /// one is for whoever reads the device directly.
    fn wheel(&mut self, travel: i32, hi_res: RelativeAxisCode, notch: RelativeAxisCode) {
        if travel == 0 {
            return;
        }
        let rest = if hi_res == RelativeAxisCode::REL_WHEEL_HI_RES {
            &mut self.wheel_rest.1
        } else {
            &mut self.wheel_rest.0
        };
        *rest = rest.saturating_add(travel);
        let notches = *rest / 120;
        *rest -= notches * 120;
        self.queue_pointer(InputEvent::new(EventType::RELATIVE.0, hi_res.0, travel));
        if notches != 0 {
            self.queue_pointer(InputEvent::new(EventType::RELATIVE.0, notch.0, notches));
        }
    }
}

/// Two values for one code cannot share a frame — the second would overwrite the
/// first — so a press and release of the same key, or two positions, are split.
fn conflicts(frame: &[InputEvent], event: &InputEvent) -> bool {
    frame
        .iter()
        .any(|e| e.event_type() == event.event_type() && e.code() == event.code())
}

/// Emits a pending frame (uinput appends the `SYN_REPORT`) and empties it. A frame for
/// a device that could not be built is dropped.
fn flush(frame: &mut Vec<InputEvent>, device: Option<&mut VirtualDevice>) {
    if frame.is_empty() {
        return;
    }
    if let Some(device) = device {
        if let Err(error) = device.emit(frame) {
            eprintln!("[LittleBigMouse.Hook] remote input: write failed: {error}");
        }
    }
    frame.clear();
}

impl Injector for UinputInjector {
    fn inject(&mut self, batch: &[Injection]) {
        let pointer = batch.iter().any(|i| !matches!(i, Injection::Key { .. }));
        if pointer {
            self.ensure_pointer();
        }
        if batch.iter().any(|i| matches!(i, Injection::Key { .. })) {
            self.ensure_keyboard();
        }
        for injection in batch {
            match *injection {
                Injection::MoveTo { x, y } => {
                    let Some(desktop) = self.pointer.as_ref().map(|p| p.desktop) else {
                        continue;
                    };
                    let ax = desktop_relative(x, desktop.left(), desktop.width());
                    let ay = desktop_relative(y, desktop.top(), desktop.height());
                    self.queue_pointer(InputEvent::new(
                        EventType::ABSOLUTE.0,
                        AbsoluteAxisCode::ABS_X.0,
                        ax,
                    ));
                    self.queue_pointer(InputEvent::new(
                        EventType::ABSOLUTE.0,
                        AbsoluteAxisCode::ABS_Y.0,
                        ay,
                    ));
                }
                Injection::Button { code, pressed } => {
                    // Counted even without a device: its release must balance it.
                    if pressed {
                        self.buttons_held += 1;
                    } else {
                        self.buttons_held = self.buttons_held.saturating_sub(1);
                    }
                    self.queue_pointer(InputEvent::new(EventType::KEY.0, code, pressed as i32));
                }
                Injection::Wheel {
                    horizontal,
                    vertical,
                } => {
                    self.wheel(
                        vertical,
                        RelativeAxisCode::REL_WHEEL_HI_RES,
                        RelativeAxisCode::REL_WHEEL,
                    );
                    self.wheel(
                        horizontal,
                        RelativeAxisCode::REL_HWHEEL_HI_RES,
                        RelativeAxisCode::REL_HWHEEL,
                    );
                }
                Injection::Key { code, pressed } => {
                    self.queue_keyboard(InputEvent::new(EventType::KEY.0, code, pressed as i32));
                }
            }
        }
        flush(
            &mut self.pointer_frame,
            self.pointer.as_mut().map(|p| &mut p.device),
        );
        flush(&mut self.keyboard_frame, self.keyboard.as_mut());
    }
}
