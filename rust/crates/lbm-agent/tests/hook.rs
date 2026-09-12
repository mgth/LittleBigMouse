//! The connection to the hook, against the fake hook over a real local endpoint.

use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::{HookClient, HookSignal};
use lbm_ipc::protocol::{self, Command, Event};
use tokio::sync::mpsc::UnboundedReceiver;

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

/// A private endpoint: never the real hook's.
#[cfg(unix)]
fn endpoint(dir: &tempfile::TempDir) -> String {
    dir.path().join("hook.sock").to_string_lossy().into_owned()
}

#[cfg(windows)]
fn endpoint(_dir: &tempfile::TempDir) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        r"\\.\pipe\lbm-agent-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    )
}

async fn next(signals: &mut UnboundedReceiver<HookSignal>) -> HookSignal {
    tokio::time::timeout(Duration::from_secs(5), signals.recv())
        .await
        .expect("a signal within 5 s")
        .expect("the connection task is alive")
}

/// What a hook with no layout loaded greets with.
fn greeting_holding_nothing() -> HookSignal {
    HookSignal::Greeted {
        layout: String::new(),
    }
}

async fn event(signals: &mut UnboundedReceiver<HookSignal>) -> Event {
    match next(signals).await {
        HookSignal::Message(message) => message,
        other => panic!("expected an event, got {other:?}"),
    }
}

#[tokio::test]
async fn the_client_subscribes_then_follows_the_hook() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let fake = FakeHook::bind(&endpoint).unwrap();
    let (hook, mut signals) = HookClient::spawn(endpoint);

    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    // The greeting comes first: the opening frame asks Hello before Listen.
    assert_eq!(next(&mut signals).await, greeting_holding_nothing());
    // Subscribing is answered with the hook's state: no layout yet.
    assert_eq!(event(&mut signals).await, Event::Stopped);

    // Load and Run in one frame, as C# sends them.
    hook.send(protocol::frame(&[
        Command::Load {
            zones: ZONES.into(),
        },
        Command::Run,
    ]));
    assert!(matches!(event(&mut signals).await, Event::Loaded { .. }));
    assert_eq!(event(&mut signals).await, Event::Running);
    assert!(fake.hooked());
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

    // What the hook reports on its own reaches the client too.
    fake.broadcast(&Event::DisplayChanged);
    assert_eq!(event(&mut signals).await, Event::DisplayChanged);

    hook.send(protocol::frame(&[Command::Stop]));
    assert_eq!(event(&mut signals).await, Event::Stopped);
    assert!(!fake.hooked());
}

#[tokio::test]
async fn a_hook_that_goes_away_is_signalled_and_found_again() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let fake = FakeHook::bind(&endpoint).unwrap();
    let (hook, mut signals) = HookClient::spawn(endpoint.clone());
    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    assert_eq!(next(&mut signals).await, greeting_holding_nothing());
    assert_eq!(event(&mut signals).await, Event::Stopped);

    drop(fake);
    assert_eq!(next(&mut signals).await, HookSignal::Lost);
    assert_eq!(next(&mut signals).await, HookSignal::Unreachable);

    // Asked while nobody listens: dropped, not replayed to the next hook.
    hook.send(protocol::frame(&[Command::Stop]));

    let fake = FakeHook::bind(&endpoint).unwrap();
    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    assert_eq!(next(&mut signals).await, greeting_holding_nothing());
    assert_eq!(event(&mut signals).await, Event::Stopped);
    assert_eq!(
        fake.received(),
        [
            Command::Hello {
                protocol: protocol::PROTOCOL
            },
            Command::Listen
        ],
        "the hook found again is asked who it is, like the first one"
    );
}

#[tokio::test]
async fn an_absent_hook_is_reported_unreachable_and_then_every_five_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let (_hook, mut signals) = HookClient::spawn(endpoint(&dir));

    assert_eq!(next(&mut signals).await, HookSignal::Unreachable);
    // Not at every attempt (one every 100 ms): the next one comes ~14 attempts later.
    assert!(
        tokio::time::timeout(Duration::from_millis(500), signals.recv())
            .await
            .is_err(),
        "no second notice within half a second"
    );
}

