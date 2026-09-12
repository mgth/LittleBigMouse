//! The agent's Windows pipes: the hook's, which it connects to, and its own, which the
//! frontends connect to.
//!
//! Both are per logon session (`lbm_ipc::endpoint`), so two users of one machine each
//! have theirs. The agent's own pipe is secured as the hook's (`lbm-hook`, `ipc/server.rs`):
//! SYSTEM and the current user only, no remote client, no client from another session.
//! One more thing on the agent's (the plan's "étiquette d'intégrité sur le pipe quand
//! l'agent est élevé"): an elevated agent's pipe would carry its high integrity label,
//! and a frontend that is not elevated could not write to it — the pipe is labelled
//! medium instead.

use std::io;
use std::os::windows::io::AsRawHandle;

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, BOOL, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken};

/// This process's logon session.
pub fn session_id() -> io::Result<u32> {
    let mut session = 0;
    // SAFETY: plain query on this process, into a local.
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }
        .map_err(io::Error::other)?;
    Ok(session)
}

/// The hook's pipe in this session (what C#'s `LocalIpcClient` connects to).
pub fn hook_pipe() -> io::Result<String> {
    Ok(lbm_ipc::endpoint::pipe_name(session_id()?))
}

/// The agent's own pipe in this session.
pub fn agent_pipe() -> io::Result<String> {
    Ok(lbm_ipc::endpoint::agent_pipe_name(session_id()?))
}

/// Whether this process runs elevated (C#: `Environment.IsPrivilegedProcess`).
pub fn is_elevated() -> bool {
    with_token(|token| {
        let mut elevation = TOKEN_ELEVATION::default();
        let mut length = 0;
        // SAFETY: the buffer is a TOKEN_ELEVATION of the size given.
        unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut length,
            )
        }
        .map_err(io::Error::other)?;
        Ok(elevation.TokenIsElevated != 0)
    })
    .unwrap_or(false)
}

/// The descriptor of the agent's pipe, in SDDL: the hook's DACL, and a medium integrity
/// label when the agent is elevated.
pub fn pipe_sddl(user_sid: &str, elevated: bool) -> String {
    let dacl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{user_sid})");
    if elevated {
        // No-write-up at medium: a medium frontend writes, a low one does not.
        format!("{dacl}S:(ML;;NW;;;ME)")
    } else {
        dacl
    }
}

/// One instance of the agent's pipe; `first`: the one that claims the name (refused if
/// another process holds it).
pub fn create_pipe(name: &str, first: bool) -> io::Result<NamedPipeServer> {
    let sddl = HSTRING::from(pipe_sddl(&current_user_sid()?, is_elevated()));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: a valid SDDL string in, a descriptor out that is freed below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            &sddl,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(io::Error::other)?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: BOOL(0),
    };
    // SAFETY: the attributes and their descriptor outlive the call.
    let pipe = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
    };
    // SAFETY: allocated by the conversion above, used by nobody any more.
    unsafe {
        let _ = LocalFree(HLOCAL(descriptor.0));
    }
    pipe
}

/// Whether the client of a connected pipe is in this session.
pub fn client_is_current_session(pipe: &NamedPipeServer) -> bool {
    let handle = HANDLE(pipe.as_raw_handle());
    let (mut client_pid, mut client_session) = (0, 0);
    // SAFETY: queries on a connected pipe handle and a pid, into locals.
    unsafe { GetNamedPipeClientProcessId(handle, &mut client_pid) }.is_ok()
        && unsafe { ProcessIdToSessionId(client_pid, &mut client_session) }.is_ok()
        && session_id().is_ok_and(|session| session == client_session)
}

fn current_user_sid() -> io::Result<String> {
    with_token(|token| {
        let mut length = 0;
        // SAFETY: a size query (no buffer).
        let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut length) };
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0u8; length as usize];
        // SAFETY: the buffer has the size the query asked for.
        unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                length,
                &mut length,
            )
        }
        .map_err(io::Error::other)?;
        // SAFETY: the buffer holds a TOKEN_USER followed by its SID.
        let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        let mut text = PWSTR::null();
        // SAFETY: a valid SID in, a string out that is freed below.
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }.map_err(io::Error::other)?;
        // SAFETY: a NUL-terminated wide string from the conversion.
        let sid = unsafe { text.to_string() }.map_err(io::Error::other);
        // SAFETY: allocated by the conversion, used by nobody any more.
        unsafe {
            let _ = LocalFree(HLOCAL(text.0.cast()));
        }
        sid
    })
}

/// Runs `f` on this process's token, closed afterwards.
fn with_token<T>(f: impl FnOnce(HANDLE) -> io::Result<T>) -> io::Result<T> {
    let mut token = HANDLE::default();
    // SAFETY: opens this process's own token for query.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(io::Error::other)?;
    let result = f(token);
    // SAFETY: the token opened above.
    unsafe {
        let _ = CloseHandle(token);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pipe_is_the_users_and_medium_when_elevated() {
        assert_eq!(
            pipe_sddl("S-1-5-21-1-2-3-1001", false),
            "D:P(A;;GA;;;SY)(A;;GA;;;S-1-5-21-1-2-3-1001)"
        );
        assert_eq!(
            pipe_sddl("S-1-5-21-1-2-3-1001", true),
            "D:P(A;;GA;;;SY)(A;;GA;;;S-1-5-21-1-2-3-1001)S:(ML;;NW;;;ME)"
        );
    }

    #[test]
    fn this_session_has_its_pipes() {
        let session = session_id().unwrap();
        assert_eq!(
            hook_pipe().unwrap(),
            format!(r"\\.\pipe\LittleBigMouse-v1-session-{session}")
        );
        assert_eq!(
            agent_pipe().unwrap(),
            format!(r"\\.\pipe\LittleBigMouse-Agent-v1-session-{session}")
        );
        assert!(current_user_sid().unwrap().starts_with("S-1-"));
    }
}
