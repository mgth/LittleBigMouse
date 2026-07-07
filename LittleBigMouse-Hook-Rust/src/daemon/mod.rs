//! Command dispatch — port of `LittleBigMouseDaemon`.
//!
//! Phase 0 handles the control commands and reports state (`Running`/`Stopped`/
//! `Paused`) without any real hooking: `Run`/`Stop` just flip a flag and announce
//! it. Phase 1 wires these to the actual hook install/uninstall on the pump
//! thread; Phase 2 gives `Load` a real layout to parse.

use std::sync::atomic::Ordering;

use crate::ipc::protocol::{self, Command};
use crate::ipc::server::{ClientId, ServerHandle};
use crate::shared::Shared;

/// Dispatch one received line.
///
/// Returns `true` if this client just became a listening client, so the reader
/// stops reading and leaves the socket open for event pushes.
pub fn receive_message(
    line: &str,
    client_id: ClientId,
    server: &ServerHandle,
    shared: &Shared,
) -> bool {
    // C++ `ReceiveClientMessage`: an empty message just re-reports state.
    if line.trim().is_empty() {
        send_state(server, Some(client_id), shared);
        return false;
    }

    let mut became_listening = false;

    for command in protocol::parse(line) {
        match command {
            Command::Listen => {
                server.set_listening(client_id);
                send_state(server, Some(client_id), shared);
                became_listening = true;
            }
            Command::Run => {
                // Phase 0 stub: no hook yet. Phase 1 installs the low-level hook
                // here and lets the `OnHooked` path broadcast the new state.
                shared.hooked.store(true, Ordering::SeqCst);
                shared.paused.store(false, Ordering::SeqCst);
                send_state(server, None, shared);
            }
            Command::Stop => {
                shared.hooked.store(false, Ordering::SeqCst);
                shared.paused.store(false, Ordering::SeqCst);
                send_state(server, None, shared);
            }
            Command::State => {
                send_state(server, Some(client_id), shared);
            }
            Command::Load | Command::LoadFromFile(_) => {
                // Phase 2: parse the `ZonesLayout` into the arena.
            }
            Command::Quit => {
                // Phase 1 posts WM_QUIT to the pump thread for a clean shutdown;
                // with no pump yet, exit the process directly.
                std::process::exit(0);
            }
            Command::Unknown(_) => {}
        }
    }

    became_listening
}

/// Report current state (C++ `SendState`): `Running` when hooked, else `Paused`
/// when paused, else `Stopped`. `to = Some(id)` replies to one client; `None`
/// broadcasts to all listening clients.
fn send_state(server: &ServerHandle, to: Option<ClientId>, shared: &Shared) {
    let msg = if shared.hooked.load(Ordering::SeqCst) {
        protocol::RUNNING
    } else if shared.paused.load(Ordering::SeqCst) {
        protocol::PAUSED
    } else {
        protocol::STOPPED
    };

    match to {
        Some(id) => server.send_to(id, msg),
        None => server.broadcast(msg),
    }
}