#[cfg(unix)]
/// A hook from before this protocol: it answers nothing this agent says, because it is
/// reading JSON as XML and finding no commands in it. The agent must not sit there
/// talking to it — an upgrade can leave one running (D5), holding the mice, with the
/// new agent unable to say a word it understands.
#[tokio::test]
async fn a_hook_that_speaks_another_protocol_is_retired_not_talked_to() {
    use lbm_ipc::framing::{read_frame, write_frame};

    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);

    // A "hook" that only ever answers Hello with a protocol from another age.
    let listener = tokio::net::UnixListener::bind(&endpoint).unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorder = heard.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let recorder = recorder.clone();
            tokio::spawn(async move {
                let (mut reader, mut writer) = tokio::io::split(stream);
                while let Ok(frame) = read_frame(&mut reader).await {
                    recorder.lock().unwrap().push(frame.clone());
                    if frame.contains("Hello") {
                        let answer = protocol::event(&Event::Hello {
                            protocol: protocol::PROTOCOL + 99,
                            version: "ancient".to_owned(),
                            layout: String::new(),
                        });
                        let _ = write_frame(&mut writer, &answer).await;
                    }
                }
            });
        }
    });

    let (_hook, mut signals) = HookClient::spawn(endpoint.clone());

    // It connects, is told it cannot drive this one, and says goodbye the old way.
    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    assert_eq!(next(&mut signals).await, HookSignal::Lost);

    let farewell = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if heard
                .lock()
                .unwrap()
                .iter()
                .any(|f| f == protocol::LEGACY_QUIT)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(farewell.is_ok(), "the old hook was never told to leave");
}

#[cfg(unix)]
/// The shape a real upgrade leaves behind: a hook that predates this protocol reads the
/// opening frame as XML, finds no commands in it, and answers **nothing**. The agent must
/// not sit there talking to it — it is holding the mice, and nothing else can stop it.
#[tokio::test]
async fn a_hook_that_answers_nothing_at_all_is_retired_too() {
    use lbm_ipc::framing::read_frame;

    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);

    let listener = tokio::net::UnixListener::bind(&endpoint).unwrap();
    let heard = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorder = heard.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let recorder = recorder.clone();
            tokio::spawn(async move {
                let (mut reader, _writer) = tokio::io::split(stream);
                // Reads, understands nothing, says nothing.
                while let Ok(frame) = read_frame(&mut reader).await {
                    recorder.lock().unwrap().push(frame);
                }
            });
        }
    });

    let (_hook, mut signals) = HookClient::spawn(endpoint.clone());
    assert_eq!(next(&mut signals).await, HookSignal::Connected);

    // Nothing comes back, so the deadline is what decides. Waiting it out is the
    // test: a paused clock would prove the branch exists, not that it fires.
    // Read the signal directly: the shared helper's own patience is the deadline
    // being waited on, so going through it would race what is being tested.
    let lost = tokio::time::timeout(Duration::from_secs(20), signals.recv())
        .await
        .expect("the agent gave up on the silent hook")
        .expect("the connection task is alive");
    assert_eq!(lost, HookSignal::Lost);
    // The farewell goes out on a connection of its own, which the server accepts on
    // its own schedule: wait for it rather than for the scheduler.
    let farewell = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if heard
                .lock()
                .unwrap()
                .iter()
                .any(|f| f == protocol::LEGACY_QUIT)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(farewell.is_ok(), "the silent hook was never told to leave");
}

/// The hook greets before it answers anything else, and what it says about the layout
/// it holds reaches the runtime — that is what lets an agent reattach to a hook that
/// outlived it (D5) without recapturing the mice for a layout that was already right.
#[tokio::test]
async fn the_greeting_reaches_the_runtime_with_the_layout_the_hook_holds() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let fake = FakeHook::bind(&endpoint).unwrap();
    let (hook, mut signals) = HookClient::spawn(endpoint.clone());

    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    // Nothing loaded: it holds no layout, and the greeting still arrives — before the
    // state, because the opening frame asks Hello then Listen.
    assert_eq!(
        next(&mut signals).await,
        HookSignal::Greeted {
            layout: String::new()
        }
    );
    assert_eq!(event(&mut signals).await, Event::Stopped);

    hook.send(protocol::frame(&[
        Command::Load {
            zones: ZONES.into(),
        },
        Command::Run,
    ]));
    assert!(matches!(event(&mut signals).await, Event::Loaded { .. }));
    assert_eq!(event(&mut signals).await, Event::Running);

    // A second agent arrives at a hook that is already running: it is told what that
    // hook holds, and it is exactly what the first agent sent.
    let (_second, mut theirs) = HookClient::spawn(endpoint);
    assert_eq!(next(&mut theirs).await, HookSignal::Connected);
    assert_eq!(
        next(&mut theirs).await,
        HookSignal::Greeted {
            layout: protocol::fingerprint(ZONES)
        }
    );

    drop(fake);
}
