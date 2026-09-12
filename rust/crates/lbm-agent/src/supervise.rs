//! Keeping a hook running (decision D5 of the v6 plan) — the agent's side of what the C#
//! `DaemonProcessManager` did.
//!
//! When no hook answers at the endpoint ([`HookSignal::Unreachable`](crate::hook::HookSignal),
//! about every five seconds), the agent launches one — unless the one it launched is still
//! alive (starting up), another hook of this user is running, or it is backing off from a
//! hook that keeps dying. The launch is **detached**: its own session, its own log, so the
//! hook survives the agent (D5) — a crashed agent leaves the cursor as it was, and the next
//! agent finds the hook still running (`Connected`, then `Running`) and leaves it alone.
//!
//! The hook is told it is driven (`LBM_HOOK_UI=1`): it waits for commands instead of
//! loading `Current.xml` on its own, which it would otherwise decide from its parent's
//! path — a path that named the C# UI and does not name the agent.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Hook executables the agent may find beside itself: the historical Windows staging
/// name first, then the cargo binary name (C#: `HookExeName`, `HookProcessNames`).
#[cfg(windows)]
const HOOK_NAMES: &[&str] = &["LittleBigMouse.Hook.exe", "lbm-hook.exe"];
#[cfg(not(windows))]
const HOOK_NAMES: &[&str] = &["lbm-hook"];

/// Delays between launches of a hook that keeps dying before it could answer: the
/// first retry comes with the next "unreachable" notice (~5 s), later ones wait longer.
const BACKOFF: &[Duration] = &[
    Duration::ZERO,
    Duration::from_secs(10),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

/// What [`HookLauncher::on_unreachable`] did.
#[derive(Debug)]
pub enum Launch {
    /// A hook was started, with this process id.
    Started(u32),
    /// The hook this agent started is still alive (starting up, or not answering).
    Starting,
    /// Another hook of this user runs: not ours to double.
    AlreadyRunning,
    /// Backing off from a hook that keeps dying.
    Waiting,
    /// It could not be started.
    Failed(io::Error),
}

/// Launches the hook when none answers.
pub struct HookLauncher {
    program: PathBuf,
    args: Vec<OsString>,
    log: Option<PathBuf>,
    child: Option<Child>,
    /// Launches since the last successful connection.
    failures: usize,
    last_launch: Option<Instant>,
}

impl HookLauncher {
    /// Launches `program` with `args`; its output goes to `log` (appended) when given.
    pub fn new(program: PathBuf, args: Vec<OsString>, log: Option<PathBuf>) -> Self {
        HookLauncher {
            program,
            args,
            log,
            child: None,
            failures: 0,
            last_launch: None,
        }
    }

    /// The hook beside this executable (a deployed build, and the cargo target
    /// directory in the development tree alike — C#: `FindHookPath`).
    pub fn beside_agent(log: Option<PathBuf>) -> Option<Self> {
        let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        HOOK_NAMES
            .iter()
            .map(|name| dir.join(name))
            .find(|path| path.is_file())
            .map(|program| HookLauncher::new(program, Vec::new(), log))
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The last resort when a Stop cannot be delivered (C#: `StopCurrentSessionDaemons`):
    /// end the hook this agent launched, if it is still alive. Stopping is the safety
    /// operation — a lost connection must not leave the mice captured — and the kernel
    /// releases a dead process's grabs and barriers. Only the instance this agent
    /// started: another one could be anyone's. Returns whether one was ended.
    pub fn stop_launched(&mut self) -> bool {
        let Some(child) = &mut self.child else {
            return false;
        };
        if !matches!(child.try_wait(), Ok(None)) {
            self.child = None;
            return false;
        }
        let ended = child.kill().is_ok() && child.wait().is_ok();
        self.child = None;
        ended
    }

    /// The hook answered: whatever was launched worked, the backoff starts over.
    pub fn on_connected(&mut self) {
        self.failures = 0;
    }

    /// No hook answers: launch one, unless there is a reason not to (see [`Launch`]).
    pub fn on_unreachable(&mut self, now: Instant) -> Launch {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => return Launch::Starting,
                // Gone without ever answering, or since: reaped here.
                _ => self.child = None,
            }
        }
        if another_hook_runs(&self.program) {
            return Launch::AlreadyRunning;
        }
        if let Some(last) = self.last_launch {
            let wait = BACKOFF[self.failures.min(BACKOFF.len() - 1)];
            if now.duration_since(last) < wait {
                return Launch::Waiting;
            }
        }

        self.last_launch = Some(now);
        self.failures += 1;
        match self.spawn() {
            Ok(child) => {
                let id = child.id();
                self.child = Some(child);
                Launch::Started(id)
            }
            Err(error) => Launch::Failed(error),
        }
    }

    fn spawn(&self) -> io::Result<Child> {
        let result = self.spawn_with(true);
        // A job that forbids leaving it (Windows): launched inside it, then — the job's
        // end would take the hook with it, but that is better than no hook.
        #[cfg(windows)]
        if matches!(&result, Err(e) if e.raw_os_error() == Some(ERROR_ACCESS_DENIED)) {
            return self.spawn_with(false);
        }
        result
    }

    fn spawn_with(&self, leave_job: bool) -> io::Result<Child> {
        let output = |log: &Option<PathBuf>| match log {
            Some(path) => OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map(Stdio::from)
                .unwrap_or_else(|_| Stdio::null()),
            None => Stdio::null(),
        };
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .env("LBM_HOOK_UI", "1")
            .stdin(Stdio::null())
            .stdout(output(&self.log))
            .stderr(output(&self.log));
        detach(&mut command, leave_job);
        command.spawn()
    }
}

/// Its own session (Unix) or process group without a console (Windows): a signal or a
/// closed terminal that ends the agent does not reach the hook.
#[cfg(unix)]
fn detach(command: &mut Command, _leave_job: bool) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid is async-signal-safe and only affects the child being started.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// On Windows, also out of the agent's job when `leave_job`: an agent started by a
/// scheduled task runs in the task's job, and a stopped task ends every process of its
/// job — the hook would not survive the agent (D5).
#[cfg(windows)]
fn detach(command: &mut Command, leave_job: bool) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let breakaway = if leave_job {
        CREATE_BREAKAWAY_FROM_JOB
    } else {
        0
    };
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | breakaway);
}

