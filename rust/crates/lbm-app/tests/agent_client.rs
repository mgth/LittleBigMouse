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
use lbm_agent::reconcile::{LayoutState, Timings, World};
use lbm_agent::runtime::Agent;
use lbm_agent::world::AgentWorld;
use lbm_app::client::{self, Incoming, Message, Outgoing, PROTOCOL};
use serde_json::{json, Value};

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

/// One monitor, a layout the user wants — the agent tests' own fixture.
struct Enabled(Option<LayoutState>);

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
    _dir: tempfile::TempDir,
    /// The last state seen, so that a state already reached is not waited for.
    last: Value,
}

impl Running {
    fn start() -> Running {
        let dir = tempfile::tempdir().unwrap();
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
        runtime.spawn(async move {
            let mut agent =
                Agent::new(Enabled(None), Timings::default(), hook, inputs).with_api(calls);
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
