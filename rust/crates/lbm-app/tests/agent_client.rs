//! The client against a **real agent**, not a double.
//!
//! The v6 protocol cost four bugs to a fake that was kinder than the daemon (#668–#671),
//! and the rule that came out of it is written at the top of `fake_hook.rs`: exercise the
//! real server, in the shape the real client uses. So this brings up an actual
//! `lbm-agent` over an actual socket — driving the *fake hook*, which is what keeps the
//! test from going anywhere near a mouse — and points the window's own client at it.
//!
//! Unix only: the agent's Windows endpoint is a per-logon pipe and there is no logon
//! session here to hold one. The client's Windows path is written and compiles; it is
//! not claimed to be tested.
#![cfg(unix)]

use std::io;
use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::HookClient;
use lbm_agent::peers::{Service, Setup};
use lbm_agent::reconcile::{LayoutState, Timings, World};
use lbm_agent::runtime::Agent;
use lbm_agent::world::AgentWorld;
use lbm_app::client::{self, Incoming, Message, Outgoing, PROTOCOL};
use serde_json::{json, Value};

use lbm_app::session::{self, Config, Event as SessionEvent};

fn event_until(
    events: &std::sync::mpsc::Receiver<SessionEvent>,
    wanted: impl Fn(&SessionEvent) -> bool,
) -> SessionEvent {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = events
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .expect("session did not report the expected transition");
        if wanted(&event) {
            return event;
        }
    }
}

#[test]
fn reconnect_subscribes_again_and_does_not_reuse_the_old_request_queue() {
    let dir = std::sync::Arc::new(tempfile::tempdir().unwrap());
    let first = Running::at(dir.clone());
    let (send, events) = std::sync::mpsc::channel();
    let session = session::spawn(
        Config {
            endpoint: dir.path().join("lbm-agent.sock"),
            launch: None,
            retry: Duration::from_millis(10),
            deadline: Duration::from_secs(2),
        },
        move |event| send.send(event).is_ok(),
    );
    let SessionEvent::Connected {
        requests: old,
        snapshot,
        capabilities,
    } = event_until(&events, |event| {
        matches!(event, SessionEvent::Connected { .. })
    })
    else {
        unreachable!()
    };
    assert_eq!(snapshot["LayoutId"], "DELA0B1_ABC123");
    assert!(
        capabilities.iter().any(|c| c == "IntrinsicSize"),
        "{capabilities:?}"
    );
    assert!(snapshot["LayoutRevision"].as_u64().unwrap() > 0);
    drop(first);
    event_until(&events, |event| {
        matches!(event, SessionEvent::Disconnected(_))
    });
    assert!(
        old.send((3, "Stop", json!({}))).is_err(),
        "old commands must not wait for a new agent"
    );
    let _second = Running::at(dir);
    let SessionEvent::Connected {
        requests, snapshot, ..
    } = event_until(&events, |event| {
        matches!(event, SessionEvent::Connected { .. })
    })
    else {
        unreachable!()
    };
    let revision = snapshot["LayoutRevision"].as_u64().unwrap();
    requests.send((3, "Refresh", json!({}))).unwrap();
    let event = event_until(&events, |event| {
        matches!(event,
        SessionEvent::Message(Message::State(s)) if s["LayoutRevision"].as_u64().is_some_and(|r| r > revision))
    });
    let SessionEvent::Message(Message::State(snapshot)) = event else {
        unreachable!()
    };
    assert_eq!(
        snapshot["LayoutId"], "DELA0B1_ABC123",
        "a rebuild must be visible even with the same id"
    );
    drop(session);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !requests.is_closed() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        requests.is_closed(),
        "closing the window must close its connection"
    );
}

#[test]
fn a_missing_private_agent_can_appear_after_the_window_has_opened() {
    let dir = std::sync::Arc::new(tempfile::tempdir().unwrap());
    let (send, events) = std::sync::mpsc::channel();
    let _session = session::spawn(
        Config {
            endpoint: dir.path().join("lbm-agent.sock"),
            launch: None,
            retry: Duration::from_millis(10),
            deadline: Duration::from_secs(2),
        },
        move |event| send.send(event).is_ok(),
    );
    event_until(&events, |event| {
        matches!(event, SessionEvent::Disconnected(_))
    });
    let _server = Running::at(dir);
    event_until(&events, |event| {
        matches!(event, SessionEvent::Connected { .. })
    });
}

