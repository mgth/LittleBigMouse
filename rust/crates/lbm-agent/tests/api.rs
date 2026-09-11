//! The frontends' API over a real socket, against an agent driving the fake hook.
#![cfg(unix)]

use std::io;
use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::HookClient;
use lbm_agent::reconcile::{LayoutState, Timings, World};
use lbm_agent::runtime::Agent;
use lbm_agent::world::AgentWorld;
use lbm_ipc::framing::{read_frame, write_frame};
use lbm_ipc::protocol::Command;
use serde_json::{json, Value};
use tokio::net::UnixStream;

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

/// One monitor, a layout the user wants.
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

/// A frontend: requests out, answers and events in.
struct Frontend {
    stream: UnixStream,
    next: u64,
}

impl Frontend {
    async fn connect(path: &str) -> Frontend {
        Frontend {
            stream: UnixStream::connect(path).await.unwrap(),
            next: 0,
        }
    }

    async fn send(&mut self, mut request: Value) -> u64 {
        self.next += 1;
        request["Id"] = json!(self.next);
        write_frame(&mut self.stream, &request.to_string())
            .await
            .unwrap();
        self.next
    }

    async fn receive(&mut self) -> Value {
        let frame = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut self.stream))
            .await
            .expect("a frame within 5 s")
            .unwrap();
        serde_json::from_str(&frame).unwrap()
    }

    /// The answer to `request`, skipping events.
    async fn ask(&mut self, request: Value) -> Value {
        let id = self.send(request).await;
        loop {
            let frame = self.receive().await;
            if frame["Id"] == json!(id) {
                return frame;
            }
        }
    }

    /// The next `State` event whose snapshot satisfies `want`.
    async fn state(&mut self, want: impl Fn(&Value) -> bool) -> Value {
        loop {
            let frame = self.receive().await;
            if frame["Event"] == "State" && want(&frame["State"]) {
                return frame["State"].clone();
            }
        }
    }
}

#[tokio::test]
async fn a_frontend_sees_the_agent_and_drives_it() {
    let dir = tempfile::tempdir().unwrap();
    let hook_endpoint = dir.path().join("hook.sock").to_string_lossy().into_owned();
    let api_endpoint = dir
        .path()
        .join("lbm-agent.sock")
        .to_string_lossy()
        .into_owned();
    let fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = lbm_agent::api::listen(&api_endpoint).unwrap();
    let agent = tokio::spawn(async move {
        let mut agent = Agent::new(Enabled(None), Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    let hello = frontend
        .ask(json!({ "Method": "Hello", "Client": "test" }))
        .await;
    assert_eq!(hello["Result"]["Agent"], "lbm-agent");
    assert_eq!(hello["Result"]["Protocol"], 1);

    // Subscribed: the state now, then every change — here, the hook taking the layout.
    let subscribed = frontend.ask(json!({ "Method": "Subscribe" })).await;
    assert_eq!(subscribed["Result"]["LayoutId"], "DELA0B1_ABC123");
    let running = frontend
        .state(|s| s["Engine"] == "Running" && s["HookConnected"] == true)
        .await;
    assert_eq!(running["Enabled"], true);
    assert!(fake.hooked());

    // The user's Stop, from the frontend: recorded, unhooked, and seen.
    let stop = frontend.ask(json!({ "Method": "Stop" })).await;
    assert_eq!(stop["Result"], Value::Null);
    let stopped = frontend
        .state(|s| s["Engine"] == "Stopped" && s["Enabled"] == false)
        .await;
    assert_eq!(stopped["LayoutId"], "DELA0B1_ABC123");
    assert!(!fake.hooked());

    // Anything else is refused, not guessed at; a snapshot is always there.
    let unknown = frontend.ask(json!({ "Method": "Teleport" })).await;
    assert!(unknown["Error"]
        .as_str()
        .unwrap()
        .starts_with("bad request"));
    let snapshot = frontend.ask(json!({ "Method": "Snapshot" })).await;
    assert_eq!(snapshot["Result"]["Engine"], "Stopped");

    // Quit: the hook is told to leave, and so does the agent.
    let quit = frontend.ask(json!({ "Method": "Quit" })).await;
    assert_eq!(quit["Result"], Value::Null);
    tokio::time::timeout(Duration::from_secs(5), agent)
        .await
        .expect("the agent left")
        .unwrap();
    // The agent left only once its Quit was written: the hook has it.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(fake.received().last(), Some(Command::Quit)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the hook was told to quit");
}
