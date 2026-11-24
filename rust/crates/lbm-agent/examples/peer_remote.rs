//! A remote's multi-PC node and nothing else of an agent, for trying what a master asks of
//! a remote — the rulers on its screens — on a machine whose own agent runs.
//!
//! ```text
//! cargo run -p lbm-agent --example peer_remote -- CONFIG_DIR PORT [SECONDS]
//! ```
//!
//! It runs the agent's own multi-PC manager (`lbm_agent::peers::Service`) with the mode
//! on at `PORT`, the pairings of `CONFIG_DIR` (`peers.json`: pair the master there
//! beforehand: nobody is asked here) and this machine's
//! screens as the agent reads them. Rulers a master asks for are drawn for real. It has
//! no hook: what a master sends of the pointer and the keyboard goes nowhere. It holds
//! no instance lock and no endpoint of the agent's, so the user's own agent and hook can
//! go on running beside it; give it a port they do not use.
//!
//! It prints what the manager says and stops on Ctrl+C or after `SECONDS` (ten minutes
//! by default), saying `Bye` to its master.

use std::path::PathBuf;
use std::process::ExitCode;

use lbm_agent::discovery::Discovery;
use lbm_agent::peers::{Answer, Notice, PeerRequest, Service, Setup};

#[cfg(target_os = "linux")]
fn overlay_host() {
    // The rulers' surfaces live in this same binary run as a child (lbm_overlay::linux).
    if std::env::args().nth(1).as_deref() == Some("--screen-overlay-host") {
        std::process::exit(lbm_overlay::linux::run_host());
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    overlay_host();
    let mut args = std::env::args().skip(1);
    let (Some(directory), Some(port)) = (args.next(), args.next().and_then(|p| p.parse().ok()))
    else {
        eprintln!("usage: peer_remote CONFIG_DIR PORT [SECONDS]");
        return ExitCode::FAILURE;
    };
    let lifetime =
        std::time::Duration::from_secs(args.next().and_then(|s| s.parse().ok()).unwrap_or(600));
    // The screens in physical pixels, as the agent reads them (see its `main`).
    #[cfg(windows)]
    lbm_display::windows::set_process_per_monitor_dpi_aware();
    let discovery = Discovery::detect();

    let mut service = Service::new(PathBuf::from(directory), Setup::new(None));
    let mut notices = service.take_notices().expect("the notices, once");
    service.local_displays(discovery.peer_displays());
    let on = service.request(PeerRequest::Set {
        enabled: true,
        port: Some(port),
        name: None,
        pointer_lock_shortcut: None,
        clipboard: Default::default(),
    });
    let state = match on {
        Answer::Now(result) => result,
        Answer::Later(result) => result.await,
    };
    match state {
        Ok(state) => println!("on: {state}"),
        Err(error) => {
            eprintln!("peer_remote: {error}");
            return ExitCode::FAILURE;
        }
    }
    let deadline = tokio::time::sleep(lifetime);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            notice = notices.recv() => match notice {
                Some(Notice::Peers(state)) => {
                    for peer in &state.peers {
                        println!(
                            "peer {} ({}) {:?} {:?} connected={} overlay_error={:?}",
                            peer.peer.name,
                            peer.peer.machine_id,
                            peer.role,
                            peer.state,
                            peer.peer.connected,
                            peer.overlay_error,
                        );
                    }
                }
                Some(other) => println!("{other:?}"),
                None => break,
            },
            _ = &mut deadline => break,
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    service.stop().await;
    println!("stopped");
    ExitCode::SUCCESS
}