#[test]
fn incompatible_and_silent_endpoints_never_enable_the_controls() {
    use lbm_ipc::framing::{read_frame_blocking, write_frame_blocking};
    for answer in [Some(999), None] {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = dir.path().join("agent.sock");
        let listener = std::os::unix::net::UnixListener::bind(&endpoint).unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let hello: Value =
                serde_json::from_str(&read_frame_blocking(&mut socket).unwrap()).unwrap();
            if let Some(protocol) = answer {
                write_frame_blocking(
                    &mut socket,
                    &json!({"Id":hello["Id"], "Result":{
                    "Agent":"lbm-agent", "Protocol":protocol }})
                    .to_string(),
                )
                .unwrap();
            }
            // The client must close without subscribing or sending user commands.
            assert!(read_frame_blocking(&mut socket).is_err());
        });
        let (send, events) = std::sync::mpsc::channel();
        let session = session::spawn(
            Config {
                endpoint,
                launch: None,
                retry: Duration::from_secs(2),
                deadline: Duration::from_millis(100),
            },
            move |event| send.send(event).is_ok(),
        );
        match events.recv_timeout(Duration::from_secs(2)).unwrap() {
            SessionEvent::Disconnected(reason) => assert!(reason.contains(if answer.is_some() {
                "incompatible"
            } else {
                "timed out"
            })),
            event => panic!("unverified agent reached the UI: {event:?}"),
        }
        drop(session);
        server.join().unwrap();
    }
}

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

/// One monitor, a layout the user wants — the agent tests' own fixture — and the screens
/// it reports to a master in the multi-PC mode (none outside those tests).
struct Enabled(Option<LayoutState>, Vec<lbm_ipc::peer::PeerDisplay>);

impl World for Enabled {
    fn display_signature(&mut self) -> String {
        "one-monitor".into()
    }
    fn rebuild_layout(&mut self) {
        self.0.get_or_insert(LayoutState {
            enabled: true,
            is_virtual: false,
            saved: true,
        });
    }
    fn layout(&self) -> Option<LayoutState> {
        self.0
    }
    fn set_enabled(&mut self, enabled: bool) {
        if let Some(l) = &mut self.0 {
            l.enabled = enabled;
        }
    }
}

impl AgentWorld for Enabled {
    fn peer_displays(&mut self) -> Vec<lbm_ipc::peer::PeerDisplay> {
        self.1.clone()
    }
    fn zones(&self) -> Option<(String, bool)> {
        self.0.map(|_| (ZONES.to_owned(), false))
    }
    fn desktop(&self) -> Option<lbm_ipc::protocol::Desktop> {
        self.0.map(|_| lbm_ipc::protocol::Desktop {
            left: 0,
            top: 0,
            width: 3840,
            height: 1080,
        })
    }
    fn save_enabled(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn save_layout(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn layout_id(&self) -> Option<String> {
        self.0.map(|_| "DELA0B1_ABC123".to_owned())
    }
}

/// A running agent on its own socket, and the client connected to it.
///
/// Field order is drop order: the fake hook closes its socket when it falls, and it has
/// to do that before the directory holding it goes. Dropping it early — which a first
/// draft of this did — leaves the agent with no hook at all.
struct Running {
    incoming: Incoming,
    outgoing: Outgoing,
    /// The endpoint listens until this falls.
    _listener: lbm_agent::api::Listener,
    _fake: FakeHook,
    /// Everything runs on this; dropping it stops the agent.
    _runtime: tokio::runtime::Runtime,
    _dir: std::sync::Arc<tempfile::TempDir>,
    /// The last state seen, so that a state already reached is not waited for.
    last: Value,
}

impl Running {
    fn start() -> Running {
        Self::at(std::sync::Arc::new(tempfile::tempdir().unwrap()))
    }

    fn at(dir: std::sync::Arc<tempfile::TempDir>) -> Running {
        Self::machine(dir, None)
    }

    /// An agent offering the multi-PC mode, listening on loopback only, that reports
    /// `displays` to the masters it pairs with.
    fn with_machines(displays: Vec<lbm_ipc::peer::PeerDisplay>) -> Running {
        Self::machine(
            std::sync::Arc::new(tempfile::tempdir().unwrap()),
            Some(displays),
        )
    }

    fn machine(
        dir: std::sync::Arc<tempfile::TempDir>,
        multi_pc: Option<Vec<lbm_ipc::peer::PeerDisplay>>,
    ) -> Running {
        let at = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
        let (hook_endpoint, api_endpoint) = (at("hook.sock"), at("lbm-agent.sock"));

        // Everything the agent is made of spawns tasks, so it is all built inside the
        // runtime rather than beside it — `tokio::spawn` outside one panics, which is
        // how this test first failed.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let entered = runtime.enter();

        // The fake hook, and nothing else: no mouse is opened anywhere in this test.
        let fake = FakeHook::bind(&hook_endpoint).unwrap();
        let (hook, signals) = HookClient::spawn(hook_endpoint);
        let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
        let (calls, listener) = lbm_agent::api::listen(&api_endpoint).unwrap();
        let peers = multi_pc.as_ref().map(|_| {
            let mut setup = Setup::new(None);
            setup.bind = std::net::Ipv4Addr::LOCALHOST.into();
            Service::new(dir.path().join("config"), setup)
        });
        runtime.spawn(async move {
            let world = Enabled(None, multi_pc.unwrap_or_default());
            let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
            if let Some(peers) = peers {
                agent = agent.with_peers(peers);
            }
            agent.run(signals, inputs_rx, std::future::pending()).await;
        });
        drop(entered);

        // The socket appears when the listener binds, which is on another thread.
        let path = std::path::PathBuf::from(&api_endpoint);
        let (incoming, outgoing) = (0..200)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(10));
                // A deadline, so that a message that never comes fails the test instead of
                // hanging it — which is how this harness first behaved.
                client::connect_with(&path, Some(Duration::from_secs(5))).ok()
            })
            .expect("the agent never came up");

