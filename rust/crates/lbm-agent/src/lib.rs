//! The v6 resident process of LittleBigMouse (`docs/v6-architecture-plan.md` on the
//! `v6` branch, phase 3), as a library for its binary and its tests.
//!
//! - [`reconcile`]: the decisions — when a display change rebuilds the layout, when the
//!   engine is hooked — as a state machine without I/O.
//! - [`hook`]: the connection to the hook, kept up.
//! - [`fake_hook`]: a hook that hooks nothing, for `--fake-hook` and the tests.
//! - [`world`]: the displays, the profiles and the layout the agent acts on;
//!   [`discovery`]: where the displays come from (Linux backends, the Win32 tree).
//! - [`watch`]: the display changes the hook does not report (Linux: uevents, inotify,
//!   and the poll behind them).
//! - [`supervise`]: launching a hook when none answers (D5).
//! - [`gap_guard`]: the 1 px gaps KWin's barriers need while the engine runs (D7).
//! - [`runtime`]: the event loop tying them together.
//! - [`instance`] and [`log`]: one agent per session, and its log over five runs.
//! - [`api`]: the frontends' way in (JSON over a local socket or pipe, D6).
//! - `sleep` (Linux): system sleep from logind.
//! - [`tray`] and [`icons`]: the tray, a frontend in process (Linux: `ksni`; Windows:
//!   the notification area).
//! - [`autostart`] and [`schtask`]: starting with the session (XDG autostart, the
//!   Windows scheduled task).
//! - [`elevation`] (Windows): the one UAC consent, when it is wanted and possible.
//! - `winpipe` (Windows): the per-session pipes, the hook's and the agent's.

pub mod api;
pub mod autostart;
pub mod discovery;
pub mod elevation;
pub mod fake_hook;
pub mod gap_guard;
pub mod hook;
#[cfg(any(target_os = "linux", windows))]
pub mod icons;
pub mod instance;
pub mod log;
pub mod reconcile;
pub mod runtime;
pub mod schtask;
#[cfg(target_os = "linux")]
pub mod sleep;
pub mod supervise;
#[cfg(any(target_os = "linux", windows))]
pub mod tray;
pub mod wallpaper;
pub mod watch;
#[cfg(windows)]
pub mod winpipe;
#[cfg(windows)]
pub mod winwatch;
pub mod world;
