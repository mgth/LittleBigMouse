//! Remote injection: what a machine driven by another does with the input stream its
//! agent relays (docs/multi-pc.md, lot 4).
//!
//! Two layers. A [`Session`] reads `lbm_ipc::input` events and decides what may reach
//! the system: nothing before an `Enter`, and, whenever the stream ends one way or
//! another, a release for every key and button it left pressed — the safety rule of
//! the multi-PC mode. An [`Injector`] then does what the session decided, with the
//! one call each system offers: `SendInput` on Windows, a pair of uinput devices on
//! Linux. Keeping the decisions out of the injectors is what lets them be tested
//! without touching a real device, and lets both systems keep the same rules.

pub mod coordinates;
pub mod keymap;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
pub mod windows;

use lbm_ipc::input::InputEvent;

/// One thing to do to the system, in the receiver's own terms: positions in pixels of
/// its desktop, buttons and keys as Linux codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injection {
    MoveTo { x: i32, y: i32 },
    Button { code: u16, pressed: bool },
    Wheel { horizontal: i32, vertical: i32 },
    Key { code: u16, pressed: bool },
}

/// Puts injections into the system. Called with one batch at a time — everything one
/// report of the sender's devices produced — so an implementation can hand the whole
/// batch to the system in one go, as both real ones do.
///
/// Called from the input endpoint's own thread, never the routing thread: an
/// implementation may block on its system call.
pub trait Injector: Send + 'static {
    fn inject(&mut self, batch: &[Injection]);
}

/// The input stream of one driving client, as far as it may reach the system.
#[derive(Debug, Default)]
pub struct Session {
    driven: bool,
    /// What the stream pressed and has not released yet, in pressing order. Bounded by
    /// the number of codes there are, whatever a client sends.
    buttons: Vec<u16>,
    keys: Vec<u16>,
    out: Vec<Injection>,
}

impl Session {
    /// What a batch of the stream does to the system.
    ///
    /// A press of something already pressed goes through: it is the sender's key
    /// repeat, which Windows does not produce by itself for injected keys. A release
    /// of something this stream never pressed does not: it would let a stream lift a
    /// key the local user is holding.
    pub fn apply(&mut self, events: &[InputEvent]) -> &[Injection] {
        self.out.clear();
        for event in events {
            match *event {
                InputEvent::Enter { x, y } => {
                    self.driven = true;
                    self.out.push(Injection::MoveTo { x, y });
                }
                InputEvent::Leave => self.release_into_out(),
                // Not driven: the pointer is not ours to move nor the keyboard ours to
                // type on, whatever arrives.
                _ if !self.driven => {}
                InputEvent::Motion { x, y } => self.out.push(Injection::MoveTo { x, y }),
                InputEvent::Button { code, pressed } => {
                    if track(&mut self.buttons, code, pressed) {
                        self.out.push(Injection::Button { code, pressed });
                    }
                }
                InputEvent::Wheel {
                    horizontal,
                    vertical,
                } => {
                    if horizontal != 0 || vertical != 0 {
                        self.out.push(Injection::Wheel {
                            horizontal,
                            vertical,
                        });
                    }
                }
                InputEvent::Key { code, pressed } => {
                    if track(&mut self.keys, code, pressed) {
                        self.out.push(Injection::Key { code, pressed });
                    }
                }
            }
        }
        &self.out
    }

    /// The stream ended without a `Leave` — the client went away, sent garbage, or a
    /// newer one took its place — or the hook is leaving: the same releases a `Leave`
    /// would have produced. Empty when nothing is held.
    pub fn release_all(&mut self) -> &[Injection] {
        self.out.clear();
        self.release_into_out();
        &self.out
    }

    /// Buttons first, so a drag ends before its modifier does (a Ctrl+drag copies,
    /// a drag without it moves); then keys in the reverse of their pressing order, so
    /// Ctrl+C lets go of C before Ctrl.
    fn release_into_out(&mut self) {
        self.driven = false;
        for code in self.buttons.drain(..).rev() {
            self.out.push(Injection::Button {
                code,
                pressed: false,
            });
        }
        for code in self.keys.drain(..).rev() {
            self.out.push(Injection::Key {
                code,
                pressed: false,
            });
        }
    }
}