        Running {
            incoming,
            outgoing,
            _listener: listener,
            _fake: fake,
            _runtime: runtime,
            _dir: dir,
            last: Value::Null,
        }
    }

    /// The answer to `id`, skipping whatever else arrives first — events and answers
    /// share the connection, and a client that assumed the next frame was its answer
    /// would be right only until the agent had something to say.
    fn answer(&mut self, id: u64) -> Result<Value, String> {
        for _ in 0..200 {
            let message = self.incoming.receive().expect("the agent stopped talking");
            if let Message::Answer { id: got, result } = message {
                if got == id {
                    return result;
                }
            }
        }
        panic!("no answer to {id}");
    }

    /// The state once it satisfies `wanted` — **including when it already did**.
    ///
    /// A `State` event is a *change*, and the agent may well have reached what is being
    /// waited for before the connection was made: it takes the layout on its own, and
    /// this client connects by polling. A frontend that only ever watched for changes
    /// would sit for ever in front of a state that was already correct, which is how
    /// two of these tests first failed.
    fn settled(&mut self, wanted: impl Fn(&Value) -> bool) -> Value {
        if wanted(&self.last) {
            return self.last.clone();
        }
        for _ in 0..200 {
            let message = self.incoming.receive().expect("the agent stopped talking");
            if let Message::State(state) = message {
                self.last = state.clone();
                if wanted(&state) {
                    return state;
                }
            }
        }
        panic!("no state matched; last seen {}", self.last);
    }
}

#[test]
fn the_agent_answers_who_it_is_and_which_protocol() {
    let mut agent = Running::start();

    let id = agent
        .outgoing
        .ask("Hello", json!({ "Client": "lbm-app" }))
        .unwrap();
    let hello = agent.answer(id).expect("Hello was refused");

    assert_eq!(hello["Agent"], "lbm-agent");
    assert_eq!(
        hello["Protocol"], PROTOCOL,
        "the client speaks a protocol the agent does not"
    );
    // The size editor lets a size cross the detected shape only for an agent that
    // says it keeps marked sizes as stored (#795 review).
    assert!(
        hello["Capabilities"]
            .as_array()
            .is_some_and(|all| all.iter().any(|c| c == "IntrinsicSize")),
        "{hello}"
    );
}

/// Subscribing answers with the state now, and then every change arrives on its own.
#[test]
fn subscribing_gives_the_state_and_then_the_changes() {
    let mut agent = Running::start();

    let id = agent.outgoing.ask("Subscribe", json!({})).unwrap();
    let now = agent.answer(id).expect("Subscribe was refused");
    assert_eq!(now["LayoutId"], "DELA0B1_ABC123");
    agent.last = now;

    // The agent takes the layout on its own; the client learns it without asking.
    let running = agent.settled(|s| s["Engine"] == "Running" && s["HookConnected"] == true);
    assert_eq!(running["Enabled"], true);
}

