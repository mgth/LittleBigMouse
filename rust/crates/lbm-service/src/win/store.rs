//! Where the approved masters live, and how they are written
//! (docs/multi-pc-service-windows.md, § 1).
//!
//! `%ProgramData%\LittleBigMouse\service\trusted-masters.json`. The folder is made with
//! a DACL of its own, inheriting nothing: SYSTEM and Administrators, full control, no
//! one else, not even to read. Every read checks the file with the same rules as the
//! service's own folder ([`lbm_admin_only`]): a store anyone else could have written is
//! no store, and the service then trusts no master. A folder someone else made first
//! (`%ProgramData%` lets every user create one) fails that check too, which refuses
//! rather than trusts.

use std::io;
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

use crate::trust::{TrustStore, FILE};

/// Protected (`P`), inheritable to files and folders below: SYSTEM and Administrators.
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

/// The approved masters, as the service may trust them. No file is an empty store; a
/// file others than administrators could change is refused.
pub fn read() -> Result<TrustStore, String> {
    let file = folder()?.join(FILE);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(TrustStore::default()),
        Err(error) => return Err(format!("{}: {error}", file.display())),
    };
    if !lbm_admin_only::is_admin_only(&file) {
        return Err(format!(
            "{} can be changed by others than administrators: no master is trusted",
            file.display()
        ));
    }
    TrustStore::parse(&text)
}

/// Writes the store, making its folder first if there is none. Elevated only: the
/// folder refuses everyone else.
pub fn write(store: &TrustStore) -> Result<(), String> {
    let folder = folder()?;
    if let Some(parent) = folder.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    if !folder.is_dir() {
        create_protected(&folder)?;
    }
    let file = folder.join(FILE);
    let staged = folder.join(format!("{FILE}.new"));
    std::fs::write(&staged, store.to_text()).map_err(|e| format!("{}: {e}", staged.display()))?;
    std::fs::rename(&staged, &file).map_err(|e| format!("{}: {e}", file.display()))?;
    if !lbm_admin_only::is_admin_only(&file) {
        return Err(format!(
            "{} is in a folder others than administrators can change (made by someone \
             else?): the service will not trust it; remove the folder and try again",
            folder.display()
        ));
    }
    Ok(())
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
