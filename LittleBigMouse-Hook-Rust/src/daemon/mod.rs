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
use crate::zones::ZonesLayout;

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
            Command::Load(xml) => {
                load_layout(shared, &xml);
            }
            Command::LoadFromFile(_) => {
                // Phase 4: read the file under %ProgramData% and replay its lines.
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

/// C++ `LittleBigMouseDaemon::ReceiveLoadMessage`: stop hooking, parse the
/// layout into the engine, and adopt its priorities for the next hook.
fn load_layout(shared: &Shared, xml: &str) {
    if shared.hooked.load(Ordering::SeqCst) {
        hook::request_unhook(shared);
    }
    if let Some(layout) = ZonesLayout::from_xml(xml) {
        let (zones, main) = (layout.zones.len(), layout.main_zones.len());
        shared
            .priority
            .store(layout.priority.as_u8(), Ordering::SeqCst);
        shared
            .priority_unhooked
            .store(layout.priority_unhooked.as_u8(), Ordering::SeqCst);
        if let Ok(mut engine) = shared.engine.lock() {
            engine.load(layout);
        }
        eprintln!("[LittleBigMouse.Hook] layout loaded: {zones} zones ({main} main)");
    } else {
        eprintln!("[LittleBigMouse.Hook] layout load FAILED to parse");
    }
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
