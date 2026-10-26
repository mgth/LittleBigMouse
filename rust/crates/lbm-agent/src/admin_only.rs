//! Whether a program may be started elevated from where it lies: only SYSTEM,
//! Administrators and TrustedInstaller can replace it, or anything loaded from its
//! folder — the C# `AdminOnlyPath`, the same rules.
//!
//! An elevated logon task ([`crate::schtask`]) runs its program with full rights and no UAC
//! prompt. If that program sits in a folder the user can write to (a folder chosen at the
//! root of a drive inherits "Authenticated Users: Modify", a portable copy runs from
//! Downloads), any program running as the user, unelevated, can swap the exe or drop a DLL
//! beside it and be started as administrator at the next logon.
//!
//! The check reads the security descriptors: the exe and its folder must grant no write,
//! delete or permission change to anyone else, and no folder above may let anyone else
//! rename or replace what it holds. Anything it cannot read or prove counts as writable;
//! deny entries are ignored, which can only make it stricter. Subfolders are not walked:
//! the installer creates them under the folder, with the rights it inherits.

/// The kind of an access control entry, as far as this check cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AceKind {
    Allow,
    Deny,
    /// An entry this check does not read: counted as granting whatever its mask holds.
    Other,
}

/// One access control entry: who, which rights, whether it applies to the object itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ace {
    pub sid: String,
    pub mask: u32,
    pub kind: AceKind,
    pub inherit_only: bool,
}

/// A security descriptor's owner and DACL; no DACL grants everything to everyone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub owner: Option<String>,
    pub dacl: Option<Vec<Ace>>,
}

// Access mask bits (winnt.h).
const WRITE_DATA: u32 = 0x2; // FILE_WRITE_DATA / FILE_ADD_FILE
const APPEND_DATA: u32 = 0x4; // FILE_APPEND_DATA / FILE_ADD_SUBDIRECTORY
const DELETE_CHILD: u32 = 0x40; // FILE_DELETE_CHILD
const DELETE: u32 = 0x1_0000;
const WRITE_DAC: u32 = 0x4_0000;
const WRITE_OWNER: u32 = 0x8_0000;
const GENERIC_ALL: u32 = 0x1000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;

/// Rights on the exe or its folder that let someone replace what runs.
pub const REPLACE_RIGHTS: u32 = WRITE_DATA
    | APPEND_DATA
    | DELETE_CHILD
    | DELETE
    | WRITE_DAC
    | WRITE_OWNER
    | GENERIC_ALL
    | GENERIC_WRITE;

/// Rights on a folder above that let someone rename the folder below and put another in
/// its place. Creating entries is not one of them: the root of a drive lets every user
/// create folders, which replaces nothing.
pub const RENAME_RIGHTS: u32 = DELETE_CHILD | DELETE | WRITE_DAC | WRITE_OWNER | GENERIC_ALL;

const TRUSTED: &[&str] = &[
    "S-1-5-18",                                                       // SYSTEM
    "S-1-5-32-544",                                                   // BUILTIN\Administrators
    "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464", // NT SERVICE\TrustedInstaller
];

fn trusted(sid: &str) -> bool {
    TRUSTED.iter().any(|t| t.eq_ignore_ascii_case(sid))
}

/// True when a trusted account owns the object and only trusted accounts hold any of
/// `rights` on it. An owner can always rewrite the DACL, so an untrusted owner fails on
/// its own.
pub fn only_trusted_hold(descriptor: &Descriptor, rights: u32) -> bool {
    if !descriptor.owner.as_deref().is_some_and(trusted) {
        return false;
    }
    let Some(dacl) = &descriptor.dacl else {
        return false;
    };
    dacl.iter()
        .filter(|ace| !ace.inherit_only && ace.mask & rights != 0)
        .all(|ace| match ace.kind {
            AceKind::Deny => true,
            AceKind::Allow => trusted(&ace.sid),
            AceKind::Other => false,
        })
}

/// True when `exe`, its folder and every folder above are out of reach of anyone but the
/// trusted accounts. False on any doubt.
#[cfg(windows)]
pub fn is_admin_only(exe: &std::path::Path) -> bool {
    let Some(folder) = exe.parent().filter(|_| exe.is_file()) else {
        return false;
    };
    let holds = |path: &std::path::Path, rights| {
        windows_read::read(path).is_ok_and(|descriptor| only_trusted_hold(&descriptor, rights))
    };
    holds(exe, REPLACE_RIGHTS)
        && holds(folder, REPLACE_RIGHTS)
        && folder
            .ancestors()
            .skip(1)
            .all(|above| holds(above, RENAME_RIGHTS))
}