/// What `CreateProcess` says when the job does not let its processes leave it.
#[cfg(windows)]
const ERROR_ACCESS_DENIED: i32 = 5;

/// Is a hook with the same executable name as `program` running for this user? (C#
/// checked any `lbm-hook`, any user's; a hook of another user never listens on this
/// user's endpoint, so it is not a reason to stay without one.)
#[cfg(target_os = "linux")]
fn another_hook_runs(program: &Path) -> bool {
    let Some(name) = program.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    processes_named(Path::new("/proc"), name, uid, std::process::id())
}

/// Windows: a process of that executable name in this logon session (C#'s
/// `DaemonProcessManager` looked at any, in any session).
#[cfg(windows)]
fn another_hook_runs(program: &Path) -> bool {
    let Some(name) = program.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Ok(session) = crate::winpipe::session_id() else {
        return false;
    };
    windows_processes_named(name, session, std::process::id())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn another_hook_runs(_program: &Path) -> bool {
    false
}

/// Is a process other than `me`, in logon `session`, running the executable `name`
/// (compared as Windows compares file names, ignoring case)?
#[cfg(windows)]
pub fn windows_processes_named(name: &str, session: u32, me: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;

    // SAFETY: a snapshot of the process list, closed below.
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return false;
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = false;
    // SAFETY: the entry is sized as the API requires, the snapshot is open.
    let mut next = unsafe { Process32FirstW(snapshot, &mut entry) };
    while next.is_ok() && !found {
        let length = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        let exe = String::from_utf16_lossy(&entry.szExeFile[..length]);
        if entry.th32ProcessID != me && exe.eq_ignore_ascii_case(name) {
            let mut process_session = 0;
            // SAFETY: a query on a pid, into a local.
            found = unsafe { ProcessIdToSessionId(entry.th32ProcessID, &mut process_session) }
                .is_ok()
                && process_session == session;
        }
        // SAFETY: as above.
        next = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    // SAFETY: the snapshot opened above.
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    found
}

/// Under a `/proc`-like `root`: is a process other than `me`, of user `uid`, running
/// the executable `name`? (`comm` holds the first 15 bytes of the name.)
pub fn processes_named(root: &Path, name: &str, uid: u32, me: u32) -> bool {
    let comm: String = name.chars().take(15).collect();
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let dir = entry.path();
        let Some(pid) = dir
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<u32>().ok())
        else {
            return false;
        };
        if pid == me {
            return false;
        }
        let same_name = std::fs::read_to_string(dir.join("comm"))
            .is_ok_and(|c| c.trim_end_matches('\n') == comm);
        same_name
            && std::fs::read_to_string(dir.join("status")).is_ok_and(|status| {
                status
                    .lines()
                    .find_map(|l| l.strip_prefix("Uid:"))
                    .and_then(|ids| ids.split_whitespace().next())
                    .and_then(|real| real.parse::<u32>().ok())
                    == Some(uid)
            })
    })
}

