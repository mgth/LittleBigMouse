//! The end of the sharing tools' processes the agent found ([`crate::input_sharing`]),
//! as an event: what was said running is no longer said once it has quit.
//!
//! One wait per process, on the system's own word that it ended — never a look at the
//! process list on a timer:
//!
//! - Linux: a pidfd (`pidfd_open`, Linux 5.3), readable once the process has ended,
//!   registered with the runtime's reactor. Nothing wakes before that.
//! - Windows: `RegisterWaitForSingleObject` on a handle opened with `SYNCHRONIZE`; the
//!   thread pool calls back once the process has ended.
//!
//! Watches follow the last list the agent found: a process no longer in it is let go.
//! A process that could not be watched (a pidfd refused, an elevated process whose
//! handle is denied) stays said until the next look, as before this module.
//!
//! Only an end is an event here: a tool *started* after the last look is seen at the
//! next of the moments the agent looks ([`crate::input_sharing`]); there is no
//! notification of a new process an unprivileged agent can take on either system.

use lbm_ipc::sharing::Running;
use std::collections::HashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// The watches, and where their ends are said (by process id).
pub struct Exits {
    tx: UnboundedSender<u32>,
    rx: Option<UnboundedReceiver<u32>>,
    watches: HashMap<u32, Watch>,
}

impl Default for Exits {
    fn default() -> Self {
        Self::new()
    }
}

impl Exits {
    pub fn new() -> Self {
        let (tx, rx) = unbounded_channel();
        Exits {
            tx,
            rx: Some(rx),
            watches: HashMap::new(),
        }
    }

    /// Where the process ids of the watched processes that ended arrive. Once.
    pub fn take_events(&mut self) -> Option<UnboundedReceiver<u32>> {
        self.rx.take()
    }

    /// Watches the processes of `found`, and only those. Must run inside the tokio
    /// runtime (Linux registers with its reactor).
    pub fn follow(&mut self, found: &[Running]) {
        self.watches
            .retain(|pid, _| found.iter().any(|r| r.pid == *pid));
        for running in found {
            if self.watches.contains_key(&running.pid) {
                continue;
            }
            match Watch::start(running, self.tx.clone()) {
                Start::Watching(watch) => {
                    self.watches.insert(running.pid, watch);
                }
                // Ended between the look and now: said ended at once, nothing to watch.
                Start::Gone => {
                    let _ = self.tx.send(running.pid);
                }
                Start::Refused => eprintln!(
                    "[lbm-agent] cannot watch {} ({}) for its end: said until the next look",
                    running.process, running.pid
                ),
            }
        }
    }

    /// Lets the watch of `pid` go (it ended).
    pub fn forget(&mut self, pid: u32) {
        self.watches.remove(&pid);
    }

    /// How many processes are watched now.
    pub fn watched(&self) -> usize {
        self.watches.len()
    }
}

/// What starting a watch came to.
enum Start {
    Watching(Watch),
    /// No process has that id any more: it ended between the look and the watch.
    Gone,
    /// The process is there, but the system will not let it be waited on.
    Refused,
}

#[cfg(target_os = "linux")]
struct Watch(tokio::task::JoinHandle<()>);

#[cfg(target_os = "linux")]
impl Watch {
    fn start(running: &Running, tx: UnboundedSender<u32>) -> Start {
        use std::os::fd::{FromRawFd, OwnedFd};
        use tokio::io::unix::AsyncFd;
        use tokio::io::Interest;

        let pid = running.pid;
        let Ok(raw) = libc::pid_t::try_from(pid) else {
            return Start::Refused;
        };
        // SAFETY: pidfd_open takes a pid and flags and returns a new descriptor or -1.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, raw, 0) };
        if fd < 0 {
            // ESRCH: already gone between the look and now.
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return Start::Gone;
            }
            return Start::Refused;
        }
        // SAFETY: a descriptor pidfd_open just returned, owned from here on.
        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        // The pid may have been reused between the look and the pidfd: the process it
        // names now must still be the one found, or the one found has ended.
        let same = std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .is_ok_and(|comm| comm.trim_end_matches('\n') == running.process);
        let Ok(fd) = AsyncFd::with_interest(fd, Interest::READABLE) else {
            return Start::Refused;
        };
        Start::Watching(Watch(tokio::spawn(async move {
            if same {
                // Readable once the process has ended; nothing wakes before.
                let _ = fd.readable().await;
            }
            let _ = tx.send(pid);
        })))
    }
}

