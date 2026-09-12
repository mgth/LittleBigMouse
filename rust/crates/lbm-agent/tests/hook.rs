//! The connection to the hook, against the fake hook over a real local endpoint.

use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::{HookClient, HookSignal};
use lbm_ipc::client::{self, DaemonEvent};
use lbm_ipc::protocol::{self, Command};
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

async fn event(signals: &mut UnboundedReceiver<HookSignal>) -> DaemonEvent {
    match next(signals).await {
        HookSignal::Message(message) => message.event,
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
    // Subscribing is answered with the hook's state: no layout yet.
    assert_eq!(event(&mut signals).await, DaemonEvent::Stopped);

    // Load and Run in one frame, as C# sends them.
    hook.send(client::messages(&[client::load(ZONES), client::run()]));
    assert_eq!(event(&mut signals).await, DaemonEvent::Loaded);
    assert_eq!(event(&mut signals).await, DaemonEvent::Running);
    assert!(fake.hooked());
    assert_eq!(
        fake.received(),
        [Command::Listen, Command::Load(ZONES.into()), Command::Run]
    );

    // What the hook reports on its own reaches the client too.
    fake.broadcast(protocol::DISPLAY_CHANGED);
    assert_eq!(event(&mut signals).await, DaemonEvent::DisplayChanged);

    hook.send(client::messages(&[client::stop()]));
    assert_eq!(event(&mut signals).await, DaemonEvent::Stopped);
    assert!(!fake.hooked());
}

#[tokio::test]
async fn a_hook_that_goes_away_is_signalled_and_found_again() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&dir);
    let fake = FakeHook::bind(&endpoint).unwrap();
    let (hook, mut signals) = HookClient::spawn(endpoint.clone());
    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    assert_eq!(event(&mut signals).await, DaemonEvent::Stopped);

    drop(fake);
    assert_eq!(next(&mut signals).await, HookSignal::Lost);
    assert_eq!(next(&mut signals).await, HookSignal::Unreachable);

    // Asked while nobody listens: dropped, not replayed to the next hook.
    hook.send(client::messages(&[client::stop()]));

    let fake = FakeHook::bind(&endpoint).unwrap();
    assert_eq!(next(&mut signals).await, HookSignal::Connected);
    assert_eq!(event(&mut signals).await, DaemonEvent::Stopped);
    assert_eq!(fake.received(), [Command::Listen]);
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