#[cfg(test)]
mod tests {
    /// The test process sees itself under its own name only when it is not `me`.
    #[cfg(windows)]
    #[test]
    fn a_windows_process_of_this_session_is_found_by_name() {
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_str().unwrap();
        let session = crate::winpipe::session_id().unwrap();
        assert!(super::windows_processes_named(name, session, 0));
        assert!(super::windows_processes_named(
            &name.to_uppercase(),
            session,
            0
        ));
        assert!(!super::windows_processes_named(
            name,
            session,
            std::process::id()
        ));
        assert!(!super::windows_processes_named(
            name,
            session.wrapping_add(1000),
            0
        ));
    }

    use super::*;

    fn process(root: &Path, pid: u32, comm: &str, uid: u32) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
        std::fs::write(
            dir.join("status"),
            format!("Name:\t{comm}\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\n"),
        )
        .unwrap();
    }

    /// A stand-in "hook" that would run for a minute: the fallback ends it. Named
    /// uniquely (a link to `sleep`), or any `sleep` of this user would count as "another
    /// hook running".
    #[cfg(unix)]
    #[test]
    fn a_stop_that_cannot_be_delivered_ends_the_hook_this_agent_launched() {
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join(format!("lbmhook{}", std::process::id()));
        let sleep = ["/usr/bin/sleep", "/bin/sleep"]
            .into_iter()
            .find(|p| Path::new(p).exists())
            .expect("a sleep binary");
        std::os::unix::fs::symlink(sleep, &hook).unwrap();
        let mut launcher = HookLauncher::new(hook, vec!["60".into()], None);
        assert!(!launcher.stop_launched(), "nothing launched yet");
        let Launch::Started(pid) = launcher.on_unreachable(Instant::now()) else {
            panic!("launched");
        };
        assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
        assert!(launcher.stop_launched());
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "ended and reaped"
        );
        assert!(!launcher.stop_launched(), "nothing left to end");
    }

    #[test]
    fn only_this_users_hooks_count() {
        let proc = tempfile::tempdir().unwrap();
        let root = proc.path();
        process(root, 10, "lbm-agent", 1000);
        process(root, 11, "lbm-hook", 1001);
        std::fs::create_dir_all(root.join("self")).unwrap();
        assert!(
            !processes_named(root, "lbm-hook", 1000, 10),
            "another user's hook"
        );

        process(root, 12, "lbm-hook", 1000);
        assert!(processes_named(root, "lbm-hook", 1000, 10));
        assert!(!processes_named(root, "lbm-hook", 1000, 12), "not itself");
    }

    #[test]
    fn long_names_match_their_truncated_comm() {
        let proc = tempfile::tempdir().unwrap();
        process(proc.path(), 20, "LittleBigMouse.", 1000);
        assert!(processes_named(proc.path(), "LittleBigMouse.Hook", 1000, 1));
    }
}