/// Records a press or release in `held`; whether it is to be injected.
fn track(held: &mut Vec<u16>, code: u16, pressed: bool) -> bool {
    if pressed {
        if !held.contains(&code) {
            held.push(code);
        }
        return true;
    }
    match held.iter().position(|&c| c == code) {
        Some(index) => {
            held.remove(index);
            true
        }
        None => false,
    }
}

/// This system's injector.
#[cfg(windows)]
pub fn platform_injector(_shared: &'static crate::shared::Shared) -> Box<dyn Injector> {
    Box::new(windows::SendInputInjector::new())
}

/// This system's injector. The desktop the pointer device spans is read from `shared`.
#[cfg(target_os = "linux")]
pub fn platform_injector(shared: &'static crate::shared::Shared) -> Box<dyn Injector> {
    Box::new(linux::UinputInjector::new(shared))
}

/// No injection on a system this hook has no backend for: the endpoint still serves,
/// and what it receives goes nowhere.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn platform_injector(_shared: &'static crate::shared::Shared) -> Box<dyn Injector> {
    struct Nowhere;
    impl Injector for Nowhere {
        fn inject(&mut self, _batch: &[Injection]) {}
    }
    Box::new(Nowhere)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT_CTRL: u16 = 29;
    const C: u16 = 46;
    const BTN_LEFT: u16 = 0x110;

    fn key(code: u16, pressed: bool) -> InputEvent {
        InputEvent::Key { code, pressed }
    }

    #[test]
    fn nothing_reaches_the_system_before_an_enter() {
        let mut session = Session::default();
        let ignored = [
            InputEvent::Motion { x: 1, y: 2 },
            InputEvent::Button {
                code: BTN_LEFT,
                pressed: true,
            },
            InputEvent::Wheel {
                horizontal: 0,
                vertical: 120,
            },
            key(C, true),
        ];
        assert_eq!(session.apply(&ignored), []);
        assert_eq!(session.release_all(), []);
    }

    #[test]
    fn a_leave_releases_buttons_then_keys_in_reverse_order() {
        let mut session = Session::default();
        session.apply(&[
            InputEvent::Enter { x: 0, y: 0 },
            key(LEFT_CTRL, true),
            key(C, true),
            InputEvent::Button {
                code: BTN_LEFT,
                pressed: true,
            },
        ]);
        assert_eq!(
            session.apply(&[InputEvent::Leave, key(C, true)]),
            [
                Injection::Button {
                    code: BTN_LEFT,
                    pressed: false
                },
                Injection::Key {
                    code: C,
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
    fn a_repeated_press_goes_through_and_is_released_once() {
        let mut session = Session::default();
        session.apply(&[InputEvent::Enter { x: 0, y: 0 }]);
        assert_eq!(session.apply(&[key(C, true), key(C, true)]).len(), 2);
        assert_eq!(
            session.release_all(),
            [Injection::Key {
                code: C,
                pressed: false
            }]
        );
    }

    #[test]
    fn a_release_of_something_never_pressed_is_not_injected() {
        let mut session = Session::default();
        session.apply(&[InputEvent::Enter { x: 0, y: 0 }]);
        assert_eq!(session.apply(&[key(C, false)]), []);
    }

    #[test]
    fn a_second_enter_moves_the_pointer_and_keeps_what_is_held() {
        let mut session = Session::default();
        session.apply(&[InputEvent::Enter { x: 0, y: 0 }, key(C, true)]);
        assert_eq!(
            session.apply(&[InputEvent::Enter { x: 5, y: 6 }]),
            [Injection::MoveTo { x: 5, y: 6 }]
        );
        assert_eq!(session.release_all().len(), 1);
    }

    #[test]
    fn a_still_wheel_is_not_injected() {
        let mut session = Session::default();
        session.apply(&[InputEvent::Enter { x: 0, y: 0 }]);
        assert_eq!(
            session.apply(&[InputEvent::Wheel {
                horizontal: 0,
                vertical: 0
            }]),
            []
        );
    }
}
