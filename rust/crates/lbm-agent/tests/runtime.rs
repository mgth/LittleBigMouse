//! The agent end to end: the event loop, a world standing in for the machine, and the
//! fake hook over a private endpoint.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::HookClient;
use lbm_agent::reconcile::{Input, LayoutState, Timings, World};
use lbm_agent::runtime::{Agent, SleepSignal};
use lbm_agent::world::AgentWorld;
use lbm_ipc::protocol::{self, Command, Event};

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

/// A new hook at the endpoint of one that just went away. On Windows the pipe is
/// created as its first instance, which is refused ("access denied") while any instance
/// of the old one remains — until its tasks are dropped and the agent has let go of its
/// end — so the new hook retries, as a hook started right after a crash would.
async fn rebind(endpoint: &str) -> FakeHook {
    for _ in 0..250 {
        match FakeHook::bind(endpoint) {
            Ok(fake) => return fake,
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    panic!("the endpoint {endpoint} never came free");
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
        [
            Command::Hello {
                protocol: protocol::PROTOCOL
            },
            Command::Listen,
            Command::Load {
                zones: ZONES.into()
            },
            Command::Run
        ]
    );

    // The hook unhooks itself over a display change and reports it. The first one after
    // boot rebuilds (as in C#, the guard does not know the configuration yet)...
    fake.broadcast(&Event::Stopped);
    fake.broadcast(&Event::DisplayChanged);
    until("rebuilt and handed over again", || runs(&fake) == 2).await;
    assert!(fake.hooked());
    assert_eq!(record.lock().unwrap().rebuilds, 2);

    // ...then a change that settles back to the built configuration is re-hooked, not
    // rebuilt.
    fake.broadcast(&Event::Stopped);
    fake.broadcast(&Event::DisplayChanged);
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
    let fake = rebind(&endpoint).await;
    until("the new hook runs the layout", || fake.hooked()).await;

    stop.send(()).unwrap();
    agent.await.unwrap();
}

#[tokio::test]
async fn the_hook_is_let_go_before_the_system_sleeps_and_taken_back_after() {
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
    let (sleep, sleep_rx) = tokio::sync::mpsc::unbounded_channel();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let agent = tokio::spawn(async move {
        let mut agent = Agent::new(world, quick(), hook, inputs).with_sleep(sleep_rx);
        agent
            .run(signals, inputs_rx, async {
                let _ = stopped.await;
            })
            .await;
    });
    until("the first layout is running", || fake.hooked()).await;

    // logind waits for the answer: by then, the hook is down.
    let (done, done_rx) = tokio::sync::oneshot::channel();
    sleep.send(SleepSignal::Starting(done)).unwrap();
    done_rx.await.unwrap();
    until("unhooked", || !fake.hooked()).await;
    assert!(matches!(fake.received().last(), Some(Command::Stop)));

    // A display change while asleep is left to the wake.
    fake.broadcast(&Event::DisplayChanged);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!fake.hooked());

    // Awake: the wake goes through the display flow (the first one after boot rebuilds,
    // as above) and the hook is back. Nothing was recorded: the user stopped nothing.
    sleep.send(SleepSignal::Ended).unwrap();
    until("re-hooked after the wake", || fake.hooked()).await;
    assert_eq!(record.lock().unwrap().rebuilds, 2);
    assert!(record.lock().unwrap().enabled_saves.is_empty());

    stop.send(()).unwrap();
    agent.await.unwrap();
}

/// A world whose engine prologue opens gaps once (as the KWin gap guard does), which
/// changes the display signature; the epilogue closes them.
struct GappingWorld {
    layout: Option<LayoutState>,
    gapped: bool,
    rebuilds: Arc<Mutex<u32>>,
    restores: Arc<Mutex<u32>>,
}

impl World for GappingWorld {
    fn display_signature(&mut self) -> String {
        if self.gapped { "gapped" } else { "one-monitor" }.into()
    }

    fn rebuild_layout(&mut self) {
        *self.rebuilds.lock().unwrap() += 1;
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

impl AgentWorld for GappingWorld {
    fn zones(&self) -> Option<(String, bool)> {
        self.layout.map(|_| (ZONES.to_owned(), false))
    }

    fn save_enabled(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn save_layout(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn prepare_for_engine(&mut self) -> bool {
        !std::mem::replace(&mut self.gapped, true)
    }

    fn restore_after_engine(&mut self) -> bool {
        let moved = std::mem::replace(&mut self.gapped, false);
        if moved {
            *self.restores.lock().unwrap() += 1;
        }
        moved
    }
}

#[tokio::test]
async fn a_start_that_moves_the_outputs_waits_for_the_new_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let fake = FakeHook::bind(&endpoint).unwrap();
    let rebuilds = Arc::new(Mutex::new(0));
    let restores = Arc::new(Mutex::new(0));
    let world = GappingWorld {
        layout: None,
        gapped: false,
        rebuilds: rebuilds.clone(),
        restores: restores.clone(),
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

    // The first Start opens the gaps: its zones describe the old desktop and are never
    // sent. The display change rebuilds, and the Start in the gapped geometry goes out.
    until("running in the gapped geometry", || fake.hooked()).await;
    assert_eq!(*rebuilds.lock().unwrap(), 2);
    assert_eq!(
        fake.received(),
        [
            Command::Hello {
                protocol: protocol::PROTOCOL
            },
            Command::Listen,
            Command::Load {
                zones: ZONES.into()
            },
            Command::Run
        ],
        "one hand-over, the one computed after the gaps"
    );

    // Stopping closes them.
    control.send(Input::UserStop).unwrap();
    until("stopped", || !fake.hooked()).await;
    until("the gaps closed", || *restores.lock().unwrap() == 1).await;

    stop.send(()).unwrap();
    agent.await.unwrap();
}
