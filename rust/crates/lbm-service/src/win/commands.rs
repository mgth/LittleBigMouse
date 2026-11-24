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
    notify_service();
    Ok(())
}

pub fn untrust(machine_id: &str) -> Result<(), String> {
    if store::edit(|trusted| trusted.untrust(machine_id))? {
        println!("withdrawn");
        notify_service();
    } else {
        println!("not trusted");
    }
    Ok(())
}

pub fn list() -> Result<(), String> {
    for master in store::read()?.masters() {
        println!("{}  {}", master.machine_id, master.name);
    }
    Ok(())
}

/// Tells a running service to read the store again. Nothing to tell when it is not
/// installed or not running: it reads the store when it starts.
fn notify_service() {
    let notified = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .and_then(|manager| manager.open_service(SERVICE_NAME, ServiceAccess::USER_DEFINED_CONTROL))
        .and_then(|service| {
            let code =
                UserEventCode::from_raw(CONTROL_RELOAD_TRUST).expect("128 is a user-defined code");
            service.notify(code).map(|_| ())
        });
    if let Err(error) = notified {
        eprintln!("the service was not told ({error}); it reads the store when it starts");
    }
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
