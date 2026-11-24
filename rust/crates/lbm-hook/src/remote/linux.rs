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
//!
//! A freshly created device is not seen by the compositor at once: what it emits in
//! the first few hundred milliseconds is lost (measured under KWin 6: everything up to
//! 300 ms after the creation, nothing from 450 ms on). Both devices are therefore
//! built when a driven session starts ([`Injector::begin`]), together, and kept for
//! the life of the hook; and what arrives for a device that young is held back until
//! it is [`SETTLE`] old — once, at the start of the first session — and then put in
//! ([`Injector::catch_up`]). Held back, not waited for: the endpoint keeps reading,
//! and a master moving at 1 kHz must not see its stream back up.

use std::sync::TryLockError;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AbsoluteAxisCode, EventType, InputEvent, LedCode, RelativeAxisCode};

use crate::geometry::Rect;
use crate::hook::linux::evdev::{probe, uinput};
use crate::shared::Shared;

use super::coordinates::desktop_relative;
use super::{Injection, Injector};

/// How long a device that could not be created (no write access to /dev/uinput) is
/// left alone before trying again: not at every report.
const RETRY: Duration = Duration::from_secs(5);

/// How old a device must be before what it emits reaches the compositor's clients;
/// see the module's note.
const SETTLE: Duration = Duration::from_millis(700);

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
    /// What may be written now, what waits for a young device, and what was pressed.
    gate: Gate,
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
            gate: Gate::default(),
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
                self.gate.young_until(Instant::now() + SETTLE);
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
                self.gate.young_until(Instant::now() + SETTLE);
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

/// What a session sent while its devices were too young to be written to, in order.
/// Motion and wheel are folded into the one before when it is of the same kind, so a
/// fast mouse adds nothing: what grows it is presses, releases and the motion between
/// them, a few hundred milliseconds' worth.
#[derive(Debug, Default)]
struct Backlog(Vec<Injection>);

impl Backlog {
    fn push(&mut self, injection: Injection) {
        match (self.0.last_mut(), injection) {
            (Some(Injection::MoveTo { x, y }), Injection::MoveTo { x: nx, y: ny }) => {
                (*x, *y) = (nx, ny);
            }
            (
                Some(Injection::Wheel {
                    horizontal,
                    vertical,
                }),
                Injection::Wheel {
                    horizontal: more_h,
                    vertical: more_v,
                },
            ) => {
                *horizontal = horizontal.saturating_add(more_h);
                *vertical = vertical.saturating_add(more_v);
            }
            _ => self.0.push(injection),
        }
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn take(&mut self) -> Vec<Injection> {
        std::mem::take(&mut self.0)
    }
}

/// The rules between the stream and the devices, kept apart from the devices so they
/// are tested without one: input is held back while a device is too young (see the
/// module's note), in order, and every press handed out to be written is remembered
/// until its release is — what the system holds pressed because of this injector.
#[derive(Debug, Default)]
struct Gate {
    /// Until when the youngest device cannot be written to usefully.
    settled_at: Option<Instant>,
    backlog: Backlog,
    /// Presses handed out and not released since, in pressing order.
    down: Vec<Injection>,
}

impl Gate {
    /// A device was just built: what arrives before `at` is held back.
    fn young_until(&mut self, at: Instant) {
        self.settled_at = Some(at);
    }

    /// What of `batch` is to be written now: nothing while a device is young, else
    /// whatever was held back first, then the batch.
    fn pass(&mut self, batch: &[Injection], now: Instant) -> Vec<Injection> {
        if self.settled_at.is_some_and(|at| now < at) {
            for injection in batch {
                self.backlog.push(*injection);
            }
            return Vec::new();
        }
        let mut out = self.catch_up();
        out.extend_from_slice(batch);
        self.handed_out(&out);
        out
    }

    /// When what was held back is to be written, with or without more input.
    fn due(&self) -> Option<Instant> {
        if self.backlog.is_empty() {
            return None;
        }
        Some(self.settled_at.unwrap_or_else(Instant::now))
    }

    /// What was held back, to be written now.
    fn catch_up(&mut self) -> Vec<Injection> {
        self.settled_at = None;
        let held = self.backlog.take();
        self.handed_out(&held);
        held
    }

    /// Injection stops: what was held back is dropped — a press there never reached
    /// the system — and the releases of every press that did are returned, to be
    /// written now: buttons first, then keys in the reverse of their pressing order, as
    /// a session's end does it.
    fn discard(&mut self) -> Vec<Injection> {
        self.backlog.take();
        let down = std::mem::take(&mut self.down);
        let buttons = down.iter().filter_map(|i| match *i {
            Injection::Button { code, .. } => Some(Injection::Button {
                code,
                pressed: false,
            }),
            _ => None,
        });
        let keys = down.iter().filter_map(|i| match *i {
            Injection::Key { code, .. } => Some(Injection::Key {
                code,
                pressed: false,
            }),
            _ => None,
        });
        buttons.rev().chain(keys.rev()).collect()
    }