#[cfg(windows)]
mod windows_read {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        GetAce, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };

    use super::{Ace, AceKind, Descriptor};

    // ACE types (winnt.h) whose layout is ACCESS_ALLOWED_ACE: header, mask, SID.
    const ACCESS_ALLOWED: u8 = 0x0;
    const ACCESS_DENIED: u8 = 0x1;
    const ACCESS_ALLOWED_CALLBACK: u8 = 0x9;
    const ACCESS_DENIED_CALLBACK: u8 = 0xA;
    const INHERIT_ONLY: u8 = 0x8; // INHERIT_ONLY_ACE

    /// The owner and DACL of a file or folder.
    pub fn read(path: &Path) -> io::Result<Descriptor> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut owner = PSID::default();
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: a NUL-terminated path in; the out pointers point into `sd`, freed below.
        unsafe {
            GetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                Some(&mut owner),
                None,
                Some(&mut dacl),
                None,
                &mut sd,
            )
        }
        .ok()
        .map_err(io::Error::other)?;
        let descriptor = (|| {
            let owner = if owner.is_invalid() {
                None
            } else {
                Some(sid_string(owner)?)
            };
            let dacl = if dacl.is_null() {
                None
            } else {
                Some(entries(dacl)?)
            };
            Ok(Descriptor { owner, dacl })
        })();
        // SAFETY: allocated by GetNamedSecurityInfoW, nothing points into it any more.
        unsafe {
            let _ = LocalFree(HLOCAL(sd.0));
        }
        descriptor
    }

    fn entries(dacl: *mut ACL) -> io::Result<Vec<Ace>> {
        // SAFETY: a DACL from GetNamedSecurityInfoW, alive until the descriptor is freed.
        let count = unsafe { (*dacl).AceCount };
        (0..u32::from(count))
            .map(|index| {
                let mut entry = std::ptr::null_mut();
                // SAFETY: an index below AceCount of a valid ACL.
                unsafe { GetAce(dacl, index, &mut entry) }.map_err(io::Error::other)?;
                // SAFETY: every ACE starts with its header.
                let header = unsafe { *(entry as *const ACE_HEADER) };
                let inherit_only = header.AceFlags & INHERIT_ONLY != 0;
                let kind = match header.AceType {
                    ACCESS_ALLOWED | ACCESS_ALLOWED_CALLBACK => AceKind::Allow,
                    ACCESS_DENIED | ACCESS_DENIED_CALLBACK => AceKind::Deny,
                    _ => {
                        return Ok(Ace {
                            sid: String::new(),
                            mask: u32::MAX,
                            kind: AceKind::Other,
                            inherit_only,
                        })
                    }
                };
                let ace = entry as *const ACCESS_ALLOWED_ACE;
                // SAFETY: these four types share ACCESS_ALLOWED_ACE's layout; the SID
                // starts at SidStart, inside the ACE.
                let (mask, sid) = unsafe {
                    (
                        (*ace).Mask,
                        PSID(std::ptr::addr_of!((*ace).SidStart) as *mut _),
                    )
                };
                Ok(Ace {
                    sid: sid_string(sid)?,
                    mask,
                    kind,
                    inherit_only,
                })
            })
            .collect()
    }

    fn sid_string(sid: PSID) -> io::Result<String> {
        let mut text = PWSTR::null();
        // SAFETY: a valid SID in, a string out that is freed below.
        unsafe { ConvertSidToStringSidW(sid, &mut text) }.map_err(io::Error::other)?;
        // SAFETY: a NUL-terminated wide string from the conversion.
        let sid = unsafe { text.to_string() }.map_err(io::Error::other);
        // SAFETY: allocated by the conversion, used by nobody any more.
        unsafe {
            let _ = LocalFree(HLOCAL(text.0.cast()));
        }
        sid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSTEM: &str = "S-1-5-18";
    const ADMINISTRATORS: &str = "S-1-5-32-544";
    const TRUSTED_INSTALLER: &str =
        "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
    const USERS: &str = "S-1-5-32-545";
    const AUTHENTICATED_USERS: &str = "S-1-5-11";
    const CREATOR_OWNER: &str = "S-1-3-0";
    const ALL_APPLICATION_PACKAGES: &str = "S-1-15-2-1";
    const SOME_USER: &str = "S-1-5-21-1004336348-1177238915-682003330-1001";

    const FULL_CONTROL: u32 = 0x1F_01FF;
    const READ_AND_EXECUTE: u32 = 0x12_00A9;
    const MODIFY: u32 = 0x13_01BF;

    fn ace(sid: &str, mask: u32, kind: AceKind, inherit_only: bool) -> Ace {
        Ace {
            sid: sid.to_owned(),
            mask,
            kind,
            inherit_only,
        }
    }

    fn allow(sid: &str, mask: u32) -> Ace {
        ace(sid, mask, AceKind::Allow, false)
    }

    fn program_files() -> Vec<Ace> {
        vec![
            allow(TRUSTED_INSTALLER, FULL_CONTROL),
            allow(SYSTEM, FULL_CONTROL),
            allow(ADMINISTRATORS, FULL_CONTROL),
            allow(USERS, READ_AND_EXECUTE),
            ace(CREATOR_OWNER, GENERIC_ALL, AceKind::Allow, true),
            allow(ALL_APPLICATION_PACKAGES, READ_AND_EXECUTE),
        ]
    }

    fn owned_by(owner: &str, dacl: Vec<Ace>) -> Descriptor {
        Descriptor {
            owner: Some(owner.to_owned()),
            dacl: Some(dacl),
        }
    }

    #[test]
    fn program_files_is_admin_only() {
        assert!(only_trusted_hold(
            &owned_by(TRUSTED_INSTALLER, program_files()),
            REPLACE_RIGHTS
        ));
        assert!(only_trusted_hold(
            &owned_by(ADMINISTRATORS, program_files()),
            REPLACE_RIGHTS
        ));
    }

    #[test]
    fn a_folder_at_the_root_of_a_drive_is_writable() {
        let mut inherited = program_files();
        inherited.push(allow(AUTHENTICATED_USERS, MODIFY));
        let descriptor = owned_by(ADMINISTRATORS, inherited);
        assert!(!only_trusted_hold(&descriptor, REPLACE_RIGHTS));
        assert!(!only_trusted_hold(&descriptor, RENAME_RIGHTS));
    }

    #[test]
    fn the_root_lets_users_create_folders_but_not_replace_them() {
        let root = owned_by(
            TRUSTED_INSTALLER,
            vec![
                allow(ADMINISTRATORS, FULL_CONTROL),
                allow(SYSTEM, FULL_CONTROL),
                allow(USERS, READ_AND_EXECUTE),
                ace(AUTHENTICATED_USERS, MODIFY, AceKind::Allow, true),
                // FILE_ADD_SUBDIRECTORY: create folders, this folder only.
                allow(AUTHENTICATED_USERS, APPEND_DATA),
            ],
        );
        assert!(only_trusted_hold(&root, RENAME_RIGHTS));
        assert!(!only_trusted_hold(&root, REPLACE_RIGHTS));
    }

    #[test]
    fn an_untrusted_owner_can_rewrite_any_dacl() {
        assert!(!only_trusted_hold(
            &owned_by(SOME_USER, program_files()),
            REPLACE_RIGHTS
        ));
        let no_owner = Descriptor {
            owner: None,
            dacl: Some(program_files()),
        };
        assert!(!only_trusted_hold(&no_owner, REPLACE_RIGHTS));
    }

    #[test]
    fn no_dacl_grants_everything() {
        let null_dacl = Descriptor {
            owner: Some(ADMINISTRATORS.to_owned()),
            dacl: None,
        };
        assert!(!only_trusted_hold(&null_dacl, REPLACE_RIGHTS));
        assert!(only_trusted_hold(
            &owned_by(ADMINISTRATORS, Vec::new()),
            REPLACE_RIGHTS
        ));
    }

    #[test]
    fn the_users_own_account_is_not_trusted() {
        // What a profile folder looks like: Downloads, %TEMP%, a portable copy.
        let profile = vec![
            allow(SYSTEM, FULL_CONTROL),
            allow(ADMINISTRATORS, FULL_CONTROL),
            allow(SOME_USER, FULL_CONTROL),
        ];
        assert!(!only_trusted_hold(
            &owned_by(SYSTEM, profile),
            REPLACE_RIGHTS
        ));
    }

    #[test]
    fn deny_entries_are_ignored_and_unread_ones_refuse() {
        let mut denied = program_files();
        denied.push(ace(AUTHENTICATED_USERS, FULL_CONTROL, AceKind::Deny, false));
        assert!(only_trusted_hold(
            &owned_by(ADMINISTRATORS, denied),
            REPLACE_RIGHTS
        ));

        let mut unread = program_files();
        unread.push(ace("", u32::MAX, AceKind::Other, false));
        assert!(!only_trusted_hold(
            &owned_by(ADMINISTRATORS, unread),
            REPLACE_RIGHTS
        ));
    }

    #[test]
    fn reading_rights_alone_is_harmless() {
        let mut readable = program_files();
        readable.push(allow(AUTHENTICATED_USERS, READ_AND_EXECUTE));
        assert!(only_trusted_hold(
            &owned_by(ADMINISTRATORS, readable),
            REPLACE_RIGHTS
        ));
    }

    #[cfg(windows)]
    #[test]
    fn a_file_in_the_users_temp_folder_is_not_admin_only() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let descriptor = windows_read::read(file.path()).unwrap();
        assert!(descriptor.owner.is_some());
        assert!(descriptor.dacl.is_some_and(|dacl| !dacl.is_empty()));
        assert!(!is_admin_only(file.path()));
    }

    #[cfg(windows)]
    #[test]
    fn a_missing_file_is_not_admin_only() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_admin_only(&dir.path().join("lbm-agent.exe")));
    }
}