/// A press reaches the agent and the answer comes back — under the id it was asked with,
/// which is what lets an answer be told from an event on one connection.
#[test]
fn a_stop_is_asked_answered_and_then_seen_in_the_state() {
    let mut agent = Running::start();
    let subscribed = agent.outgoing.ask("Subscribe", json!({})).unwrap();
    agent.last = agent.answer(subscribed).unwrap();
    agent.settled(|s| s["Engine"] == "Running");

    let id = agent.outgoing.ask("Stop", json!({})).unwrap();
    assert_eq!(agent.answer(id), Ok(Value::Null));

    let stopped = agent.settled(|s| s["Engine"] == "Stopped");
    assert_eq!(stopped["Enabled"], false, "a user's Stop is remembered");
}

/// Ids are the client's and they do not repeat: two requests in flight have to be
/// tellable apart, which is the whole reason the frame carries one.
#[test]
fn every_request_gets_its_own_id() {
    let mut agent = Running::start();

    let first = agent
        .outgoing
        .ask("Hello", json!({ "Client": "a" }))
        .unwrap();
    let second = agent
        .outgoing
        .ask("Hello", json!({ "Client": "b" }))
        .unwrap();
    assert_ne!(first, second);

    assert!(agent.answer(first).is_ok());
    assert!(agent.answer(second).is_ok());
}

/// Nothing listening is an answer, not a failure to work around — and above all it does
/// not start an agent.
#[test]
fn no_agent_is_an_error_and_nothing_is_started() {
    let dir = tempfile::tempdir().unwrap();
    let nowhere = dir.path().join("lbm-agent.sock");

    assert!(client::connect(&nowhere).is_err());
    assert!(
        !nowhere.exists(),
        "connecting created something: the client must never bring an agent up"
    );
}

/// **An id reserved before the frame goes out is an id the answer cannot beat.**
///
/// The window pairs an answer with what it answers by recording the id before sending.
/// This checks the half that has to be true of the *agent*: that the id it answers under
/// is the one the request carried, for two requests in flight at once — a pairing done by
/// arrival order instead would come apart exactly here.
#[test]
fn every_answer_comes_back_under_the_id_its_request_carried() {
    let mut running = Running::start();

    // Two out before either is answered, and the second is the one with a payload.
    let snapshot = running.outgoing.reserve();
    running
        .outgoing
        .ask_as(snapshot, "Snapshot", json!({}))
        .expect("sent");
    let seen = running.outgoing.reserve();
    running
        .outgoing
        .ask_as(seen, "SeenProcesses", json!({}))
        .expect("sent");
    assert_ne!(snapshot, seen, "two requests must not share an id");

    let mut answers: Vec<(u64, bool)> = Vec::new();
    for _ in 0..200 {
        match running
            .incoming
            .receive()
            .expect("the agent stopped talking")
        {
            Message::Answer { id, result } => {
                answers.push((id, result.is_ok()));
                if answers.len() == 2 {
                    break;
                }
            }
            _ => continue,
        }
    }
    assert_eq!(answers.len(), 2, "one of the two was never answered");
    let ids: Vec<u64> = answers.iter().map(|(id, _)| *id).collect();
    assert!(
        ids.contains(&snapshot) && ids.contains(&seen),
        "the answers came back under ids nobody asked with: {ids:?} for {snapshot} and {seen}"
    );
    assert!(
        answers.iter().all(|(_, ok)| *ok),
        "the agent refused one of them: {answers:?}"
    );
}

/// `SeenProcesses` answers with a list — the one request whose *value* the window reads.
/// An empty session is still an array, not null, or the window would show nothing and be
/// unable to tell that from a failure.
#[test]
fn seen_processes_answers_with_a_list() {
    let mut running = Running::start();
    let id = running.outgoing.reserve();
    running
        .outgoing
        .ask_as(id, "SeenProcesses", json!({}))
        .expect("sent");

    for _ in 0..200 {
        if let Message::Answer {
            id: answered,
            result,
        } = running
            .incoming
            .receive()
            .expect("the agent stopped talking")
        {
            if answered != id {
                continue;
            }
            let value = result.expect("SeenProcesses was refused");
            assert!(
                value.is_array(),
                "the window reads this as a list of names: {value}"
            );
            return;
        }
    }
    panic!("SeenProcesses was never answered");
}