    fn handed_out(&mut self, out: &[Injection]) {
        for injection in out {
            match *injection {
                Injection::Button { pressed: true, .. } | Injection::Key { pressed: true, .. } => {
                    if !self.down.contains(injection) {
                        self.down.push(*injection);
                    }
                }
                Injection::Button {
                    code,
                    pressed: false,
                } => self.down.retain(|d| {
                    *d != Injection::Button {
                        code,
                        pressed: true,
                    }
                }),
                Injection::Key {
                    code,
                    pressed: false,
                } => self.down.retain(|d| {
                    *d != Injection::Key {
                        code,
                        pressed: true,
                    }
                }),
                _ => {}
            }
        }
    }
}

/// This machine's lock keys, as the LEDs of its keyboards show them: the compositor (or
/// the X server) lights them on every keyboard from its one keyboard state, which is
/// the state a tap of the virtual keyboard toggles. Read when a master asks, at a
/// crossing: the devices are listed then, not watched. A lock no keyboard has a LED
/// for is unknown, and left alone.
fn keyboard_locks() -> lbm_ipc::input::Locks {
    let keyboards = evdev::enumerate().filter_map(|(_, device)| {
        if device.name().is_some_and(uinput::is_ours) {
            return None;
        }
        let supported = device.supported_leds()?;
        let lit = device.get_led_state().ok()?;
        let led = |code: LedCode| supported.contains(code).then(|| lit.contains(code));
        Some([
            led(LedCode::LED_CAPSL),
            led(LedCode::LED_NUML),
            led(LedCode::LED_SCROLLL),
        ])
    });
    locks_of(keyboards)
}

/// The locks several keyboards show: on when any shows it on, unknown when none has
/// the LED. Any rather than all, because a keyboard that missed an update stays dark.
fn locks_of(keyboards: impl Iterator<Item = [Option<bool>; 3]>) -> lbm_ipc::input::Locks {
    let mut seen = [None; 3];
    for keyboard in keyboards {
        for (seen, lit) in seen.iter_mut().zip(keyboard) {
            if let Some(lit) = lit {
                *seen = Some(seen.unwrap_or(false) || lit);
            }
        }
    }
    let [caps, num, scroll] = seen;
    lbm_ipc::input::Locks { caps, num, scroll }
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
        let batch = &*super::expand_locks(batch, keyboard_locks);
        let pointer = batch.iter().any(|i| !matches!(i, Injection::Key { .. }));
        if pointer {
            self.ensure_pointer();
        }
        if batch.iter().any(|i| matches!(i, Injection::Key { .. })) {
            self.ensure_keyboard();
        }
        let now = self.gate.pass(batch, Instant::now());
        self.write(&now);
    }

    fn due(&self) -> Option<Instant> {
        self.gate.due()
    }

    /// What was held back is dropped, and what reached the system is let go of at
    /// once: a key pressed on a settled keyboard may have its release held back behind
    /// a pointer rebuilt for a new desktop.
    fn discard(&mut self) {
        let releases = self.gate.discard();
        self.write(&releases);
    }

    fn catch_up(&mut self) {
        let held = self.gate.catch_up();
        self.write(&held);
    }

    /// Nothing to see: a uinput device moves the compositor's cursor but cannot hide
    /// it, and there is no portable way to: a Wayland client chooses the cursor only
    /// over its own surfaces, and the Windows trick of a one-pixel window under the
    /// pointer would need a topmost, input-taking surface no portable protocol offers.
    /// Moving it away instead — a corner, an edge — is what sets off hot corners and
    /// edge actions. The pointer stays where the master left it.
    fn park(&mut self) {}

    fn unpark(&mut self) {}

    /// Builds both devices ahead of the session's first motion, so that they settle
    /// together and a first key is not held back on its own. Never on the hook's way
    /// out: that is `unpark` alone.
    fn begin(&mut self) {
        self.ensure_pointer();
        self.ensure_keyboard();
    }
}

