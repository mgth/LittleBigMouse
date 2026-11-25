//! Where the approved masters live, and how they are written
//! (docs/multi-pc-service-windows.md, § 1).
//!
//! `%ProgramData%\LittleBigMouse\service\trusted-masters.json`. The folder is made with
//! a DACL of its own, inheriting nothing: SYSTEM and Administrators, full control, no
//! one else, not even to read. Before every read and every write the folder, all it
//! holds and the folders above are checked with the same rules as the service's own
//! folder ([`lbm_admin_only::is_admin_only_folder`]): a store anyone else could have
//! written is no store, and the service then trusts no master. A folder someone else
//! made first (`%ProgramData%` lets every user create one) fails that check too, and
//! nothing is written into it.

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::CreateDirectoryW;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_ProgramData, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

use crate::trust::{self, TrustStore};

/// Owner Administrators; protected (`P`); inheritable to files and folders below:
/// SYSTEM and Administrators.
const FOLDER_SDDL: PCWSTR = w!("O:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)");

/// The service's folder.
pub fn folder() -> Result<PathBuf, String> {
    // SAFETY: the returned string is freed below, after being copied.
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_ProgramData, KF_FLAG_DEFAULT, None) }
        .map_err(|e| format!("%ProgramData%: {e}"))?;
    // SAFETY: a NUL-terminated string from the shell.
    let text = unsafe { path.to_string() };
    // SAFETY: allocated by SHGetKnownFolderPath.
    unsafe { CoTaskMemFree(Some(path.0 as *const _)) };
    let program_data = text.map_err(|e| format!("%ProgramData%: {e}"))?;
    Ok(Path::new(&program_data)
        .join("LittleBigMouse")
        .join("service"))
}

fn protected(folder: &Path) -> Result<(), String> {
    if lbm_admin_only::is_admin_only_folder(folder) {
        Ok(())
    } else {
        Err(format!(
            "{} can be changed by others than administrators (made by someone else?): \
             no master is trusted from it; remove the folder and approve again",
            folder.display()
        ))
    }
}

/// The approved masters, as the service may trust them.
pub fn read() -> Result<TrustStore, String> {
    trust::read_in(&folder()?, &protected)
}

/// Changes the store, making its folder first if there is none. Elevated only: the
/// folder refuses everyone else.
pub fn edit<R>(change: impl FnOnce(&mut TrustStore) -> Result<R, String>) -> Result<R, String> {
    let folder = made_folder()?;
    trust::edit_in(&folder, &protected, change)
}

/// The service's folder, made with its protected DACL if there is none, and checked:
/// where the service keeps its identity as well as the store.
pub fn ready_folder() -> Result<PathBuf, String> {
    let folder = made_folder()?;
    protected(&folder)?;
    Ok(folder)
}

fn made_folder() -> Result<PathBuf, String> {
    let folder = folder()?;
    if let Some(parent) = folder.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    if !folder.exists() {
        create_protected(&folder)?;
    }
    Ok(folder)
}

fn create_protected(folder: &Path) -> Result<(), String> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: a constant SDDL string; the descriptor is freed below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            FOLDER_SDDL,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(|e| format!("security descriptor: {e}"))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let wide: Vec<u16> = folder.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: a NUL-terminated path and attributes alive for the call.
    let made = unsafe { CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(&attributes)) };
    // SAFETY: allocated by the conversion above.
    unsafe {
        let _ = LocalFree(HLOCAL(descriptor.0));
    }
    made.map_err(|e| format!("{}: {e}", folder.display()))
}