/// A request the agent turns down comes back as an error under its own id, which is what
/// lets the window say *which* request failed instead of going quiet.
#[test]
fn a_refused_request_is_an_error_under_its_own_id() {
    let mut running = Running::start();
    let id = running.outgoing.reserve();
    // This world keeps no options, so the agent refuses in words.
    running
        .outgoing
        .ask_as(id, "SaveOptions", json!({ "Options": {} }))
        .expect("sent");

    for _ in 0..200 {
        if let Message::Answer {
            id: answered,
            result,
        } = running
            .incoming
            .receive()
            .expect("the agent stopped talking")
        {
            if answered != id {
                continue;
            }
            let why = result.expect_err("this world cannot keep options");
            assert!(!why.is_empty(), "a refusal with nothing to show the user");
            return;
        }
    }
    panic!("the refusal never came back");
}

impl Running {
    /// The answer to one request, which must be accepted.
    fn ask_ok(&mut self, method: &str, extra: Value) -> Value {
        let id = self.outgoing.ask(method, extra).unwrap();
        self.answer(id)
            .unwrap_or_else(|why| panic!("{method} refused: {why}"))
    }

    /// The multi-PC state once it satisfies `wanted`: as it is now, or as a `Peers` event
    /// announces it later — the event being what the window waits on, never a poll.
    fn peers_until(&mut self, wanted: impl Fn(&Value) -> bool) -> Value {
        let now = self.ask_ok("Peers", json!({}));
        if wanted(&now) {
            return now;
        }
        for _ in 0..200 {
            let message = self.incoming.receive().expect("the agent stopped talking");
            if let Message::Peers(state) = message {
                if wanted(&state) {
                    return state;
                }
            }
        }
        panic!("no Peers event matched");
    }
}

/// A laptop screen, as the remote reports it.
fn laptop_screen() -> lbm_ipc::peer::PeerDisplay {
    lbm_ipc::peer::PeerDisplay {
        id: "eDP-1".into(),
        connector: "eDP-1".into(),
        name: "Built-in display".into(),
        edid: None,
        x: 0,
        y: 0,
        width: 1920,
        height: 1200,
        scale: 1.0,
        effective_dpi: 96.0,
        orientation: 0,
        frequency: 60.0,
        primary: true,
        attached: true,
        width_mm: 302.0,
        height_mm: 189.0,
        input_source: None,
    }
}

fn machines_of(state: &Value) -> (lbm_ui::machines::State, Vec<lbm_app::machines::Remote>) {
    lbm_app::machines::read(state).expect("a multi-PC state")
}