/// The stream's injections into the devices, as they come.
impl UinputInjector {
    fn write(&mut self, batch: &[Injection]) {
        if batch.is_empty() {
            return;
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
                // Turned into keys above (expand_locks), or delivered through
                // park/begin (super::deliver), never in a batch.
                Injection::Locks(_) | Injection::Park | Injection::Unpark => {}
            }
        }
        flush(
            &mut self.pointer_frame,
            self.pointer.as_mut().map(|p| &mut p.device),
        );
        flush(&mut self.keyboard_frame, self.keyboard.as_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BTN_LEFT: u16 = 0x110;
    const LEFT_CTRL: u16 = 29;

    fn held(injections: &[Injection]) -> Vec<Injection> {
        let mut backlog = Backlog::default();
        for injection in injections {
            backlog.push(*injection);
        }
        backlog.take()
    }

    fn key(code: u16, pressed: bool) -> Injection {
        Injection::Key { code, pressed }
    }

    fn button(code: u16, pressed: bool) -> Injection {
        Injection::Button { code, pressed }
    }

    #[test]
    fn a_young_device_holds_input_back_until_it_is_due() {
        let t0 = Instant::now();
        let mut gate = Gate::default();
        gate.young_until(t0 + SETTLE);
        assert_eq!(gate.pass(&[key(LEFT_CTRL, true)], t0), []);
        assert_eq!(gate.due(), Some(t0 + SETTLE));
        assert_eq!(gate.catch_up(), [key(LEFT_CTRL, true)]);
        assert_eq!(gate.due(), None);
        assert_eq!(
            gate.pass(&[key(LEFT_CTRL, false)], t0 + SETTLE),
            [key(LEFT_CTRL, false)]
        );
    }

    /// Ctrl went down on a settled keyboard; the pointer is then rebuilt for a new
    /// desktop and the release of Ctrl is held back behind it. Stopping there must not
    /// drop that release with the rest.
    #[test]
    fn a_stop_releases_a_key_pressed_before_a_pointer_rebuild() {
        let t0 = Instant::now();
        let mut gate = Gate::default();
        gate.young_until(t0 + SETTLE);
        assert_eq!(
            gate.pass(&[key(LEFT_CTRL, true)], t0 + SETTLE),
            [key(LEFT_CTRL, true)]
        );
        let rebuilt = t0 + 2 * SETTLE;
        gate.young_until(rebuilt + SETTLE);
        assert_eq!(gate.pass(&[key(LEFT_CTRL, false)], rebuilt), []);
        assert_eq!(gate.discard(), [key(LEFT_CTRL, false)]);
        assert_eq!(gate.due(), None, "nothing left to put in later");
        assert_eq!(gate.discard(), [], "released once");
    }

    #[test]
    fn a_stop_drops_presses_that_never_reached_the_system_with_their_releases() {
        let t0 = Instant::now();
        let mut gate = Gate::default();
        gate.young_until(t0 + SETTLE);
        gate.pass(&[button(BTN_LEFT, true), key(LEFT_CTRL, true)], t0);
        assert_eq!(gate.discard(), []);
        assert_eq!(gate.due(), None);
    }

    #[test]
    fn a_stop_lets_go_of_buttons_first_then_keys_in_reverse() {
        let t0 = Instant::now();
        let mut gate = Gate::default();
        const C: u16 = 46;
        gate.pass(
            &[
                key(LEFT_CTRL, true),
                key(C, true),
                key(C, true),
                button(BTN_LEFT, true),
            ],
            t0,
        );
        assert_eq!(
            gate.discard(),
            [
                button(BTN_LEFT, false),
                key(C, false),
                key(LEFT_CTRL, false)
            ]
        );
    }

    #[test]
    fn a_fast_mouse_held_back_is_one_position_the_last() {
        let motion: Vec<_> = (0..1000)
            .map(|i| Injection::MoveTo { x: i, y: 2 * i })
            .collect();
        assert_eq!(held(&motion), [Injection::MoveTo { x: 999, y: 1998 }]);
    }

    #[test]
    fn presses_and_releases_keep_their_place_between_the_positions() {
        assert_eq!(
            held(&[
                Injection::MoveTo { x: 1, y: 1 },
                Injection::MoveTo { x: 2, y: 2 },
                Injection::Key {
                    code: LEFT_CTRL,
                    pressed: true
                },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Injection::MoveTo { x: 3, y: 3 },
                Injection::MoveTo { x: 4, y: 4 },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                Injection::Key {
                    code: LEFT_CTRL,
                    pressed: false
                },
            ]),
            [
                Injection::MoveTo { x: 2, y: 2 },
                Injection::Key {
                    code: LEFT_CTRL,
                    pressed: true
                },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: true
                },
                Injection::MoveTo { x: 4, y: 4 },
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                Injection::Key {
                    code: LEFT_CTRL,
                    pressed: false
                },
            ]
        );
    }

    #[test]
    fn wheel_travel_held_back_is_summed_not_lost() {
        assert_eq!(
            held(&[
                Injection::Wheel {
                    horizontal: 0,
                    vertical: 30
                },
                Injection::Wheel {
                    horizontal: 15,
                    vertical: 60
                },
                Injection::MoveTo { x: 0, y: 0 },
                Injection::Wheel {
                    horizontal: 0,
                    vertical: -120
                },
            ]),
            [
                Injection::Wheel {
                    horizontal: 15,
                    vertical: 90
                },
                Injection::MoveTo { x: 0, y: 0 },
                Injection::Wheel {
                    horizontal: 0,
                    vertical: -120
                },
            ]
        );
    }

    #[test]
    fn a_lock_is_on_when_a_keyboard_shows_it_and_unknown_when_none_can() {
        let none = locks_of(std::iter::empty());
        assert_eq!(none, lbm_ipc::input::Locks::default());
        let laptop = [Some(true), Some(false), None];
        let usb = [Some(false), Some(false), None];
        let locks = locks_of([usb, laptop].into_iter());
        assert_eq!(
            (locks.caps, locks.num, locks.scroll),
            (Some(true), Some(false), None)
        );
    }
}
