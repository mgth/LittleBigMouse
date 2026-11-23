//! The per-user local endpoints this hook serves, and who may reach them.
//!
//! Windows: a current-session named pipe whose DACL grants only the current user
//! and SYSTEM, refusing remote clients and clients from other sessions. Linux: a
//! 0600 Unix-domain socket. The command endpoint and the input endpoint of the
//! multi-PC mode (docs/multi-pc.md) both go through here, so the second endpoint is
//! exactly as private as the first — it injects keystrokes, it must not be less.
//!
//! An [`Acceptor`] only hands out connected streams; what is spoken over them, and
//! what "newest client wins" means, is each server's business.

use std::io;

pub use imp::{default_endpoint, Acceptor};

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::time::Duration;

    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, LocalFree, BOOL, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER,
    };
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcessToken,
    };

    pub type Stream = NamedPipeServer;

    pub struct Acceptor {
        endpoint: String,
        next: Option<NamedPipeServer>,
    }

    impl Acceptor {
        pub fn bind(endpoint: &str) -> io::Result<Self> {
            Ok(Self {
                endpoint: endpoint.to_string(),
                next: Some(create_pipe(endpoint, true)?),
            })
        }

        /// The next client of this user's session. Never gives up: a pipe instance
        /// that cannot be created is retried, so `None` is not an outcome here.
        pub async fn accept(&mut self) -> Option<Stream> {
            loop {
                let pipe = match self.next.take() {
                    Some(first) => first,
                    None => match create_pipe(&self.endpoint, false) {
                        Ok(pipe) => pipe,
                        Err(error) => {
                            // A transient failure (handle pressure, AV
                            // interference) must not kill IPC for the rest of
                            // the daemon's life: log, back off, retry.
                            eprintln!(
                                "[LittleBigMouse.Hook] local IPC accept error: {error}; retrying"
                            );
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    },
                };
                if pipe.connect().await.is_err() {
                    continue;
                }
                if !client_is_current_session(&pipe) {
                    continue;
                }
                return Some(pipe);
            }
        }
    }

    pub fn default_endpoint() -> io::Result<String> {
        Ok(lbm_ipc::endpoint::pipe_name(current_session_id()?))
    }

    fn current_session_id() -> io::Result<u32> {
        let mut session = 0;
        unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) }
            .map_err(io::Error::other)?;
        Ok(session)
    }

    fn client_is_current_session(pipe: &NamedPipeServer) -> bool {
        let handle = HANDLE(pipe.as_raw_handle());
        let mut client_pid = 0;
        let mut client_session = 0;
        unsafe { GetNamedPipeClientProcessId(handle, &mut client_pid) }.is_ok()
            && unsafe { ProcessIdToSessionId(client_pid, &mut client_session) }.is_ok()
            && current_session_id().is_ok_and(|session| session == client_session)
    }

    fn create_pipe(endpoint: &str, first: bool) -> io::Result<NamedPipeServer> {
        let sid = current_user_sid()?;
        let sddl = HSTRING::from(format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})"));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
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
        let result = unsafe {
            ServerOptions::new()
                .first_pipe_instance(first)
                .reject_remote_clients(true)
                .create_with_security_attributes_raw(
                    endpoint,
                    (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
                )
        };
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        result
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = HANDLE::default();
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(io::Error::other)?;

        let result = (|| {
            let mut length = 0;
            let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut length) };
            if length == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut buffer = vec![0u8; length as usize];
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
            let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
            let mut text = PWSTR::null();
            unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }
                .map_err(io::Error::other)?;
            let sid = unsafe { text.to_string() }.map_err(io::Error::other);
            unsafe {
                let _ = LocalFree(HLOCAL(text.0.cast()));
            }
            sid
        })();

        unsafe {
            let _ = CloseHandle(token);
        }
        result
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use tokio::net::{UnixListener, UnixStream};

    pub type Stream = UnixStream;

    pub struct Acceptor {
        listener: UnixListener,
        path: PathBuf,
    }

    impl Acceptor {
        pub fn bind(endpoint: &str) -> io::Result<Self> {
            let path = PathBuf::from(endpoint);
            if let Some(parent) = path.parent() {
                // Only restrict a directory we create ourselves: the normal
                // parents (XDG_RUNTIME_DIR, the LBM data dir) already exist
                // with correct ownership, and chmod on a shared parent like
                // /tmp is EPERM. The socket itself is chmod 0600 below.
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
                }
            }
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let listener = UnixListener::bind(&path)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self { listener, path })
        }

        /// The next client, or `None` once the listener has failed for good.
        pub async fn accept(&mut self) -> Option<Stream> {
            self.listener.accept().await.ok().map(|(stream, _)| stream)
        }
    }

    impl Drop for Acceptor {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    pub fn default_endpoint() -> io::Result<String> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let data = crate::platform::paths::lbm_data_file("");
        lbm_ipc::endpoint::socket_path(runtime.as_deref(), data.as_deref())
            .map(|path| path.to_string_lossy().into_owned())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no per-user runtime directory"))
    }
}
