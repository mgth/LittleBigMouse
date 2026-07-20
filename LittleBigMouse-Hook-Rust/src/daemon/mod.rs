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
            Command::Run => run(shared),
            Command::Stop => {
                // C++ Stop: unhook and clear the pause flag. `Stopped` is
                // broadcast from the unhook path.
                shared.paused.store(false, Ordering::SeqCst);
                hook::set_enabled(shared, false);
            }
            Command::State => {
                send_state(server, Some(client_id), shared);
            }
            Command::Load(xml) => {
                load_layout(shared, &xml);
            }
            Command::LoadFromFile(path) => {
                load_from_file(shared, &path);
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
    hook::set_enabled(shared, false);
    if let Some(layout) = ZonesLayout::from_xml(xml) {
        let (zones, main) = (layout.zones.len(), layout.main_zones.len());
        shared
            .priority
            .store(layout.priority.as_u8(), Ordering::SeqCst);
        shared
            .priority_unhooked
            .store(layout.priority_unhooked.as_u8(), Ordering::SeqCst);
        // Recover from a poisoned lock (a prior panic under the lock): a fresh Load fully replaces
        // the layout and resets tracking, so it is exactly the right place to shrug off the poison —
        // this is what lets a Stop/Start (Load) heal crossing instead of staying broken.
        shared
            .engine
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .load(layout);
        eprintln!("[LittleBigMouse.Hook] layout loaded: {zones} zones ({main} main)");
    } else {
        eprintln!("[LittleBigMouse.Hook] layout load FAILED to parse");
    }
}

/// C++ `Run` handling: load the exclusion list and install the hook, unless
/// paused by an excluded foreground app.
///
/// `want_hook` is desired state — always express it. The C++ early-returned
/// while `hooked` was still true, but a preceding Load just requested an ASYNC
/// unhook, so Load+Run over a running engine raced into a stopped one: the UI
/// play button "applied" an options change by killing the engine, and only a
/// second click restarted it. Re-asserting the flag makes the swap seamless
/// (the router never observes the transient false) or at worst a quick
/// re-arm — both correct.
fn run(shared: &Shared) {
    load_excluded(shared);
    let foreground = crate::platform::process::foreground_process_path();
    let excluded = foreground
        .as_deref()
        .is_some_and(|path| shared.is_excluded(path));
    shared.paused.store(excluded, Ordering::SeqCst);
    hook::set_enabled(shared, true);
}

/// C++ `LoadExcluded`: read `Excluded.txt`, skipping blank lines and `:` comments.
pub fn load_excluded(shared: &Shared) {
    let mut list = Vec::new();
    if let Some(path) = crate::platform::paths::lbm_data_file("Excluded.txt") {
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                list.push(line.to_string());
            }
        }
    }
    if let Ok(mut excluded) = shared.excluded.lock() {
        *excluded = list;
    }
}

/// C++ `LoadFromFile`: read `Current.xml` and replay its command lines
/// (`Load` then `Run`) — the standalone/autostart path.
pub fn load_from_file(shared: &Shared, path: &str) {
    match std::fs::read_to_string(path) {
        Ok(content) => replay(shared, &content),
        Err(e) => eprintln!("[LittleBigMouse.Hook] standalone: cannot read {path}: {e}"),
    }
}

/// Replay the `Load`/`Run` command lines from a serialized layout file. Runs
/// without a socket client, so it only handles the commands the file contains.
fn replay(shared: &Shared, content: &str) {
    for line in content.lines() {
        for command in protocol::parse(line) {
            match command {
                Command::Load(xml) => load_layout(shared, &xml),
                Command::Run => run(shared),
                _ => {}
            }
        }
    }
}

/// Report current state (C++ `SendState`): `Running` when hooked, else `Paused`
/// when paused, else `Stopped`. `to = Some(id)` replies to one client; `None`
/// broadcasts to all listening clients.
fn send_state(server: &ServerHandle, to: Option<ClientId>, shared: &Shared) {
    let msg = state_message(shared);

    match to {
        Some(id) => server.send_to(id, msg),
        None => server.broadcast(msg),
    }
}

fn state_message(shared: &Shared) -> &'static str {
    if shared.hooked.load(Ordering::SeqCst) {
        protocol::RUNNING
    } else if shared.enabled.load(Ordering::SeqCst) && shared.paused.load(Ordering::SeqCst) {
        protocol::PAUSED
    } else {
        protocol::STOPPED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A serialized Load command (as the UI writes to Current.xml) plus a Run,
    // exactly the two lines the standalone path replays.
    const LOAD_LINE: &str = concat!(
        r#"<CommandMessage Command="Load"><Payload>"#,
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout></Payload></CommandMessage>"#,
    );

    #[test]
    fn replay_loads_layout_and_requests_hook() {
        let shared = Shared::new();
        let content = format!("{LOAD_LINE}\n<CommandMessage Command=\"Run\" Payload=\"\"/>\n");
        replay(&shared, &content);

        // Load populated the engine's layout...
        assert_eq!(shared.engine.lock().unwrap().layout.zones.len(), 1);
        // ...and Run requested hooking (pump_tid is 0 in the test, so the posted
        // WM_BREAK_LOOP is a no-op, but the desired state is set).
        assert!(shared.want_hook.load(Ordering::SeqCst));
    }

    #[test]
    fn load_run_over_a_hooked_engine_still_requests_hook() {
        // Load requests an ASYNC unhook; with the old `hooked` early-return in
        // run(), the Run right behind it was a silent no-op (the hook thread
        // had not processed the unhook yet) and the engine ended up stopped —
        // the UI play button needed a second click to apply an options change.
        let shared = Shared::new();
        shared.hooked.store(true, Ordering::SeqCst);
        let content = format!("{LOAD_LINE}\n<CommandMessage Command=\"Run\" Payload=\"\"/>\n");
        replay(&shared, &content);

        assert!(
            shared.want_hook.load(Ordering::SeqCst),
            "Run must express the desired state even while the previous hook is still up"
        );
    }

    #[test]
    fn disabled_engine_reports_stopped_even_if_foreground_is_excluded() {
        let shared = Shared::new();
        shared.paused.store(true, Ordering::SeqCst);
        assert_eq!(state_message(&shared), protocol::STOPPED);

        shared.enabled.store(true, Ordering::SeqCst);
        assert_eq!(state_message(&shared), protocol::PAUSED);
    }
}
