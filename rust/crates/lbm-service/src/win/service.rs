//! The service: registered with the Service Control Manager, it follows the session
//! at the console and keeps at most one injector, in that session.
//!
//! One thread decides ([`Supervisor`]); everything else reaches it as an [`Event`]:
//! the control handler's session changes and stop, an injector's exit
//! ([`launch::Injector::notify_exit`]). Session changes arrive as
//! `SERVICE_CONTROL_SESSIONCHANGE`, the service's form of
//! `WTSRegisterSessionNotification`; nothing is polled.
//!
//! The input stream comes from the approved masters ([`crate::network`], on a runtime
//! thread of its own): its first batch asks for an injector, each batch is written to
//! the injector's pipe, and its end is written as a `Leave`, which makes the injector
//! release what is pressed. An injector that is lost mid-stream is said to the master
//! (`InputReset`), whose pointer comes home. While the service listens, its port and
//! fingerprint are published for the agent of the session ([`super::offer`]).

use std::ffi::OsString;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
    ServiceType, SessionChangeReason,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use super::launch;
use crate::network::{self, Network, Stream};
use crate::supervisor::{Action, Supervisor};
use crate::writer::Writer;
use crate::SERVICE_NAME;

/// How long an injector whose input ended may take to release and leave.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// What the deciding thread hears.
pub enum Event {
    /// The console may have moved to another session.
    SessionChanged,
    /// The injector of this launch ended.
    Exited(u64),
    /// `lbm-service trust`/`untrust` changed the approved masters.
    TrustChanged,
    /// What the network heard.
    Net(Stream),
    Stop,
}

/// The SCM's entry point: hands this thread to the dispatcher.
pub fn run() -> Result<(), String> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .map_err(|e| format!("service_dispatcher::start: {e}"))
}

windows_service::define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(error) = serve() {
        eprintln!("[lbm-service] {error}");
    }
}

