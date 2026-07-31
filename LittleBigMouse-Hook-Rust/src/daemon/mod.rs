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

    let commands = protocol::parse(line);
    let rehooks = frame_rehooks(&commands);

    for command in commands {
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
                hook::request_unhook(shared);
                shared.paused.store(false, Ordering::SeqCst);
            }
            Command::State => {
                send_state(server, Some(client_id), shared);
            }
            Command::Load(xml) => {
                // Report the outcome to every listening client: a Load-without-Run
                // (virtual-layout inspection) has no later Running event to prove
                // the zones were accepted.
                match load_layout(shared, &xml, rehooks) {
                    Some(info) => {
                        server.broadcast(&protocol::loaded(
                            info.zones,
                            info.main,
                            info.virtual_layout,
                        ));
                        // A virtual layout is loaded to be INSPECTED: probe it right
                        // away so the UI gets the edge report without a round-trip.
                        if info.virtual_layout {
                            probe_loaded(shared, server);
                        }
                    }
                    None => server.broadcast(protocol::LOAD_FAILED),
                }
            }
            Command::Probe => probe_loaded(shared, server),
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

/// Sweep the last loaded layout with the edge prober and broadcast the report.
/// Runs on a re-parsed private layout, so the live engine and its lock are
/// never touched (the hook thread cannot be stalled by a probe).
fn probe_loaded(shared: &Shared, server: &ServerHandle) {
    let xml = shared
        .last_layout_xml
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    match crate::engine::probe::probe_xml(&xml) {
        Some(report) => {
            eprintln!("[LittleBigMouse.Hook] probe: report {} bytes", report.len());
            server.broadcast(&protocol::probed(&report));
        }
        None => eprintln!("[LittleBigMouse.Hook] probe: no layout to probe"),
    }
}

/// What a successful `Load` accepted — echoed back to the UI in the `Loaded` event.
pub struct LoadInfo {
    pub zones: usize,
    pub main: usize,
    pub virtual_layout: bool,
}

/// Does this frame put the hook straight back up after its `Load`?
///
/// Every Apply sends `Load`+`Run`, and so does every tick of the UI's live preview.
/// Taking the hook down in between is pure loss: `do_unhook` tears down the mouse,
/// focus, desktop and display hooks and destroys the display window, `do_hook` builds
/// all of it again, and the pair broadcasts `Stopped` then `Running` — which the UI
/// shows as the tray icon blinking off and on. Once per Apply that is merely wasteful;
/// several times a second under live preview it is visible.
fn frame_rehooks(commands: &[Command]) -> bool {
    commands.iter().any(|command| matches!(command, Command::Run))
}

