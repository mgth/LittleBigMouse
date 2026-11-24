//! A remote's input endpoint that injects nothing: every event a master sends is
//! printed, with the time, instead of reaching this machine's pointer and keyboard.
//! For trying a master (docs/multi-pc.md) against a remote whose user is at it.
//!
//! ```text
//! LBM_HOOK_ENDPOINT=/run/user/1000/x/hook.sock lbm-agent --fake-hook --config-dir … &
//! cargo run -p lbm-hook --example input_sink -- /run/user/1000/x/hook.sock
//! ```
//!
//! The agent's `--fake-hook` answers the command endpoint; this serves the input
//! endpoint beside it (`lbm_ipc::input::input_endpoint_beside`), which the fake does
//! not. It stops on Ctrl+C.

use std::sync::Arc;
use std::time::Instant;

use littlebigmouse_hook::ipc::input::{start, Outlet};
use littlebigmouse_hook::remote::{Injection, Injector};

struct Printer(Instant);

impl Printer {
    fn say(&self, what: std::fmt::Arguments) {
        println!("{:>9.3} {what}", self.0.elapsed().as_secs_f64());
    }
}

impl Injector for Printer {
    fn inject(&mut self, batch: &[Injection]) {
        for injection in batch {
            self.say(format_args!("{injection:?}"));
        }
    }

    fn park(&mut self) {
        self.say(format_args!("park"));
    }

    fn unpark(&mut self) {
        self.say(format_args!("unpark"));
    }
}

fn main() {
    let Some(command) = std::env::args().nth(1) else {
        eprintln!("usage: input_sink COMMAND_ENDPOINT");
        std::process::exit(2);
    };
    let endpoint = lbm_ipc::input::input_endpoint_beside(&command);
    let server = start(
        endpoint,
        Box::new(Printer(Instant::now())),
        Arc::new(Outlet::default()),
    )
    .unwrap_or_else(|e| panic!("no input endpoint: {e}"));
    eprintln!("input sink on {}", server.endpoint());
    loop {
        std::thread::park();
    }
}
