//! `SendInput`: how a Windows remote injects the stream its agent relays.
//!
//! Every injected event carries [`INJECTED`] in its `dwExtraInfo`. This machine's own
//! LittleBigMouse hook sees those events go by in `mouse_proc` and lets them through
//! untouched: the master's engine already routed that pointer over the whole layout,
//! and routing it a second time here would put back the walls the master took away.
//!
//! Between sessions the pointer is parked out of sight ([`hider`]).
//!
//! While a session lasts, the window in the foreground is watched ([`foreground`]):
//! when it belongs to a process of a higher integrity level, Windows drops everything
//! injected here, and the agent is told so.
//!
//! `lbm-inject`, which the Windows service launches with a SYSTEM token, uses the same
//! injector built by [`SendInputInjector::following_input_desktop`]: no foreground to
//! watch (no window is above SYSTEM), and the thread moved to the input desktop instead,
//! the secure one included ([`input_desktop`]).

mod foreground;
mod hider;
mod input_desktop;

pub use hider::local_pointer_input;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE,
    MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
    MOUSE_EVENT_FLAGS, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    XBUTTON1, XBUTTON2,
};

use super::coordinates::normalised_absolute;
use super::keymap::{windows_key, WindowsKey};
use super::{Injection, Injector};

/// The `dwExtraInfo` of every event this module injects: "LBMR". Unlike the touch
/// signatures (`0xFF5157xx`), which Windows sets, this one is ours to choose; it only
/// has to be unlikely in anyone else's injected input.
pub const INJECTED: usize = 0x4c42_4d52;

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const BTN_SIDE: u16 = 0x113;
const BTN_EXTRA: u16 = 0x114;
const BTN_FORWARD: u16 = 0x115;
const BTN_BACK: u16 = 0x116;

pub struct SendInputInjector {
    inputs: Vec<INPUT>,
    hider: hider::Hider,
    /// What this injector runs as: the hook, or `lbm-inject`.
    mode: Mode,
}

enum Mode {
    /// In the hook: the foreground is watched, and the agent told through `shared`
    /// when a window above this process drops the input.
    Hook(&'static crate::shared::Shared),
    /// In `lbm-inject`: the thread follows the input desktop.
    Inject(input_desktop::InputDesktop),
}

impl SendInputInjector {
    pub fn new(shared: &'static crate::shared::Shared) -> Self {
        Self::with(Mode::Hook(shared))
    }

    /// The injector of `lbm-inject`. Its thread must own no window and no hook, or it
    /// cannot be moved to another desktop.
    pub fn following_input_desktop() -> Self {
        Self::with(Mode::Inject(input_desktop::InputDesktop::default()))
    }

    fn with(mode: Mode) -> Self {
        Self {
            inputs: Vec::with_capacity(16),
            hider: hider::Hider::default(),
            mode,
        }
    }

    fn mouse(&mut self, dx: i32, dy: i32, data: u32, flags: MOUSE_EVENT_FLAGS) {
        self.inputs.push(INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECTED,
                },
            },
        });
    }

    fn key(&mut self, vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) {
        self.inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: INJECTED,
                },
            },
        });
    }
}

/// The virtual screen as `(left, top, width, height)`, in physical pixels since the
/// process is per-monitor DPI aware. Read per batch rather than cached: a display
/// change must not leave the pointer landing by the old geometry.
/// This session's lock keys: the toggle bit of `GetKeyState`. Read on the injector's
/// thread, whose view of the keyboard is the one the injected taps change.
fn local_locks() -> lbm_ipc::input::Locks {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_CAPITAL, VK_NUMLOCK, VK_SCROLL,
    };
    // SAFETY: no pointer; any virtual key may be asked.
    let toggled = |vk: VIRTUAL_KEY| Some(unsafe { GetKeyState(i32::from(vk.0)) } & 1 != 0);
    lbm_ipc::input::Locks {
        caps: toggled(VK_CAPITAL),
        num: toggled(VK_NUMLOCK),
        scroll: toggled(VK_SCROLL),
    }
}

fn virtual_screen() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

