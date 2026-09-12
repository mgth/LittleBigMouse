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
use lbm_ipc::protocol::Event;

/// Dispatch one received line.
pub fn receive_message(line: &str, client_id: ClientId, server: &ServerHandle, shared: &Shared) {
    // C++ `ReceiveClientMessage`: an empty message just re-reports state.
    if line.trim().is_empty() {
        send_state(server, Some(client_id), shared);
        return;
    }

    let commands = protocol::parse(line);
    let rehooks = frame_rehooks(&commands);

    for command in commands {
        match command {
            // The handshake: who is there, and what it speaks. Answered whether or
            // not the versions agree — the agent decides what to do about a mismatch,
            // and it cannot decide on silence.
            Command::Hello { .. } => {
                server.send_to(
                    client_id,
                    &Event::Hello {
                        protocol: lbm_ipc::protocol::PROTOCOL,
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                        layout: shared
                            .applied
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone(),
                    },
                );
            }
            Command::Listen => {
                server.set_listening(client_id);
                send_state(server, Some(client_id), shared);
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
            Command::Load {
                zones: xml,
                desktop,
            } => {
                // Before the layout: a Load that fails to parse still describes the
                // machine correctly, and the absolute device is rebuilt from this
                // whether or not the zones were taken.
                if let Some(d) = desktop {
                    *shared.desktop.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(crate::geometry::Rect::new(d.left, d.top, d.width, d.height));
                }
                // Report the outcome to every listening client: a Load-without-Run
                // (virtual-layout inspection) has no later Running event to prove
                // the zones were accepted.
                match load_layout(shared, &xml, rehooks) {
                    Some(info) => {
                        server.broadcast(&Event::Loaded {
                            zones: info.zones,
                            main: info.main,
                            virtual_layout: info.virtual_layout,
                        });
                    }
                    None => server.broadcast(&Event::LoadFailed),
                }
            }
            // Always reconciles, even when the text is unchanged: the answer is what
            // the user is waiting for, and a re-registration is cheap.
            Command::Shortcut { text } => {
                adopt_rescue_shortcut(shared, &text);
                hook::rescue_shortcut_changed(shared);
            }
            // The list the hook stands aside for. Adopted whole: emptying it is a thing
            // a user does, and it must not read as "nothing to say".
            Command::Excluded { processes } => {
                *shared.excluded.lock().unwrap_or_else(|p| p.into_inner()) = processes;
            }
            // Adopted at once, and never from the layout: a hook a new agent has just
            // taken over is told afresh what that agent wants of it.
            Command::BindToAgent { bound } => {
                shared.bound_to_agent.store(bound, Ordering::SeqCst);
            }
            Command::Quit => {
                // Post WM_QUIT so the pump unwinds and `main` returns cleanly.
                hook::request_quit(shared);
            }
            // A newer agent's command, and the handshake said we speak different
            // protocols: nothing to do but let the rest of the frame through.
            Command::Unknown => {}
        }
    }
}

/// The panic shortcut fired. Runs on the listener's own thread.
///
/// One thing, always: free the cursor and take the engine down. That is the whole of
/// what can be decided without knowing anything — and knowing nothing is the point,
/// because this runs when the UI may be unreachable behind the very cursor it is
/// freeing, or not running at all.
///
/// What it should *mean* is the UI's to decide, and the UI is where the knowledge
/// is. Previewing a layout? Then there is an experiment to throw away: it reloads
/// the saved one and starts again. Not previewing? Then the layout that trapped the
/// user is the one they committed to, and staying stopped is the answer — so the
/// ordinary case needs no second press.
///
/// `Rescued` is broadcast before the unhook is requested, so a listening UI always
/// sees it ahead of the `Stopped` the unhook produces. The other order would have
/// the UI drop live preview on the stop and then find nothing left to act on.
pub fn rescue_fired(shared: &'static Shared) {
    eprintln!("[LittleBigMouse.Hook] rescue: freeing the cursor and stopping");
    crate::platform::cursor::force_release_clip();
    shared.broadcast(&Event::Rescued);
    hook::request_unhook(shared);
    shared.paused.store(false, Ordering::SeqCst);
}

/// Adopt the panic shortcut a freshly loaded layout names, and tell the listener.
/// It travels with the layout like every other daemon-side setting, so it arrives
/// with the startup `LoadFromFile` too.
fn adopt_rescue_shortcut(shared: &Shared, wanted: &str) {
    let wanted = if wanted.trim().is_empty() {
        crate::shortcut::DEFAULT
    } else {
        wanted
    };
    let changed = {
        let mut current = shared
            .rescue_shortcut
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if *current == wanted {
            false
        } else {
            *current = wanted.to_string();
            true
        }
    };
    if changed {
        hook::rescue_shortcut_changed(shared);
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
    commands
        .iter()
        .any(|command| matches!(command, Command::Run))
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
        // Before the move: the layout is about to be handed to the engine.
        adopt_rescue_shortcut(shared, &layout.rescue_shortcut);
        {
            let mut engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
            // A Load+Run keeps the hook alive. Restore the outgoing engine's clip
            // BEFORE replacing its tracking state, but only when that exact LBM
            // clip is still active. A game-owned replacement must survive reload.
            if keep_hooked {
                crate::platform::cursor::restore_managed_clip(&mut engine);
            }
            engine.load(layout);
        }
        // After the engine has it: what this hook applies is what it accepted, and a
        // Load that failed to parse leaves the previous layout — and its fingerprint.
        *shared.applied.lock().unwrap_or_else(|p| p.into_inner()) =
            lbm_ipc::protocol::fingerprint(xml);
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
/// A `Run` this daemon will not honour, said out loud.
///
/// The refusal itself is the point of these guards; the saying is what was missing.
/// A client that asked for the engine and hears nothing waits for a `Running` that
/// will never come — the agent has no deadline on a Start and nothing retries it —
/// so the user who pressed Start is left looking at "stopped" with no reason given.
/// The state is the answer rather than a refusal of its own: `Paused` is exactly what
/// "an excluded application has the foreground" means, and the frontends already know
/// how to show it. (A dedicated `RunRefused` is #609's, for the geometric refusal;
/// two spellings of the same idea would be worse than one.)
fn refuse(shared: &Shared, why: &str) {
    eprintln!("[LittleBigMouse.Hook] Run refused: {why}");
    shared.broadcast(state(shared));
}

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
        refuse(shared, "the loaded layout is virtual (inspection only)");
        return;
    }

    // Nothing loaded: hooking here would grab the mice for an engine with no zones
    // to route between. The file path refused a Run that no Load had preceded; the
    // socket path never did, and it is the only one left.
    if shared
        .engine
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .layout
        .zones
        .is_empty()
    {
        refuse(shared, "no layout is loaded");
        return;
    }

    // Ask who is in front rather than wait to be told. `Run` is the one moment the
    // daemon decides to hook — reading a pause flag that only a focus *change* ever
    // sets is what let the engine hook straight over an excluded game that was already
    // running (#541). This question stays here, and nowhere else: it is the only place
    // that can ask at the instant the grab happens. The list it is answered against
    // comes from the agent (`Command::Excluded`), which owns what the user edits.
    if hook::adopt_foreground(shared) {
        eprintln!("[LittleBigMouse.Hook] Run refused: an excluded application has the foreground");
        // An excluded app holds the foreground: do not hook over it, and let go
        // if an earlier Run already did.
        if shared.hooked.load(Ordering::SeqCst) {
            // The unhook announces itself once the pump has done it. Saying anything
            // here would say `Running` — which is precisely what is being undone.
            hook::request_unhook(shared);
        } else {
            shared.broadcast(state(shared));
        }
        return;
    }

    hook::request_hook(shared);
}

/// What this daemon is doing, as one event: `Running` when hooked, else `Paused`
/// when paused, else `Stopped`.
fn state(shared: &Shared) -> &'static Event {
    if shared.hooked.load(Ordering::SeqCst) {
        &Event::Running
    } else if shared.paused.load(Ordering::SeqCst) {
        &Event::Paused
    } else {
        &Event::Stopped
    }
}