fn serve() -> Result<(), String> {
    let (events, heard) = mpsc::channel::<Event>();
    let control = events.clone();
    let handler = move |request| match request {
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = control.send(Event::Stop);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::SessionChange(change) => {
            if moves_the_console(change.reason) {
                let _ = control.send(Event::SessionChanged);
            }
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::UserEvent(code) if code.to_raw() == crate::CONTROL_RELOAD_TRUST => {
            let _ = control.send(Event::TrustChanged);
            ServiceControlHandlerResult::NoError
        }
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let status = service_control_handler::register(SERVICE_NAME, handler)
        .map_err(|e| format!("register the control handler: {e}"))?;

    let report = |state, accepted, code| {
        status.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accepted,
            exit_code: ServiceExitCode::Win32(code),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
    };

    // Refusing to start is the answer to a service installed where others can
    // replace what it would run as SYSTEM.
    let injector = match super::trusted_injector() {
        Ok(injector) => injector,
        Err(error) => {
            eprintln!("[lbm-service] {error}");
            // ERROR_ACCESS_DENIED: what the SCM shows the administrator.
            let _ = report(ServiceState::Stopped, ServiceControlAccept::empty(), 5);
            return Err(error);
        }
    };

    report(
        ServiceState::Running,
        ServiceControlAccept::STOP
            | ServiceControlAccept::SHUTDOWN
            | ServiceControlAccept::SESSION_CHANGE,
        0,
    )
    .map_err(|e| format!("report running: {e}"))?;

    decide(&injector, &events, &heard);

    report(ServiceState::Stopped, ServiceControlAccept::empty(), 0)
        .map_err(|e| format!("report stopped: {e}"))?;
    Ok(())
}

/// The reasons after which the session at the console may be another one. A lock or
/// unlock does not change it: the lock screen is the same session's secure desktop.
fn moves_the_console(reason: SessionChangeReason) -> bool {
    use SessionChangeReason::*;
    matches!(
        reason,
        ConsoleConnect | ConsoleDisconnect | SessionLogon | SessionLogoff
    )
}

/// The deciding loop, until `Stop`.
fn decide(injector: &Path, events: &mpsc::Sender<Event>, heard: &mpsc::Receiver<Event>) {
    let mut supervisor = Supervisor::new(launch::console_session());
    let mut running: Option<Active> = None;
    // The link whose stream the injector carries now.
    let mut driving: Option<u64> = None;
    let (exits, exited) = mpsc::channel::<u64>();
    // Exits come on their own channel, forwarded as events, so the wait callbacks
    // never hold the deciding thread's sender.
    let forward = events.clone();
    std::thread::spawn(move || {
        for launch in exited {
            if forward.send(Event::Exited(launch)).is_err() {
                break;
            }
        }
    });
    let network = start_network(events.clone());

    for event in heard {
        match event {
            Event::Stop => break,
            Event::SessionChanged => {
                if let Some(Action::Stop { .. }) =
                    supervisor.console_changed(launch::console_session())
                {
                    if let Some(stopped) = running.take() {
                        stopped.stop(STOP_GRACE);
                    }
                    // The master's pointer comes home: the screen it drove is gone.
                    if let (Some(network), Some(link)) = (&network, driving.take()) {
                        network.reset(link);
                    }
                }
            }
            Event::TrustChanged => {
                if let Some(network) = &network {
                    network.trust(read_trust());
                }
            }
            Event::Exited(launch) => {
                // Only the exit of the injector running now: a late one of a replaced
                // injector, even of the same session, leaves the new one alone.
                if supervisor.exited(launch) {
                    if let Some(gone) = running.take() {
                        gone.stop(Duration::ZERO);
                    }
                    if let (Some(network), Some(link)) = (&network, driving.take()) {
                        network.reset(link);
                    }
                }
            }
            Event::Net(Stream::Listening(port)) => match (port, &network) {
                (Some(port), Some(network)) => {
                    if let Err(error) = super::offer::publish(port, &network.machine_id) {
                        eprintln!("[lbm-service] {error}");
                    }
                }
                _ => super::offer::withdraw(),
            },
            Event::Net(Stream::Batch { link, batch }) => {
                if running.is_none() {
                    if let Some(Action::Launch { session, launch }) = supervisor.wanted() {
                        running = start_injector(injector, session, launch, &exits);
                        if running.is_none() {
                            supervisor.launch_failed(launch);
                        }
                    }
                }
                let written = running.as_ref().is_some_and(|r| r.writer.send(&batch));
                if written {
                    driving = Some(link);
                } else {
                    // No injector, or one that stopped reading: the session is over here.
                    // (A queue full is an injector that stopped reading: its session ends
                    // here, rather than the loop that serves the SCM waiting on it.)
                    if let Some(gone) = running.take() {
                        supervisor.exited(gone.launch);
                        gone.stop(STOP_GRACE);
                    }
                    driving = None;
                    if let Some(network) = &network {
                        network.reset(link);
                    }
                }
            }
            Event::Net(Stream::Ended { link }) => {
                if driving == Some(link) {
                    driving = None;
                    // A `Leave` the queue has no room for must not leave a key down: the
                    // injector is stopped instead, and the end of its input releases all.
                    if running.as_ref().is_some_and(|r| !r.writer.end()) {
                        if let Some(gone) = running.take() {
                            supervisor.exited(gone.launch);
                            gone.stop(STOP_GRACE);
                        }
                    }
                }
            }
        }
    }
    drop(network);
    super::offer::withdraw();
    if let Some(stopped) = running.take() {
        stopped.stop(STOP_GRACE);
    }
}

/// The network, under the service's identity, on a runtime of its own. `None` when the
/// folder that holds the identity cannot be trusted: then no master is ever admitted.
struct Running {
    handle: Network,
    machine_id: String,
    _runtime: tokio::runtime::Runtime,
}

impl std::ops::Deref for Running {
    type Target = Network;
    fn deref(&self) -> &Network {
        &self.handle
    }
}

fn start_network(events: mpsc::Sender<Event>) -> Option<Running> {
    let started = (|| {
        let folder = super::store::ready_folder()?;
        let identity = lbm_net::identity::Identity::load_or_create(&folder)
            .map_err(|e| format!("the service's identity: {e}"))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| format!("a runtime for the network: {e}"))?;
        let machine_id = identity.fingerprint().to_owned();
        let (out, heard) = mpsc::channel();
        let (handle, serving) = network::start(
            std::net::SocketAddr::from(([0, 0, 0, 0], network::PORT)),
            identity,
            network::hello(&computer_name()),
            lbm_net::link::Options::default(),
            read_trust(),
            out,
        );
        runtime.spawn(serving);
        std::thread::spawn(move || {
            for stream in heard {
                if events.send(Event::Net(stream)).is_err() {
                    break;
                }
            }
        });
        Ok::<_, String>(Running {
            handle,
            machine_id,
            _runtime: runtime,
        })
    })();
    match started {
        Ok(running) => {
            eprintln!("[lbm-service] the service is {}", running.machine_id);
            Some(running)
        }
        Err(error) => {
            eprintln!("[lbm-service] no network: {error}");
            None
        }
    }
}

fn start_injector(
    injector: &Path,
    session: u32,
    launch: u64,
    exits: &mpsc::Sender<u64>,
) -> Option<Active> {
    match launch::launch(injector, session, |line| eprintln!("[lbm-inject] {line}")) {
        Ok(mut started) => {
            eprintln!(
                "[lbm-service] injector {} in session {session}",
                started.pid
            );
            // Without its exit watched, an injector that leaves would never be
            // launched again in this session: a failed watch is a failed start.
            match started.notify_exit(launch, exits.clone()) {
                Ok(()) => match started
                    .input
                    .try_clone()
                    .inspect_err(|e| eprintln!("[lbm-service] the injector's pipe: {e}"))
                    .ok()
                    .map(Writer::start)
                {
                    Some(writer) => Some(Active {
                        launch,
                        injector: started,
                        writer,
                    }),
                    None => {
                        started.stop(STOP_GRACE);
                        None
                    }
                },
                Err(error) => {
                    eprintln!("[lbm-service] {error}");
                    started.stop(STOP_GRACE);
                    None
                }
            }
        }
        Err(error) => {
            eprintln!("[lbm-service] no injector in session {session}: {error}");
            None
        }
    }
}

/// The injector running, and what writes to its pipe.
struct Active {
    launch: u64,
    injector: launch::Injector,
    writer: Writer,
}

impl Active {
    /// The writer goes first, so that its copy of the pipe closes with the injector's:
    /// the injector reads the end of its stream and releases what is pressed.
    fn stop(self, grace: Duration) {
        drop(self.writer);
        self.injector.stop(grace);
    }
}

/// This computer's name, for the masters' logs.
fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows".into())
}

/// The approved masters; none when the store cannot be trusted, which is said.
fn read_trust() -> crate::trust::TrustStore {
    match super::store::read() {
        Ok(store) => {
            eprintln!("[lbm-service] {} approved master(s)", store.masters().len());
            store
        }
        Err(error) => {
            eprintln!("[lbm-service] {error}");
            crate::trust::TrustStore::default()
        }
    }
}
