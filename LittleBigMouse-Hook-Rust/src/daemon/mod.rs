//! Command dispatch — port of `LittleBigMouseDaemon`.
//!
//! Phase 0 handles the control commands and reports state (`Running`/`Stopped`/
//! `Paused`) without any real hooking: `Run`/`Stop` just flip a flag and announce
//! it. Phase 1 wires these to the actual hook install/uninstall on the pump
//! thread; Phase 2 gives `Load` a real layout to parse.

use std::sync::atomic::Ordering;

use crate::hook;
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
                // Ask the pump to install the hook. The `Running` state is
                // broadcast from the hook-install success path, not here
                // (C++ `Hook` -> `OnHooked` -> `SendState`).
                shared.paused.store(false, Ordering::SeqCst);
                hook::request_hook(shared);
            }
            Command::Stop => {
                // The `Stopped` state is broadcast from the unhook path.
                shared.paused.store(false, Ordering::SeqCst);
                hook::request_unhook(shared);
            }
            Command::State => {
                send_state(server, Some(client_id), shared);
            }
            Command::Load | Command::LoadFromFile(_) => {
                // Phase 2: parse the `ZonesLayout` into the arena.
            }
            Command::Quit => {
                // Post WM_QUIT so the pump unwinds and `main` returns cleanly.
                hook::request_quit(shared);
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
