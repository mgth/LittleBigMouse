//! One of each process per user session — the agent's and the hook's side of the C# UI's
//! `SingleInstanceGuard` (#606). Linux: an exclusive lock on a file in `$XDG_RUNTIME_DIR`
//! (or the temporary directory, per user); Windows: a named mutex in the session's
//! namespace. The lock is the guard: held for the life of the process, gone with it,
//! whatever the way out.
//!
//! Two hooks are worse than two agents: they would both grab the mice, and the second
//! would take the first one's socket with it (a stale socket is unlinked, and a live one
//! looks the same from outside).

use std::io;
#[cfg(unix)]
use std::path::PathBuf;

/// Held while this process is the one of the session.
pub struct InstanceLock {
    #[cfg(unix)]
    _file: std::fs::File,
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

/// Which process a lock is for. The two names are spelled out rather than derived, so
/// that renaming a binary cannot silently let two of it run.
#[derive(Clone, Copy, Debug)]
pub struct InstanceName {
    /// The lock file's name under `$XDG_RUNTIME_DIR`.
    pub file: &'static str,
    /// The session-local mutex on Windows.
    pub mutex: &'static str,
}

pub const AGENT: InstanceName = InstanceName {
    file: "lbm-agent.lock",
    mutex: r"Local\LittleBigMouse-Agent",
};

pub const HOOK: InstanceName = InstanceName {
    file: "lbm-hook.lock",
    mutex: r"Local\LittleBigMouse-Hook",
};

#[cfg(unix)]
impl InstanceLock {
    /// Where the lock lives: `$XDG_RUNTIME_DIR/<file>`, else a per-user name in the
    /// temporary directory.
    pub fn default_path(name: InstanceName) -> PathBuf {
        match std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|d| !d.as_os_str().is_empty() && d.is_dir())
        {
            Some(dir) => dir.join(name.file),
            // SAFETY: getuid has no preconditions and cannot fail.
            None => {
                let stem = name.file.trim_end_matches(".lock");
                std::env::temp_dir().join(format!("{stem}-{}.lock", unsafe { libc::getuid() }))
            }
        }
    }

    /// The lock at `path`; `Ok(None)` when another one of it holds it.
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

    /// The session's lock for `name` (see [`default_path`](Self::default_path)).
    pub fn acquire_for_session(name: InstanceName) -> io::Result<Option<InstanceLock>> {
        Self::acquire(&Self::default_path(name))
    }
}

#[cfg(windows)]
impl InstanceLock {
    /// The named mutex `name`; `Ok(None)` when another one of it created it.
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

    /// The session's lock for `name`: a mutex in the session-local namespace.
    pub fn acquire_for_session(name: InstanceName) -> io::Result<Option<InstanceLock>> {
        Self::acquire_named(name.mutex)
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
        let first = acquire().expect("the first one gets the lock");
        assert!(acquire().is_none(), "a second one is told it already runs");
        drop(first);
        assert!(acquire().is_some(), "released with the first");
    }

    #[test]
    fn the_two_processes_are_named_apart() {
        // They run together — one drives the other — so sharing a lock would mean
        // whichever started second never ran at all.
        assert_ne!(AGENT.file, HOOK.file);
        assert_ne!(AGENT.mutex, HOOK.mutex);
    }

    #[cfg(unix)]
    #[test]
    fn an_agent_and_a_hook_do_not_lock_each_other_out() {
        let dir = tempfile::tempdir().unwrap();

        let agent = InstanceLock::acquire(&dir.path().join(AGENT.file)).unwrap();
        let hook = InstanceLock::acquire(&dir.path().join(HOOK.file)).unwrap();

        assert!(agent.is_some(), "the agent takes its own");
        assert!(hook.is_some(), "and the hook takes its own");
    }

    #[cfg(unix)]
    #[test]
    fn the_lock_falls_back_to_a_per_user_name_without_a_runtime_directory() {
        // Two users on one machine must not share a lock through /tmp.
        let stem = HOOK.file.trim_end_matches(".lock");
        let path = InstanceLock::default_path(HOOK);

        assert!(
            path.ends_with(HOOK.file) || {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                name.starts_with(stem) && name.ends_with(".lock") && name != HOOK.file
            },
            "unexpected lock path {}",
            path.display()
        );
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
