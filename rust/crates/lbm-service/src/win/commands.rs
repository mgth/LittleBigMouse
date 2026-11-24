//! `lbm-service trust | untrust | list`: what an administrator runs, elevated, to
//! approve a master or withdraw one (docs/multi-pc-service-windows.md, § 1).
//!
//! `trust` asks before writing, in a dialog of its own that shows the name and the
//! fingerprint it was given. Its arguments are not trusted: a process watching for
//! the user's click could start the same command with its own fingerprint, and the
//! user would accept the UAC prompt they were expecting. The dialog is what the user
//! compares with the master's page Machines.

use windows::core::HSTRING;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_SETFOREGROUND, MB_YESNO,
};
use windows_service::service::{ServiceAccess, UserEventCode};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

use super::store;
use crate::trust::{self, Trusted};
use crate::{CONTROL_RELOAD_TRUST, SERVICE_NAME};

pub fn trust(machine_id: &str, name: &str) -> Result<(), String> {
    let machine_id = trust::normalise(machine_id)?;
    let shown = if name.is_empty() { "(no name)" } else { name };
    let text = format!(
        "Allow this computer to drive this one everywhere, including the secure \
         desktop: UAC prompts, the lock screen, and windows running as administrator?\n\n\
         Name: {shown}\nMachine id: {}\n\n\
         Compare the machine id with the one its Machines page shows. Whoever controls \
         that computer will be able to answer this one's UAC prompts.",
        spaced(&machine_id)
    );
    // SAFETY: strings alive for the call; no owner window.
    let answer = unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from("Little Big Mouse"),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2 | MB_SETFOREGROUND,
        )
    };
    if answer != IDYES {
        return Err("not approved".into());
    }
    let done = store::edit(|trusted| trusted.trust(&machine_id, name))?;
    println!(
        "{} {machine_id}",
        match done {
            Trusted::Added => "trusted",
            Trusted::Renamed => "renamed",
        }
    );
    notify_service()
}

/// The service is told even when the master was already gone from the store: a first
/// `untrust` whose notice failed is made good by running it again.
pub fn untrust(machine_id: &str) -> Result<(), String> {
    if store::edit(|trusted| trusted.untrust(machine_id))? {
        println!("withdrawn");
    } else {
        println!("not trusted");
    }
    notify_service()
}

pub fn list() -> Result<(), String> {
    for master in store::read()?.masters() {
        println!("{}  {}", master.machine_id, master.name);
    }
    Ok(())
}

/// Tells a running service to read the store again. Nothing to tell when it is not
/// installed or not running: it reads the store when it starts. A running service that
/// could not be told is an error: it goes on with its older copy, which for a
/// withdrawal means a master still trusted.
fn notify_service() -> Result<(), String> {
    use windows_service::service::ServiceState;
    const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("the service manager: {e}"))?;
    let service = match manager.open_service(
        SERVICE_NAME,
        ServiceAccess::USER_DEFINED_CONTROL | ServiceAccess::QUERY_STATUS,
    ) {
        Ok(service) => service,
        Err(windows_service::Error::Winapi(error))
            if error.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) =>
        {
            return Ok(())
        }
        Err(error) => return Err(format!("the service could not be told: {error}")),
    };
    let running = service
        .query_status()
        .map(|status| status.current_state == ServiceState::Running)
        .map_err(|e| format!("the service could not be told: {e}"))?;
    if !running {
        return Ok(());
    }
    let code = UserEventCode::from_raw(CONTROL_RELOAD_TRUST).expect("128 is a user-defined code");
    service.notify(code).map(|_| ()).map_err(|error| {
        format!(
            "the store changed but the running service was not told ({error}): run the \
             command again, or restart the service"
        )
    })
}

/// `01:23:…`, easier to compare by eye.
fn spaced(machine_id: &str) -> String {
    machine_id
        .as_bytes()
        .chunks(2)
        .map(|pair| String::from_utf8_lossy(pair).into_owned())
        .collect::<Vec<_>>()
        .join(":")
}
