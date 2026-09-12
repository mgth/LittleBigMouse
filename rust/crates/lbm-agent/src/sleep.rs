//! System sleep on Linux, from logind (the v6 plan: "veille sous Linux : `PrepareForSleep`
//! de logind" — until now only the Windows hook reacted to sleep).
//!
//! The Windows hook sees the display go off and unhooks itself (`Suspended`), then says
//! `Resumed`; the agent then settles the display and re-hooks through the watchdog. On
//! Linux nothing said so: this does. logind announces `PrepareForSleep(true)` before
//! sleeping and `PrepareForSleep(false)` after waking; a *delay* inhibitor lock, held
//! while awake, gives the agent the time to take the hook down before the machine goes
//! (released once it has, re-taken on waking).

use tokio::sync::{mpsc, oneshot};

pub use crate::runtime::SleepSignal;

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    /// A lock on `what` ("sleep"), in `mode` ("delay"): held as long as the returned
    /// descriptor is open.
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;

    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

async fn delay_lock(manager: &ManagerProxy<'_>) -> Option<zbus::zvariant::OwnedFd> {
    match manager
        .inhibit(
            "sleep",
            "LittleBigMouse",
            "Release the mouse before sleeping",
            "delay",
        )
        .await
    {
        Ok(fd) => Some(fd),
        Err(error) => {
            eprintln!("[lbm-agent] no sleep delay lock: {error}");
            None
        }
    }
}

/// Follows logind until `signals` is closed or the bus goes away. Without a system bus
/// (a container, a test runner) it says so and returns: sleep then goes unnoticed, as
/// before.
pub async fn watch(signals: mpsc::UnboundedSender<SleepSignal>) {
    use futures_util::StreamExt;

    let connection = match zbus::Connection::system().await {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("[lbm-agent] no system bus, sleep is not followed: {error}");
            return;
        }
    };
    let manager = match ManagerProxy::new(&connection).await {
        Ok(manager) => manager,
        Err(error) => {
            eprintln!("[lbm-agent] no logind, sleep is not followed: {error}");
            return;
        }
    };
    let mut prepares = match manager.receive_prepare_for_sleep().await {
        Ok(stream) => stream,
        Err(error) => {
            eprintln!("[lbm-agent] cannot follow logind: {error}");
            return;
        }
    };

    let mut lock = delay_lock(&manager).await;
    while let Some(signal) = prepares.next().await {
        let Ok(args) = signal.args() else { continue };
        if *args.start() {
            let (done, done_rx) = oneshot::channel();
            if signals.send(SleepSignal::Starting(done)).is_err() {
                return;
            }
            let _ = done_rx.await;
            // The hook is down: let the machine go.
            lock = None;
        } else {
            if signals.send(SleepSignal::Ended).is_err() {
                return;
            }
            if lock.is_none() {
                lock = delay_lock(&manager).await;
            }
        }
    }
    drop(lock);
}