#[cfg(target_os = "linux")]
impl Drop for Watch {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(windows)]
struct Watch {
    wait: windows::Win32::Foundation::HANDLE,
    process: windows::Win32::Foundation::HANDLE,
    context: *mut Context,
}

#[cfg(windows)]
struct Context {
    pid: u32,
    tx: UnboundedSender<u32>,
}

// SAFETY: the handles are process-wide kernel handles; the context is only read by the
// callback, which `Drop` waits for before freeing it.
#[cfg(windows)]
unsafe impl Send for Watch {}

#[cfg(windows)]
unsafe extern "system" fn ended(
    context: *mut core::ffi::c_void,
    _timed_out: windows::Win32::Foundation::BOOLEAN,
) {
    // SAFETY: the context `start` leaked, freed only after this callback is done.
    let context = unsafe { &*(context as *const Context) };
    let _ = context.tx.send(context.pid);
}

#[cfg(windows)]
impl Watch {
    fn start(running: &Running, tx: UnboundedSender<u32>) -> Start {
        use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, HANDLE};
        use windows::Win32::System::Threading::{
            OpenProcess, RegisterWaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
            WT_EXECUTEONLYONCE,
        };
        // SAFETY: a handle on a pid, for waiting only; closed in Drop or below.
        let process = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, running.pid) } {
            Ok(process) => process,
            // No process has that id any more (an access refusal is ERROR_ACCESS_DENIED).
            Err(e) if e.code() == ERROR_INVALID_PARAMETER.to_hresult() => return Start::Gone,
            Err(_) => return Start::Refused,
        };
        let context = Box::into_raw(Box::new(Context {
            pid: running.pid,
            tx,
        }));
        let mut wait = HANDLE::default();
        // SAFETY: the callback reads `context`, alive until Drop has unregistered.
        let registered = unsafe {
            RegisterWaitForSingleObject(
                &mut wait,
                process,
                Some(ended),
                Some(context as *const _),
                INFINITE,
                WT_EXECUTEONLYONCE,
            )
        };
        if registered.is_err() {
            // SAFETY: nothing else holds either.
            unsafe {
                drop(Box::from_raw(context));
                let _ = CloseHandle(process);
            }
            return Start::Refused;
        }
        Start::Watching(Watch {
            wait,
            process,
            context,
        })
    }
}