/// The flags and mouse data of a Linux button code, or `None` for a button Windows has
/// no message for. `BTN_SIDE`/`BTN_EXTRA` are what most mice report for their thumb
/// buttons, `BTN_BACK`/`BTN_FORWARD` what some others do; both pairs are Windows'
/// X buttons, back first.
fn button(code: u16, pressed: bool) -> Option<(MOUSE_EVENT_FLAGS, u32)> {
    let pick = |down, up| if pressed { down } else { up };
    Some(match code {
        BTN_LEFT => (pick(MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP), 0),
        BTN_RIGHT => (pick(MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP), 0),
        BTN_MIDDLE => (pick(MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP), 0),
        BTN_SIDE | BTN_BACK => (pick(MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP), XBUTTON1 as u32),
        BTN_EXTRA | BTN_FORWARD => (pick(MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP), XBUTTON2 as u32),
        _ => return None,
    })
}

impl Injector for SendInputInjector {
    fn inject(&mut self, batch: &[Injection]) {
        let batch = &*super::expand_locks(batch, local_locks);
        self.inputs.clear();
        let mut screen = None;
        for injection in batch {
            match *injection {
                Injection::MoveTo { x, y } => {
                    let (left, top, width, height) = *screen.get_or_insert_with(virtual_screen);
                    self.mouse(
                        normalised_absolute(x, left, width),
                        normalised_absolute(y, top, height),
                        0,
                        MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | MOUSEEVENTF_MOVE,
                    );
                }
                Injection::Button { code, pressed } => {
                    if let Some((flags, data)) = button(code, pressed) {
                        self.mouse(0, 0, data, flags);
                    }
                }
                // Both systems count 120 to a notch, away from the user and to the
                // right being positive: the stream's units are Windows' own.
                Injection::Wheel {
                    horizontal,
                    vertical,
                } => {
                    if vertical != 0 {
                        self.mouse(0, 0, vertical as u32, MOUSEEVENTF_WHEEL);
                    }
                    if horizontal != 0 {
                        self.mouse(0, 0, horizontal as u32, MOUSEEVENTF_HWHEEL);
                    }
                }
                Injection::Key { code, pressed } => {
                    let up = if pressed {
                        KEYBD_EVENT_FLAGS(0)
                    } else {
                        KEYEVENTF_KEYUP
                    };
                    match windows_key(code) {
                        Some(WindowsKey::Scan { code, extended }) => {
                            let extended = if extended {
                                KEYEVENTF_EXTENDEDKEY
                            } else {
                                KEYBD_EVENT_FLAGS(0)
                            };
                            self.key(0, code, KEYEVENTF_SCANCODE | extended | up);
                        }
                        Some(WindowsKey::Virtual { vk, scan, extended }) => {
                            let extended = if extended {
                                KEYEVENTF_EXTENDEDKEY
                            } else {
                                KEYBD_EVENT_FLAGS(0)
                            };
                            self.key(vk, scan, extended | up);
                        }
                        None => {}
                    }
                }
                // Delivered through park/unpark (super::deliver), never in a batch.
                // Turned into keys above (expand_locks), or delivered through
                // park/unpark (super::deliver), never here.
                Injection::Locks(_) | Injection::Park | Injection::Unpark => {}
            }
        }
        if self.inputs.is_empty() {
            return;
        }
        let inputs = &self.inputs;
        let send = |start: usize| {
            // SAFETY: a slice of initialised INPUTs and their size.
            unsafe { SendInput(&inputs[start..], std::mem::size_of::<INPUT>() as i32) as usize }
        };
        match &mut self.mode {
            Mode::Hook(_) => {
                foreground::input();
                // A UIPI block is silent: `SendInput` counts what it dropped as sent,
                // which is why the foreground is watched. Fewer than asked is a desktop
                // this process cannot reach (a secure desktop): as final, said the same
                // way, once.
                if send(0) < inputs.len() {
                    foreground::short();
                }
            }
            Mode::Inject(desktop) => {
                if !super::stream::send_following(inputs.len(), send, || desktop.attach()) {
                    eprintln!("[lbm-inject] input lost: SendInput fell short");
                }
            }
        }
    }

    fn begin(&mut self) {
        match &mut self.mode {
            Mode::Hook(shared) => foreground::begin(shared),
            // The secure desktop may have come up while nothing was driven.
            Mode::Inject(desktop) => {
                desktop.attach();
            }
        }
        self.unpark();
    }

    fn park(&mut self) {
        if let Mode::Hook(_) = self.mode {
            foreground::end();
        }
        self.hider.park();
    }

    fn unpark(&mut self) {
        self.hider.unpark();
    }
}
