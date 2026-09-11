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
        detach(&mut command);
        command.spawn()
    }
}

/// Its own session (Unix) or process group without a console (Windows): a signal or a
/// closed terminal that ends the agent does not reach the hook.
#[cfg(unix)]
fn detach(command: &mut Command) {
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

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

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

#[cfg(not(target_os = "linux"))]
fn another_hook_runs(_program: &Path) -> bool {
    // Session-scoped process enumeration comes with the Windows agent; until then the
    // endpoint decides (a hook of this session that answers is never "unreachable").
    false
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
