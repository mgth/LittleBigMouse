//! `lbm-inject`: what the Windows service launches into the user's session to inject
//! a master's input where the hook cannot — the secure desktop, and the windows above
//! the user (docs/multi-pc-service-windows.md).
//!
//! It reads input batches on its standard input, the pipe the service hands it at
//! launch, and injects them with `SendInput`, following the input desktop. It opens
//! no file, no socket and no named object, and reads no configuration: run with the
//! service's rights, it must do nothing but inject. Launched by anyone else, it has
//! that someone's rights and nothing more.
//!
//! It ends with its input: what the stream left pressed is released first.

fn main() {
    std::process::exit(run());
}

#[cfg(windows)]
fn run() -> i32 {
    use littlebigmouse_hook::{platform, remote};

    // Per-monitor DPI awareness: the injector's positions are physical pixels.
    platform::init();
    let mut injector = remote::windows::SendInputInjector::following_input_desktop();
    // The main thread injects, and owns no window nor hook: it can follow the desktop.
    match remote::stream::pump(&mut std::io::stdin().lock(), &mut injector) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("[lbm-inject] input stream ended: {error}");
            1
        }
    }
}

#[cfg(not(windows))]
fn run() -> i32 {
    eprintln!("lbm-inject only runs on Windows: elsewhere the hook injects");
    2
}
