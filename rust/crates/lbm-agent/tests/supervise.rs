//! Supervision at process level: the agent launches the hook when none answers, detached,
//! and the hook outlives the agent (D5). The "hook" is this crate's binary serving a
//! fake hook (`--serve-fake-hook`): nothing here ever starts the real one.

use std::io;
use std::time::Duration;

use lbm_agent::hook::{HookClient, HookSignal};
use lbm_agent::reconcile::{LayoutState, Timings, World};
use lbm_agent::runtime::Agent;
use lbm_agent::supervise::HookLauncher;
use lbm_agent::world::AgentWorld;
use lbm_ipc::protocol::{self, Command, Event};
use tokio::sync::mpsc::UnboundedReceiver;

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

/// An enabled layout, nothing else.
struct Enabled(Option<LayoutState>);

impl World for Enabled {
    fn display_signature(&mut self) -> String {
        "one-monitor".into()
    }
    fn rebuild_layout(&mut self) {
        self.0 = Some(LayoutState {
            enabled: true,
            is_virtual: false,
            saved: true,
        });
    }
    fn layout(&self) -> Option<LayoutState> {
        self.0
    }
    fn set_enabled(&mut self, _: bool) {}
}

impl AgentWorld for Enabled {
    fn zones(&self) -> Option<(String, bool)> {
        self.0.map(|_| (ZONES.to_owned(), false))
    }

    /// The desktop the fixture's zones sit on. Named rather than inferred, as the real
    /// world names it: a world that answered None would be exercising the fallback.
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
}

#[cfg(unix)]
fn endpoint(dir: &tempfile::TempDir) -> String {
    dir.path().join("hook.sock").to_string_lossy().into_owned()
}

#[cfg(windows)]
fn endpoint(_dir: &tempfile::TempDir) -> String {
    format!(r"\\.\pipe\lbm-agent-supervise-test-{}", std::process::id())
}

/// Asks the hook at `endpoint` for its state until it answers `want` (up to 10 s).
async fn until_state(endpoint: &str, want: Event) {
    let (observer, mut signals) = HookClient::spawn(endpoint.to_owned());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        observer.send(protocol::frame(&[Command::State]));
        match tokio::time::timeout_at(deadline, next_state(&mut signals)).await {
            Ok(Some(state)) if state == want => return,
            Ok(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            Err(_) => panic!("the hook at {endpoint} never answered {want:?}"),
        }
    }
}

/// The next state the hook reports; `None` for a connection signal (after a short
/// pause, so a retry does not spin).
async fn next_state(signals: &mut UnboundedReceiver<HookSignal>) -> Option<Event> {
    match signals.recv().await? {
        HookSignal::Message(m) => Some(m),
        HookSignal::Connected
        | HookSignal::Greeted { .. }
        | HookSignal::Lost
        | HookSignal::Unreachable => {
            tokio::time::sleep(Duration::from_millis(50)).await;
            None
        }
    }
}

#[tokio::test]
async fn an_absent_hook_is_launched_detached_and_outlives_the_agent() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let launcher = HookLauncher::new(
        env!("CARGO_BIN_EXE_lbm-agent").into(),
        vec!["--serve-fake-hook".into(), endpoint.clone().into()],
        Some(dir.path().join("hook.log")),
    );

    // No hook: the agent launches one, which asks for the layout and runs it.
    let (hook, signals) = HookClient::spawn(endpoint.clone());
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let agent = tokio::spawn(async move {
        let mut agent =
            Agent::new(Enabled(None), Timings::default(), hook, inputs).with_launcher(launcher);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    until_state(&endpoint, Event::Running).await;

    // The agent goes away (a crash): the hook it launched is still running its layout.
    agent.abort();
    let _ = agent.await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    until_state(&endpoint, Event::Running).await;

    // Done: the hook process leaves when told to.
    let (observer, mut signals) = HookClient::spawn(endpoint.clone());
    loop {
        if let Some(HookSignal::Connected) = signals.recv().await {
            break;
        }
    }
    observer.send(protocol::frame(&[Command::Quit]));
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(HookSignal::Lost) = signals.recv().await {
                return;
            }
        }
    })
    .await
    .expect("the hook left");
}
