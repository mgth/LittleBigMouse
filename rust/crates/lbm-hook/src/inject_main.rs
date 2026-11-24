//! `lbm-inject`: what the Windows service launches into the user's session to inject
//! a master's input where the hook cannot — the secure desktop, and the windows above
//! the user (docs/multi-pc-service-windows.md).
//!
//! ```text
//! lbm-inject --pipe <name> --server <pid>     as the service launches it
//! lbm-inject                                  reading its standard input, by hand
//! ```
//!
//! It reads input batches and injects them with `SendInput`, following the input
//! desktop. The service is in session 0 and this process in the user's: handles are
//! not inherited across sessions, so the input is a named pipe the service made for
//! this launch, under a name nobody can guess, open to SYSTEM alone, and served by the
//! process `--server` names, which is checked before anything is read. It opens
//! nothing else and reads no configuration: run with the service's rights, it must do
//! nothing but inject. Launched by anyone else, it has that someone's rights.
//!
//! It ends with its input: what the stream left pressed is released first.

fn main() {
    std::process::exit(run());
}

#[cfg(windows)]
fn run() -> i32 {
    use littlebigmouse_hook::{platform, remote};

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut input: Box<dyn std::io::Read> = match &arguments[..] {
        [] => Box::new(std::io::stdin().lock()),
        [pipe, name, server, pid] if pipe == "--pipe" && server == "--server" => {
            let Ok(pid) = pid.parse() else {
                eprintln!("[lbm-inject] --server takes a process id");
                return 2;
            };
            match remote::windows::open_service_pipe(name, pid) {
                Ok(pipe) => Box::new(pipe),
                Err(error) => {
                    eprintln!("[lbm-inject] {error}");
                    return 1;
                }
            }
        }
        _ => {
            eprintln!("usage: lbm-inject [--pipe <name> --server <pid>]");
            return 2;
        }
    };

    // Per-monitor DPI awareness: the injector's positions are physical pixels.
    platform::init();
    let mut injector = remote::windows::SendInputInjector::following_input_desktop();
    // The main thread injects, and owns no window nor hook: it can follow the desktop.
    match remote::stream::pump(&mut input, &mut injector) {
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
