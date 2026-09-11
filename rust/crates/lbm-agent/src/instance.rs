//! One agent per user session — the agent's side of the C# UI's `SingleInstanceGuard`
//! (#606). Linux: an exclusive lock on a file in `$XDG_RUNTIME_DIR` (or the temporary
//! directory, per user); Windows: a named mutex in the session's namespace. The lock is
//! the guard: held for the life of the process, gone with it, whatever the way out.

use std::io;
#[cfg(unix)]
use std::path::PathBuf;

/// Held while this agent is the one of the session.
pub struct InstanceLock {
    #[cfg(unix)]
    _file: std::fs::File,
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(unix)]
impl InstanceLock {
    /// Where the lock lives: `$XDG_RUNTIME_DIR/lbm-agent.lock`, else a per-user name in
    /// the temporary directory.
    pub fn default_path() -> PathBuf {
        match std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|d| !d.as_os_str().is_empty() && d.is_dir())
        {
            Some(dir) => dir.join("lbm-agent.lock"),
            // SAFETY: getuid has no preconditions and cannot fail.
            None => {
                std::env::temp_dir().join(format!("lbm-agent-{}.lock", unsafe { libc::getuid() }))
            }
        }
    }

    /// The lock at `path`; `Ok(None)` when another agent holds it.
    pub fn acquire(path: &std::path::Path) -> io::Result<Option<InstanceLock>> {
        use std::os::fd::AsRawFd;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        // SAFETY: the descriptor is valid for the life of `file`.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::EWOULDBLOCK) => Ok(None),
                _ => Err(error),
            };
        }
        Ok(Some(InstanceLock { _file: file }))
    }

    /// The session's lock (see [`default_path`](Self::default_path)).
    pub fn acquire_for_session() -> io::Result<Option<InstanceLock>> {
        Self::acquire(&Self::default_path())
    }
}

#[cfg(windows)]
impl InstanceLock {
    /// The named mutex `name`; `Ok(None)` when another agent created it.
    pub fn acquire_named(name: &str) -> io::Result<Option<InstanceLock>> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;

        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `wide` is NUL-terminated and outlives the call.
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(wide.as_ptr())) }
            .map_err(io::Error::other)?;
        // SAFETY: read right after the call that set it.
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // SAFETY: the handle was just returned and is closed once.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Ok(None);
        }
        Ok(Some(InstanceLock { handle }))
    }

    /// The session's lock: a mutex in the session-local namespace.
    pub fn acquire_for_session() -> io::Result<Option<InstanceLock>> {
        Self::acquire_named(r"Local\LittleBigMouse-Agent")
    }
}

#[cfg(windows)]
impl Drop for InstanceLock {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed once.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C#: `SingleInstanceGuardTests.SecondAcquire_WhileFirstHeld_ReportsAlreadyRunning_ThenReleases`.
    fn a_second_acquire_reports_the_first_then_it_is_free_again(
        acquire: impl Fn() -> Option<InstanceLock>,
    ) {
        let first = acquire().expect("the first agent gets the lock");
        assert!(acquire().is_none(), "a second agent is told one runs");
        drop(first);
        assert!(acquire().is_some(), "released with the first");
    }

    #[cfg(unix)]
    #[test]
    fn a_second_lock_file_acquire_reports_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lbm-agent.lock");
        a_second_acquire_reports_the_first_then_it_is_free_again(|| {
            InstanceLock::acquire(&path).unwrap()
        });
    }

    #[cfg(windows)]
    #[test]
    fn a_second_mutex_acquire_reports_the_first() {
        let name = format!(r"Local\LittleBigMouse-Agent-test-{}", std::process::id());
        a_second_acquire_reports_the_first_then_it_is_free_again(|| {
            InstanceLock::acquire_named(&name).unwrap()
        });
    }
}
