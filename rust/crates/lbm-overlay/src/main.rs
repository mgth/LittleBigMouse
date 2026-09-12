//! Spike: can a ruler be put where it has to go?
//!
//! Phase 6 calls the overlays the hard part, and it is right: the rulers and the
//! resistance bands are drawn *on the screens themselves*, at exact desktop pixels, on
//! top of everything, without taking the desktop hostage.
//!
//! Two backs, because the two display servers refuse and allow different things:
//!
//! * **Wayland** forbids a client to position itself at all. `zwlr_layer_shell_v1` is
//!   the way round it — an anchor and margins are a position — and KWin has it. It also
//!   avoids a question the plan's other suggestion raises: `lbm-pattern` exists because
//!   KWin rescales XWayland surfaces on any screen whose scale differs from the global
//!   factor. Measured here, XWayland is 1:1 — but both screens are at 1.25, so this
//!   machine cannot show the failure that report describes, and nothing measured either
//!   clears or condemns XWayland. Layer-shell does not have to care, which is reason
//!   enough.
//! * **X11** allows everything and asks nothing: a client places its own window, and an
//!   override-redirect one is not even the window manager's business. What it takes
//!   instead is a 32-bit visual for the transparency and the Shape extension for the
//!   pointer to pass through. The interesting part is that the server can be *asked
//!   back* whether it granted all three — a stronger check than the Wayland side can
//!   make, where the protocol has no way to read a surface's state.
//!
//! The two also count in different coordinates, which is a finding and not a detail:
//! Wayland places in the output's logical pixels (a 3840-wide screen at scale 1.25 is
//! 3072 of them), X11 in the server's device pixels. A ruler has to be told which.
//!
//! Usage: `lbm-overlay-spike [seconds] [output-name] [--x11]`, e.g.
//! `lbm-overlay-spike 5 DP-3 --x11`. Without `--x11` it takes the Wayland road when
//! `WAYLAND_DISPLAY` is set.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("lbm-overlay-spike: Linux only for now; the Windows spike is its own");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

/// What the band is asked to be, and where, from the chosen screen's top-left corner.
pub const BAND: (i32, i32) = (420, 64);
pub const AT: (i32, i32) = (160, 120);

#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let x11 = args.iter().any(|a| a == "--x11");
    let plain: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let seconds: u64 = plain.first().and_then(|a| a.parse().ok()).unwrap_or(5);
    let output = plain.get(1).map(|a| a.to_string());

    let code = if x11 || std::env::var_os("WAYLAND_DISPLAY").is_none() {
        x11::run(seconds, output)
    } else {
        wayland::run(seconds, output)
    };
    std::process::exit(code);
}
