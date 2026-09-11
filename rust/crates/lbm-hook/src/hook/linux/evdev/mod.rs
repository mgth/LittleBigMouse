//! evdev/uinput backend: the real Linux router, the counterpart of the Windows
//! low-level hook. It grabs every physical mouse (`EVIOCGRAB`, so the compositor
//! no longer sees them), runs the unchanged engine over an authoritative cursor
//! position, and drives one uinput virtual pointer. LBM is the sole source of
//! pointer motion — exactly like Windows, where `SetCursorPos` is the whole game
//! — so there is no portal, no capture notification, no compositor round-trip,
//! and fast motion is handled because we own the pipeline synchronously.
//!
//! The virtual pointer is ABSOLUTE: its ABS_X/ABS_Y range is the desktop's
//! logical pixel size, which KWin maps 1:1 onto the whole desktop (verified
//! live). Absolute devices are not accelerated by libinput, so the position we
//! emit IS the position the cursor takes — no gain, no drift, and a warp is just
//! the next absolute point. (A relative virtual pointer would inherit KWin's
//! per-device acceleration and desync from the engine's zone geometry, putting
//! "walls" in the middle of screens.)
//!
//! A second uinput device, the virtual keyboard, re-emits the keyboard usages of
//! grabbed mice: wireless receivers (Logitech Lightspeed…) expose one combined
//! kbd+mouse node, and its onboard macro buttons emit KEY_* codes the pointer
//! device does not declare — the kernel silently drops undeclared codes, so
//! without it those keys vanish while LBM runs.
//!
//! # RULE — nothing potentially blocking on the routing thread
//!
//! From the first `EVIOCGRAB` the physical mice deliver ONLY to this process:
//! any stall in the pump freezes the user's pointer system-wide. Concretely:
//! - no device enumeration inline (~10 ms PER /dev/input node on some
//!   machines — audio jack-detection nodes; ~210 ms per full scan, measured by
//!   `examples/enum_bench.rs`) — enumeration lives on the scanner thread, the
//!   pump only drains its channel and grabs (a cheap ioctl);
//! - no blocking locks — the engine is accessed with `try_lock` (a contended
//!   frame emits raw / keeps cached bounds for one cycle);
//! - no allocation per report — every buffer the cycle needs lives in
//!   [`PumpBuffers`], owned by the `Router` and reused; a call into the global
//!   allocator a thousand times a second is a stall waiting to happen. Measured
//!   by `benches/evdev_pump.rs`;
//! - no synchronous IPC/DBus — the KWin cursor probe runs at arm time, BEFORE
//!   the grabs;
//! - no unbounded writes — state broadcasts go to sockets with a write
//!   timeout (ipc/server.rs); stderr must stay a file/journal, never an
//!   undrained pipe (the C# spawns the daemon with inherited handles — keep
//!   it that way);
//! - the only permitted wait is the bounded 100 ms `poll()` (and the
//!   equivalent sleep when no device is left).
//!
//! Audited 2026-07-19. The same rule applies to the other platform pumps
//! (hook/windows LL-hook callback, hook/linux/x11).
//!
//! Safety: a grab is released when its fd closes, so even `kill -9` frees the
//! mice. We additionally ungrab on unhook, on quit, and on drop; `LBM_EVDEV_
//! AUTORELEASE_SECS` force-unhooks after N seconds for cautious first runs.
//!
//! Freeing the devices is not enough on its own: a button held at that moment
//! was pressed on the *virtual* pointer, so it must be released there before the
//! device disappears, or the seat keeps it down and the user loses that button.
//! `Drop for Router` pays that debt, and the mask it reads is seeded at arm time
//! from `EVIOCGKEY` — a press that predates the grab never reached the pump.
//!
//! Ctrl-override reads the modifier from keyboards WITHOUT grabbing them (evdev
//! nodes are multi-reader; the compositor keeps them), plus the ctrl usages of
//! the grabbed combined nodes. Hot-plug is handled by a periodic rescan (new
//! mice would otherwise drive the cursor directly, next to the engine) and by
//! purging dead nodes — a removed device reports POLLERR forever and would
//! otherwise turn the pump into a busy loop. Focus-based exclusion is handled
//! by the [`super::focus`] watcher, common to all backends: it flips
//! `want_hook`, and the reconcile loop in `router` ungrabs/regrabs accordingly.
//!
//! # Layout
//!
//! The backend is one pipeline, split here along the seams it already had:
//!
//! * `devices` — finding the physical nodes, grabbing them, the hot-plug
//!   scanner thread and the removal of nodes that died;
//! * `frame` — the poll set, the event drain, the routing of each event to the
//!   pointer or the keyboard batch, and the composition of the frames. Owns the
//!   buffers the pump reuses;
//! * `uinput` — the two virtual devices this process creates;
//! * `cursor` — the authoritative cursor position the engine is run against;
//! * `probe` — asking the world where things are: the compositor's cursor
//!   position, the desktop bounds, a fallback start point;
//! * `router` — the pump itself, and the reconcile loop that arms and disarms
//!   it.

mod cursor;
mod devices;
mod frame;
mod probe;
mod router;
mod uinput;

// The backend's surface, unchanged by the split: the two entry points
// `hook::linux` reconciles with, the cursor probe `examples/cursor_probe` calls,
// and the pump pieces `benches/evdev_pump` drives without a device.
pub use cursor::EvdevCursor;
pub use devices::available;
pub use frame::{Frame, PumpBuffers};
pub use probe::kwin_cursor_pos;
pub use router::run;

// --- shared EV_KEY taxonomy ---------------------------------------------------
//
// Which codes are buttons and which are keyboard usages is the one piece of
// knowledge every module below needs: `frame` routes on it, `uinput` declares
// the complement of it on the virtual keyboard, `cursor` and `devices` index
// the held-button mask with it.

/// EV_KEY code ranges of mouse buttons. Everything else on a grabbed mouse is a
/// keyboard usage: wireless receivers (Logitech Lightspeed…) expose one combined
/// kbd+mouse node, and onboard macro buttons emit KEY_ESC/KEY_TAB/… on it. The
/// kernel silently drops events whose (type, code) is not declared on a uinput
/// device, so those keys must go to a virtual device that declares them.
pub(crate) const BTN_RANGE: std::ops::RangeInclusive<u16> = 0x100..=0x15f;
/// BTN_TRIGGER_HAPPY block — joystick buttons, not keyboard usages.
pub(crate) const BTN_TRIGGER_HAPPY_RANGE: std::ops::RangeInclusive<u16> = 0x2c0..=0x2e7;
/// BTN_LEFT..=BTN_TASK — the buttons whose being held counts as a drag, and
/// exactly the set the virtual pointer declares in `build_virtual`.
pub(crate) const BTN_MOUSE_RANGE: std::ops::RangeInclusive<u16> = 0x110..=0x117;