#[cfg(windows)]
impl Drop for Watch {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows::Win32::System::Threading::UnregisterWaitEx;
        // SAFETY: INVALID_HANDLE_VALUE makes the call wait for a callback under way, so
        // the context is no longer read when it is freed.
        unsafe {
            let _ = UnregisterWaitEx(self.wait, INVALID_HANDLE_VALUE);
            let _ = CloseHandle(self.process);
            drop(Box::from_raw(self.context));
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
struct Watch;

#[cfg(not(any(target_os = "linux", windows)))]
impl Watch {
    fn start(_running: &Running, _tx: UnboundedSender<u32>) -> Start {
        Start::Refused
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use lbm_ipc::sharing::Tool;
    use std::time::Duration;

    /// A process of this test whose `comm` is `name`: a shell script of that name (Linux
    /// names a script's process after the script, not its interpreter). It says when it
    /// runs, then waits on its stdin, held open by the returned child, until killed.
    /// Not a link to `sleep`: a multi-call `sleep` (uutils) refuses another name.
    fn tool_like(dir: &std::path::Path, name: &str) -> std::process::Child {
        use std::io::BufRead;
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};
        let script = dir.join(name);
        std::fs::write(&script, "#!/bin/sh\necho ready\nread line\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut child = Command::new(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut said = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut said)
            .unwrap();
        assert_eq!(said, "ready\n");
        child
    }

    fn found(child: &std::process::Child, process: &str) -> Running {
        Running {
            tool: Tool::Deskflow,
            process: process.to_owned(),
            pid: child.id(),
            role: None,
            window: true,
            connected: None,
        }
    }

    #[tokio::test]
    async fn the_end_of_a_watched_process_is_said_once_and_nothing_before() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = tool_like(dir.path(), "deskflow");
        let mut exits = Exits::new();
        let mut ended = exits.take_events().unwrap();
        exits.follow(&[found(&child, "deskflow")]);
        assert_eq!(exits.watched(), 1);
        // Running: nothing said.
        assert!(
            tokio::time::timeout(Duration::from_millis(200), ended.recv())
                .await
                .is_err()
        );

        child.kill().unwrap();
        let pid = tokio::time::timeout(Duration::from_secs(5), ended.recv())
            .await
            .expect("the end is said")
            .unwrap();
        assert_eq!(pid, child.id());
        let _ = child.wait();
        exits.forget(pid);
        assert_eq!(exits.watched(), 0);
    }

    /// A process left out of the next look is let go: its end says nothing.
    #[tokio::test]
    async fn a_process_no_longer_found_is_let_go() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = tool_like(dir.path(), "deskflow");
        let mut exits = Exits::new();
        let mut ended = exits.take_events().unwrap();
        exits.follow(&[found(&child, "deskflow")]);
        exits.follow(&[]);
        assert_eq!(exits.watched(), 0);
        child.kill().unwrap();
        let _ = child.wait();
        assert!(
            tokio::time::timeout(Duration::from_millis(300), ended.recv())
                .await
                .is_err()
        );
    }

    /// The pid names another program by the time it is watched (reused, or the look was
    /// wrong): the one found is taken as ended.
    #[tokio::test]
    async fn a_pid_that_names_another_program_is_the_found_one_ended() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = tool_like(dir.path(), "notdeskflow");
        let mut exits = Exits::new();
        let mut ended = exits.take_events().unwrap();
        exits.follow(&[found(&child, "deskflow")]);
        let pid = tokio::time::timeout(Duration::from_secs(5), ended.recv())
            .await
            .expect("said ended")
            .unwrap();
        assert_eq!(pid, child.id());
        child.kill().unwrap();
        let _ = child.wait();
    }

    /// Already gone (and reaped) when watched: said ended at once.
    #[tokio::test]
    async fn a_process_gone_before_its_watch_is_said_ended() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = tool_like(dir.path(), "deskflow");
        let running = found(&child, "deskflow");
        child.kill().unwrap();
        child.wait().unwrap();
        let mut exits = Exits::new();
        let mut ended = exits.take_events().unwrap();
        exits.follow(std::slice::from_ref(&running));
        let pid = tokio::time::timeout(Duration::from_secs(5), ended.recv())
            .await
            .expect("said ended")
            .unwrap();
        assert_eq!(pid, running.pid);
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use lbm_ipc::sharing::Tool;
    use std::time::Duration;

    fn found(pid: u32) -> Running {
        Running {
            tool: Tool::Deskflow,
            process: "cmd.exe".to_owned(),
            pid,
            role: None,
            window: true,
            connected: None,
        }
    }

    #[tokio::test]
    async fn the_end_of_a_watched_process_is_said() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 60 127.0.0.1 >NUL"])
            .spawn()
            .unwrap();
        let mut exits = Exits::new();
        let mut ended = exits.take_events().unwrap();
        exits.follow(&[found(child.id())]);
        assert_eq!(exits.watched(), 1);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), ended.recv())
                .await
                .is_err()
        );
        child.kill().unwrap();
        let pid = tokio::time::timeout(Duration::from_secs(5), ended.recv())
            .await
            .expect("the end is said")
            .unwrap();
        assert_eq!(pid, child.id());
        let _ = child.wait();
    }

    /// Ended, and its last handle closed, before the watch: no process has its id any
    /// more, and it is said ended at once rather than taken for one that refuses.
    #[tokio::test]
    async fn a_process_gone_before_its_watch_is_said_ended() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "exit"])
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        drop(child);
        let mut exits = Exits::new();
        let mut ended = exits.take_events().unwrap();
        exits.follow(&[found(pid)]);
        let said = tokio::time::timeout(Duration::from_secs(5), ended.recv())
            .await
            .expect("said ended")
            .unwrap();
        assert_eq!(said, pid);
    }
}