/// The whole of lot 7's conversation, against two real agents on loopback: the desk's
/// window connects to the laptop, the laptop's window sees the desk waiting with its
/// fingerprint, both accept with the requests the Machines page sends, and the desk's
/// window gets the laptop's screens by the `Peers` event and puts them on its map.
#[test]
fn a_pending_machine_is_accepted_by_its_fingerprint_and_its_screens_reach_the_map() {
    use lbm_ui::machines::Did;

    let mut laptop = Running::with_machines(vec![laptop_screen()]);
    let mut desk = Running::with_machines(Vec::new());
    for agent in [&mut laptop, &mut desk] {
        agent.ask_ok("Subscribe", json!({}));
    }
    let hello = desk.ask_ok("Hello", json!({ "Client": "lbm-app" }));
    assert!(
        hello["Capabilities"]
            .as_array()
            .is_some_and(|all| all.iter().any(|c| c == lbm_app::machines::CAPABILITY)),
        "{hello}"
    );

    let window = lbm_app::machines::Controller::default();
    let (method, mut extra) = window.request(&Did::Enable(true));
    assert_eq!(method, "SetMultiPc");
    // The page's switch sends no port and no name: the agent keeps its own. The test
    // gives them, so that two agents on one machine do not want the same port.
    extra["Port"] = json!(0);
    extra["Name"] = json!("laptop");
    let (laptop_state, _) = machines_of(&laptop.ask_ok(method, extra.clone()));
    let address = laptop_state.listen_address.clone().expect("listening");
    let laptop_id = laptop_state.machine_id.clone().expect("an identity");
    extra["Name"] = json!("desk");
    let desk_id = machines_of(&desk.ask_ok(method, extra))
        .0
        .machine_id
        .expect("an identity");

    let (method, extra) = window.request(&Did::Connect(address));
    let entry = desk.ask_ok(method, extra);
    assert_eq!(entry["MachineId"], laptop_id.as_str());
    assert_eq!(entry["State"], "Pending");

    // The laptop's page: the desk, waiting, with the fingerprint its user compares.
    let waiting = laptop.peers_until(|s| {
        machines_of(s)
            .0
            .peers
            .iter()
            .any(|p| p.machine_id == desk_id && p.pending)
    });
    let (seen, remotes) = machines_of(&waiting);
    let desk_seen = seen.peers.iter().find(|p| p.machine_id == desk_id).unwrap();
    assert_eq!(desk_seen.name, "desk");
    assert_eq!(desk_seen.role, lbm_ui::machines::Role::Master);
    assert!(remotes.is_empty(), "a master's screens are not this map's");
    assert_eq!(
        lbm_ui::machines::fingerprint(&desk_seen.machine_id),
        lbm_ui::machines::fingerprint(&desk_id),
        "the laptop prints the fingerprint the desk prints of itself"
    );

    // Both users say yes, each with the request the page's Accept sends.
    let (method, extra) = window.request(&Did::Accept(desk_id.clone()));
    assert_eq!(method, "PeerAccept");
    assert_eq!(laptop.ask_ok(method, extra), Value::Null);
    let (method, extra) = window.request(&Did::Accept(laptop_id.clone()));
    assert_eq!(desk.ask_ok(method, extra), Value::Null);

    // The desk's window learns the laptop's screens from the event.
    let linked = desk.peers_until(|s| {
        machines_of(s)
            .1
            .iter()
            .any(|r| r.machine_id == laptop_id && !r.displays.is_empty())
    });
    let mut window = lbm_app::machines::Controller::default();
    assert!(window.heard(&linked), "the map has a remote to merge");
    assert_eq!(window.remotes[0].name, "laptop");

    // And the map gets it: merged after this machine's screens, before the id.
    let mut layout = lbm_layout::model::Layout::new(Default::default());
    lbm_app::machines::build(
        &mut layout,
        |l| lbm_layout::linux::add_monitor(l, &lbm_layout::linux::LinuxMonitor::fallback()),
        &window.remotes,
        |_| {},
    );
    assert_eq!(layout.monitors().len(), 2);
    let remote = layout
        .monitors()
        .iter()
        .find(|m| lbm_app::machines::remote_machine(&layout, &m.id).is_some())
        .expect("the laptop's screen is on the map");
    assert!(layout.id.contains(&remote.id), "the layout id counts it");
    assert_eq!(
        lbm_app::machines::tag(&layout, &remote.id, "desk"),
        Some(lbm_app::machines::Tag {
            machine: "laptop".into(),
            remote: true,
            shared: false,
            problem: None,
        })
    );
}

/// A shared panel's switch reaches the agent under its own id, and whatever the agent
/// makes of it, the window can tell an agent that does not know the request from one
/// that refused it.
#[test]
fn a_switch_is_answered_under_its_own_id_and_an_older_agent_is_told_from_a_refusal() {
    let mut running = Running::start();
    let id = running.outgoing.reserve();
    let switch = lbm_app::machines::Switch {
        id,
        layout: "DELA0B1_ABC123".into(),
        monitor: "DELA0B1_ABC123".into(),
        source: "DP-1@a1b2c3d4".into(),
    };
    running
        .outgoing
        .ask_as(id, "SetActiveSource", switch.request())
        .unwrap();
    let result = running.answer(id);
    match (&result, lbm_app::machines::switched(&result)) {
        (Err(why), lbm_app::machines::Switched::Unknown) => {
            assert!(why.starts_with("bad request"), "{why}");
        }
        (Err(why), lbm_app::machines::Switched::Refused(said)) => {
            assert_eq!(why, &said);
            assert!(!said.is_empty(), "a refusal with nothing to show");
        }
        (Ok(_), lbm_app::machines::Switched::Taken) => {}
        (result, switched) => panic!("{result:?} read as {switched:?}"),
    }
}
