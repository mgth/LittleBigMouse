//! The agent end to end: the event loop, a world standing in for the machine, and the
//! fake hook over a private endpoint.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::HookClient;
use lbm_agent::reconcile::{Input, LayoutState, Timings, World};
use lbm_agent::runtime::Agent;
use lbm_agent::world::AgentWorld;
use lbm_ipc::protocol::{self, Command};

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

#[derive(Default)]
struct Record {
    rebuilds: u32,
    enabled_saves: Vec<bool>,
}

/// One monitor, an enabled layout; what was asked of it is recorded.
struct FakeWorld {
    layout: Option<LayoutState>,
    record: Arc<Mutex<Record>>,
}

impl World for FakeWorld {
    fn display_signature(&mut self) -> String {
        "one-monitor".into()
    }

    fn rebuild_layout(&mut self) {
        self.record.lock().unwrap().rebuilds += 1;
        self.layout.get_or_insert(LayoutState {
            enabled: true,
            is_virtual: false,
            saved: true,
        });
    }

    fn layout(&self) -> Option<LayoutState> {
        self.layout
    }

    fn set_enabled(&mut self, enabled: bool) {
        if let Some(layout) = &mut self.layout {
            layout.enabled = enabled;
        }
    }
}

impl AgentWorld for FakeWorld {
    fn zones(&self) -> Option<(String, bool)> {
        self.layout.map(|l| (ZONES.to_owned(), l.is_virtual))
    }

    fn save_enabled(&mut self) -> io::Result<()> {
        let enabled = self.layout.is_some_and(|l| l.enabled);
        self.record.lock().unwrap().enabled_saves.push(enabled);
        Ok(())
    }

    fn save_layout(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn quick() -> Timings {
    Timings {
        debounce: Duration::from_millis(5),
        stability_step: Duration::from_millis(5),
        ..Timings::default()
    }
}

#[cfg(unix)]
fn endpoint(dir: &tempfile::TempDir) -> String {
    dir.path().join("hook.sock").to_string_lossy().into_owned()
}

#[cfg(windows)]
fn endpoint(_dir: &tempfile::TempDir) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        r"\\.\pipe\lbm-agent-runtime-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

/// Waits (up to 5 s) until `done` holds.
async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..500 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never: {what}");
}

fn runs(fake: &FakeHook) -> usize {
    fake.received()
        .iter()
        .filter(|c| matches!(c, Command::Run))
        .count()
}

#[tokio::test]
async fn the_agent_hands_its_layout_to_the_hook_and_keeps_it_hooked() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let fake = FakeHook::bind(&endpoint).unwrap();
    let record = Arc::new(Mutex::new(Record::default()));
    let world = FakeWorld {
        layout: None,
        record: record.clone(),
    };

    let (hook, signals) = HookClient::spawn(endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let control = inputs.clone();
    let agent = tokio::spawn(async move {
        let mut agent = Agent::new(world, quick(), hook, inputs);
        agent
            .run(signals, inputs_rx, async {
                let _ = stopped.await;
            })
            .await;
    });

    // Booted with a layout the user wants: the hook, asking for one, gets it, hooked.
    until("the first layout is running", || fake.hooked()).await;
    assert_eq!(record.lock().unwrap().rebuilds, 1);
    assert_eq!(
        fake.received(),
        [Command::Listen, Command::Load(ZONES.into()), Command::Run]
    );

    // The hook unhooks itself over a display change and reports it. The first one after
    // boot rebuilds (as in C#, the guard does not know the configuration yet)...
    fake.broadcast(protocol::STOPPED);
    fake.broadcast(protocol::DISPLAY_CHANGED);
    until("rebuilt and handed over again", || runs(&fake) == 2).await;
    assert!(fake.hooked());
    assert_eq!(record.lock().unwrap().rebuilds, 2);

    // ...then a change that settles back to the built configuration is re-hooked, not
    // rebuilt.
    fake.broadcast(protocol::STOPPED);
    fake.broadcast(protocol::DISPLAY_CHANGED);
    until("re-hooked", || runs(&fake) == 3).await;
    assert!(fake.hooked());
    assert_eq!(record.lock().unwrap().rebuilds, 2);

    // The user's Stop is recorded, then unhooks.
    control.send(Input::UserStop).unwrap();
    until("stopped", || !fake.hooked()).await;
    assert_eq!(record.lock().unwrap().enabled_saves, [false]);
    assert!(matches!(fake.received().last(), Some(Command::Stop)));

    stop.send(()).unwrap();
    agent.await.unwrap();
}

#[tokio::test]
async fn a_hook_that_comes_late_or_comes_back_gets_the_layout() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let record = Arc::new(Mutex::new(Record::default()));
    let world = FakeWorld {
        layout: None,
        record,
    };

    let (hook, signals) = HookClient::spawn(endpoint.clone());
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let agent = tokio::spawn(async move {
        let mut agent = Agent::new(world, quick(), hook, inputs);
        agent
            .run(signals, inputs_rx, async {
                let _ = stopped.await;
            })
            .await;
    });

    // No hook at boot; it shows up later.
    tokio::time::sleep(Duration::from_millis(150)).await;
    let fake = FakeHook::bind(&endpoint).unwrap();
    until("the late hook runs the layout", || fake.hooked()).await;

    // It goes away (a crash) and a new one takes its place.
    drop(fake);
    let fake = FakeHook::bind(&endpoint).unwrap();
    until("the new hook runs the layout", || fake.hooked()).await;

    stop.send(()).unwrap();
    agent.await.unwrap();
}
