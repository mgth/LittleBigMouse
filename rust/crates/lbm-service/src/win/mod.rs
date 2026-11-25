//! The Windows side: the service ([`service`]), how it launches the injector
//! ([`launch`]), the approved masters ([`store`]) and the commands that change them
//! ([`commands`]).

pub mod commands;
pub mod launch;
pub mod offer;
pub mod service;
pub mod store;

use std::path::{Path, PathBuf};

/// The injector beside this executable, provided that both sit in a folder only
/// SYSTEM, Administrators and TrustedInstaller can change — what the service runs
/// with SYSTEM's rights must not be replaceable by anyone else
/// (`lbm_admin_only`, the check that closed the elevated-autostart hole, #742/#743).
pub fn trusted_injector() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let injector = injector_beside(&exe)?;
    if !lbm_admin_only::is_admin_only(&exe) {
        return Err(format!(
            "{} is in a folder others than administrators can change: refusing to run",
            exe.display()
        ));
    }
    Ok(injector)
}

fn injector_beside(exe: &Path) -> Result<PathBuf, String> {
    let injector = exe
        .parent()
        .ok_or_else(|| format!("{} has no folder", exe.display()))?
        .join(crate::INJECTOR_EXE);
    if injector.is_file() {
        Ok(injector)
    } else {
        Err(format!(
            "no {} beside {}",
            crate::INJECTOR_EXE,
            exe.display()
        ))
    }
}
