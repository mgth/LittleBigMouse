//! The service: registered with the Service Control Manager, it follows the session
//! at the console and keeps at most one injector, in that session.
//!
//! One thread decides ([`Supervisor`]); everything else reaches it as an [`Event`]:
//! the control handler's session changes and stop, an injector's exit
//! ([`launch::Injector::notify_exit`]). Session changes arrive as
//! `SERVICE_CONTROL_SESSIONCHANGE`, the service's form of
//! `WTSRegisterSessionNotification`; nothing is polled.
//!
//! At this step nothing yet asks for an injector: the input stream of approved masters
//! is the next one (docs/multi-pc-service-windows.md, step 4). The service checks
//! where it runs from, follows the console and answers the SCM.

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
use crate::supervisor::{Action, Supervisor};
use crate::SERVICE_NAME;

/// How long an injector whose input ended may take to release and leave.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// What the deciding thread hears.
pub enum Event {
    /// The console may have moved to another session.
    SessionChanged,
    /// A master's stream starts and needs an injector (step 4).
    Wanted,
    /// The injector of this session ended.
    Exited(u32),
    /// `lbm-service trust`/`untrust` changed the approved masters.
    TrustChanged,
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
    // Read now and at every change; the listener of step 4 will serve these.
    let mut trusted = read_trust();
    let mut running: Option<launch::Injector> = None;
    let (exits, exited) = mpsc::channel::<u32>();
    // Exits come on their own channel, forwarded as events, so the wait callbacks
    // never hold the deciding thread's sender.
    let forward = events.clone();
    std::thread::spawn(move || {
        for session in exited {
            if forward.send(Event::Exited(session)).is_err() {
                break;
            }
        }
    });

    for event in heard {
        let action = match event {
            Event::Stop => break,
            Event::SessionChanged => supervisor.console_changed(launch::console_session()),
            Event::Wanted => supervisor.wanted(),
            Event::TrustChanged => {
                trusted = read_trust();
                None
            }
            Event::Exited(session) => {
                supervisor.exited(session);
                if running.as_ref().is_some_and(|i| i.session == session) {
                    if let Some(gone) = running.take() {
                        gone.stop(Duration::ZERO);
                    }
                }
                None
            }
        };
        match action {
            Some(Action::Launch(session)) => match launch::launch(injector, session, None) {
                Ok(mut started) => {
                    eprintln!(
                        "[lbm-service] injector {} in session {session}",
                        started.pid
                    );
                    if let Err(error) = started.notify_exit(exits.clone()) {
                        eprintln!("[lbm-service] {error}");
                    }
                    running = Some(started);
                }
                Err(error) => {
                    eprintln!("[lbm-service] no injector in session {session}: {error}");
                    supervisor.launch_failed(session);
                }
            },
            Some(Action::Stop(session)) => {
                if let Some(stopped) = running.take() {
                    debug_assert_eq!(stopped.session, session);
                    stopped.stop(STOP_GRACE);
                }
            }
            None => {}
        }
    }
    if let Some(stopped) = running.take() {
        stopped.stop(STOP_GRACE);
    }
    drop(trusted);
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