/// Report current state (C++ `SendState`). `to = Some(id)` replies to one client;
/// `None` broadcasts to the listening client.
fn send_state(server: &ServerHandle, to: Option<ClientId>, shared: &Shared) {
    let msg = state(shared);

    match to {
        Some(id) => server.send_to(id, msg),
        None => server.broadcast(msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The payload alone, as `load_layout` receives it once the CommandMessage
    // envelope is off.
    const ZONES_XML: &str = concat!(
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    /// A Load then a Run, as the agent sends them.
    fn load_and_run(shared: &Shared, layout: &str) -> bool {
        let loaded = load_layout(shared, layout, false).is_some();
        if loaded {
            run(shared);
        }
        loaded
    }

    #[test]
    fn a_load_then_a_run_requests_the_hook() {
        let shared = Shared::new();
        assert!(load_and_run(&shared, ZONES_XML));

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
        load_and_run(&shared, ZONES_XML);

        assert!(
            shared.want_hook.load(Ordering::SeqCst),
            "Run must express the desired state even while the previous hook is still up"
        );
    }

    // The same layout flagged as virtual: the daemon must accept the Load (so the
    // engine can be inspected) but refuse the Run that follows.
    const VIRTUAL_LAYOUT: &str = concat!(
        r#"<ZonesLayout Algorithm="Strait" MaxTravelDistance="200" Virtual="True"><MainZones>"#,
        r#"<Zone Id="0" Name="A"><PixelsBounds><Rect Left="0" Top="0" Width="1920" Height="1080"></Rect></PixelsBounds><PhysicalBounds><Rect Left="0" Top="0" Width="500" Height="280"></Rect></PhysicalBounds></Zone>"#,
        r#"</MainZones></ZonesLayout>"#,
    );

    #[test]
    fn virtual_layout_loads_but_run_never_hooks() {
        let shared = Shared::new();
        load_and_run(&shared, VIRTUAL_LAYOUT);

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

    #[test]
    fn a_frame_is_recognized_as_rehooking_by_its_run() {
        // The shape the agent actually sends for an Apply, and for every live-preview
        // tick: both commands in one frame.
        let load = Command::Load {
            zones: ZONES_XML.to_owned(),
            desktop: None,
        };
        assert!(frame_rehooks(&protocol::parse(&protocol::frame(&[
            load.clone(),
            Command::Run
        ]))));

        assert!(
            !frame_rehooks(&protocol::parse(&protocol::frame(&[load]))),
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
    fn a_layout_carries_the_rescue_shortcut() {
        // It travels with the layout like every other daemon-side setting: the hook
        // is told nothing else about it.
        let shared = Shared::new();
        let layout = ZONES_XML.replace(
            r#"<ZonesLayout Algorithm="Strait""#,
            r#"<ZonesLayout RescueShortcut="Ctrl+Alt+F9" Algorithm="Strait""#,
        );
        load_layout(&shared, &layout, false);

        assert_eq!(
            *shared.rescue_shortcut.lock().unwrap(),
            "Ctrl+Alt+F9",
            "the daemon must adopt what the layout names"
        );
    }

    #[test]
    fn a_layout_naming_no_shortcut_keeps_the_default() {
        // Layouts written by an older UI have no such attribute; the rescue must
        // still exist for them.
        let shared = Shared::new();
        load_layout(&shared, ZONES_XML, false);

        assert_eq!(
            *shared.rescue_shortcut.lock().unwrap(),
            crate::shortcut::DEFAULT
        );
    }

    #[test]
    fn a_run_with_nothing_loaded_never_grabs_the_mice() {
        let shared = Shared::new();

        run(&shared);

        assert!(
            !shared.want_hook.load(Ordering::SeqCst),
            "a Run alone must not hook an engine with no layout"
        );
    }

    //==================//
    // The rescue       //
    //==================//

    /// The panic shortcut, which until now nothing exercised at all.
    ///
    /// It is the path with the least margin for a regression: it runs when the cursor
    /// is trapped and nothing can be clicked, so a listener that stops hearing it has
    /// no other way to find out, and neither does the user.
    #[tokio::test]
    async fn the_rescue_frees_the_cursor_and_says_it_did() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;

        // As it is when a layout has trapped the user: hooked, and paused because the
        // thing in front happened to be excluded.
        shared.want_hook.store(true, Ordering::SeqCst);
        shared.paused.store(true, Ordering::SeqCst);

        rescue_fired(shared);

        // Said at all — which is the part a regression would take away, and which
        // nothing checked until there was a way to listen. (That it is said *before*
        // the unhook stays a matter of reading the two lines: the `Stopped` it must
        // precede comes from the pump, and there is none here. A stand-in pump would
        // only be asserting its own timing.)
        assert_eq!(agent.next("the rescue").await, Event::Rescued);
        assert!(
            !shared.want_hook.load(Ordering::SeqCst),
            "the rescue must take the engine down, not only announce it"
        );
        assert!(
            !shared.paused.load(Ordering::SeqCst),
            "a rescue out of a pause must not leave the engine parked in it: the next \
             Run would decide against a flag describing a window that is long gone"
        );
    }
}