/// C++ `LittleBigMouseDaemon::ReceiveLoadMessage`: stop hooking, parse the
/// layout into the engine, and adopt its priorities for the next hook.
///
/// `keep_hooked` is for the `Load`+`Run` frames described on
/// [`frame_rehooks`]: the engine swap itself is safe under a live hook (the
/// callback takes the same lock), the teardown was only ever there because the
/// hook had no reason to stay up across a layout it no longer knew.
fn load_layout(shared: &Shared, xml: &str, keep_hooked: bool) -> Option<LoadInfo> {
    if shared.hooked.load(Ordering::SeqCst) && !keep_hooked {
        shared.unhook_requests.fetch_add(1, Ordering::SeqCst);
        hook::request_unhook(shared);
    }
    if let Some(layout) = ZonesLayout::from_xml(xml) {
        let info = LoadInfo {
            zones: layout.zones.len(),
            main: layout.main_zones.len(),
            virtual_layout: layout.virtual_layout,
        };
        let tag = if info.virtual_layout { " VIRTUAL" } else { "" };
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
        // The one thing the teardown used to guarantee: a cursor confined by the
        // outgoing geometry is never left confined by it. The engine re-clips on the
        // next move if the new layout still calls for it.
        if keep_hooked {
            crate::platform::cursor::release_clip();
        }
        // Kept for the edge prober, which re-parses rather than touching the
        // live engine.
        *shared
            .last_layout_xml
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = xml.to_string();
        eprintln!(
            "[LittleBigMouse.Hook] layout loaded: {} zones ({} main){tag}",
            info.zones, info.main
        );
        Some(info)
    } else {
        eprintln!("[LittleBigMouse.Hook] layout load FAILED to parse");
        None
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
    // A virtual (foreign) layout is loaded for inspection only: hooking it would
    // confine the local mouse inside a geometry that does not exist on this
    // machine. The refusal lives daemon-side, keyed on the wire flag, so no UI
    // path — present or future — can capture the mouse with a client's layout.
    let virtual_layout = shared
        .engine
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .layout
        .virtual_layout;
    if virtual_layout {
        eprintln!("[LittleBigMouse.Hook] Run refused: the loaded layout is virtual (inspection only)");
        return;
    }

    load_excluded(shared);
    if !shared.paused.load(Ordering::SeqCst) {
        hook::request_hook(shared);
    }
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
    // A file that parses into a layout is authoritative even if it holds no
    // Run line (a stopped state is a valid persisted state — falling back to
    // the .bak then would auto-start an older layout). The backup only covers
    // unreadable or corrupt primaries, written atomically by the UI.
    let primary_ok = std::fs::read_to_string(path)
        .map(|content| replay(shared, &content))
        .unwrap_or(false);
    if primary_ok {
        return;
    }

    let backup = format!("{path}.bak");
    match std::fs::read_to_string(&backup) {
        Ok(content) if replay(shared, &content) => {
            eprintln!("[LittleBigMouse.Hook] recovered startup configuration from {backup}")
        }
        Ok(_) => eprintln!("[LittleBigMouse.Hook] startup configuration and backup are invalid"),
        Err(error) => eprintln!(
            "[LittleBigMouse.Hook] cannot recover startup configuration from {path} or {backup}: {error}"
        ),
    }
}

/// Replay the `Load`/`Run` command lines from a serialized layout file. Runs
/// without a socket client, so it only handles the commands the file contains.
/// Returns whether a layout was successfully loaded; `Run` is only honoured
/// after a successful `Load` (a Run alone must not hook a stale engine).
fn replay(shared: &Shared, content: &str) -> bool {
    let mut loaded = false;
    for line in content.lines() {
        for command in protocol::parse(line) {
            match command {
                // One command per line here, so a Run is never in sight when the Load
                // is handled. It costs nothing: this is the startup path, where there
                // is no hook up to take down.
                Command::Load(xml) => loaded = load_layout(shared, &xml, false).is_some(),
                Command::Run if loaded => run(shared),
                _ => {}
            }
        }
    }
    loaded
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

    // Same layout flagged as virtual: the daemon must accept the Load (so the
    // engine can be inspected) but refuse the Run that follows.
    const VIRTUAL_LOAD_LINE: &str = concat!(
        r#"<CommandMessage Command="Load"><Payload>"#,
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200" Virtual="True"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout></Payload></CommandMessage>"#,
    );

    #[test]
    fn virtual_layout_loads_but_run_never_hooks() {
        let shared = Shared::new();
        let content = format!("{VIRTUAL_LOAD_LINE}\n<CommandMessage Command=\"Run\" Payload=\"\"/>\n");
        replay(&shared, &content);

        // The layout IS loaded (inspection works)...
        let engine = shared.engine.lock().unwrap();
        assert_eq!(engine.layout.zones.len(), 1);
        assert!(engine.layout.virtual_layout, "the wire flag must be parsed");
        drop(engine);

        // ...but the hook must never be requested for it.
        assert!(
            !shared.want_hook.load(Ordering::SeqCst),
            "Run must be refused on a virtual layout"
        );
    }

    // The payload alone, as `load_layout` receives it once the CommandMessage
    // envelope is off.
    const ZONES_XML: &str = concat!(
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    #[test]
    fn a_frame_is_recognized_as_rehooking_by_its_run() {
        // The shape the UI actually sends for an Apply, and for every live-preview
        // tick: both commands in one frame.
        let frame = format!(
            "<Messages><CommandMessage Command=\"Load\"><Payload>{ZONES_XML}</Payload></CommandMessage><CommandMessage Command=\"Run\" Payload=\"\"/></Messages>"
        );
        assert!(frame_rehooks(&protocol::parse(&frame)));

        let load_only = format!(
            "<CommandMessage Command=\"Load\"><Payload>{ZONES_XML}</Payload></CommandMessage>"
        );
        assert!(
            !frame_rehooks(&protocol::parse(&load_only)),
            "a Load on its own — the virtual-layout inspection path — has nothing putting the hook back"
        );
    }

    #[test]
    fn a_load_a_run_follows_never_takes_the_hook_down() {
        let shared = Shared::new();
        shared.hooked.store(true, Ordering::SeqCst);
        shared.want_hook.store(true, Ordering::SeqCst);

        assert!(load_layout(&shared, ZONES_XML, true).is_some());

        assert_eq!(
            shared.unhook_requests.load(Ordering::SeqCst),
            0,
            "tearing the hooks down to put them straight back up is the whole cost being avoided"
        );
        assert!(shared.want_hook.load(Ordering::SeqCst));
        assert_eq!(shared.engine.lock().unwrap().layout.zones.len(), 1);
    }

    #[test]
    fn a_load_on_its_own_still_takes_the_hook_down() {
        // Nothing is putting it back, so leaving it up would hold the cursor to a
        // geometry the engine no longer knows.
        let shared = Shared::new();
        shared.hooked.store(true, Ordering::SeqCst);
        shared.want_hook.store(true, Ordering::SeqCst);

        assert!(load_layout(&shared, ZONES_XML, false).is_some());

        assert_eq!(shared.unhook_requests.load(Ordering::SeqCst), 1);
        assert!(!shared.want_hook.load(Ordering::SeqCst));
    }

    #[test]
    fn run_without_a_successful_load_is_ignored() {
        let shared = Shared::new();
        replay(&shared, "<CommandMessage Command=\"Run\" Payload=\"\"/>\n");
        assert!(
            !shared.want_hook.load(Ordering::SeqCst),
            "a Run alone must not hook an engine with no layout"
        );
    }

    #[test]
    fn corrupt_primary_recovers_last_good_backup() {
        let shared = Shared::new();
        let id = format!("{}-{:?}", std::process::id(), std::thread::current().id());
        let path = std::env::temp_dir().join(format!("lbm-current-{id}.xml"));
        let backup = format!("{}.bak", path.display());
        std::fs::write(&path, "<truncated").unwrap();
        std::fs::write(
            &backup,
            format!("{LOAD_LINE}\n<CommandMessage Command=\"Run\" Payload=\"\"/>\n"),
        )
        .unwrap();

        load_from_file(&shared, path.to_str().unwrap());

        assert_eq!(shared.engine.lock().unwrap().layout.zones.len(), 1);
        assert!(shared.want_hook.load(Ordering::SeqCst));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
    }

    #[test]
    fn valid_stopped_primary_does_not_fall_back_to_backup() {
        // A primary holding only a Load (user stopped, state persisted) is
        // authoritative: the backup must not auto-start an older layout.
        let shared = Shared::new();
        let id = format!("{}-{:?}", std::process::id(), std::thread::current().id());
        let path = std::env::temp_dir().join(format!("lbm-stopped-{id}.xml"));
        let backup = format!("{}.bak", path.display());
        std::fs::write(&path, format!("{LOAD_LINE}\n")).unwrap();
        std::fs::write(
            &backup,
            format!("{LOAD_LINE}\n<CommandMessage Command=\"Run\" Payload=\"\"/>\n"),
        )
        .unwrap();

        load_from_file(&shared, path.to_str().unwrap());

        assert_eq!(shared.engine.lock().unwrap().layout.zones.len(), 1);
        assert!(
            !shared.want_hook.load(Ordering::SeqCst),
            "backup must not override a valid stopped state"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
    }
}
