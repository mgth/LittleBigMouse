//! The agent's log: `agent.log` for the current run, the five before it kept as
//! `agent.prev.log`, `agent.prev.2.log` … `agent.prev.5.log` — the rotation of the C# UI's
//! `ui.log` (`LogRotation.cs`, #605).
//!
//! One previous generation is not enough: the run that matters is the one that failed,
//! and relaunching twice is the natural reaction to a program that does not come up
//! (#589's log was overwritten that way). The current log is moved out of the way
//! *first*: on Windows that is the move that fails while another instance holds the file,
//! and nothing must have shifted by then.
//!
//! Only when there is no terminal: run from one, the agent writes to it (C#: a real
//! console or an explicit redirection wins over the file).

use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

/// How many previous runs are kept.
pub const KEEP: usize = 5;

/// The file holding the run that ended `generation` runs ago (1: the last one).
pub fn previous_path(path: &Path, generation: usize) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let name = if generation == 1 {
        format!("{stem}.prev{ext}")
    } else {
        format!("{stem}.prev.{generation}{ext}")
    };
    path.with_file_name(name)
}

fn staging_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    path.with_file_name(format!("{stem}.rotating{ext}"))
}

/// C# `LogRotation.Rotate`: make room for a new `path` — the file there becomes the first
/// previous generation, every older one moves down, the oldest falls off. Fails when the
/// current log cannot be moved (held by another instance on Windows), the chain untouched.
pub fn rotate(path: &Path, keep: usize) -> io::Result<()> {
    let staging = staging_path(path);
    // A rotation interrupted between the two moves left the last run in staging: fold
    // it in rather than leave it orphaned.
    if staging.exists() {
        archive(path, &staging, keep)?;
    }
    if !path.exists() {
        return Ok(());
    }
    fs::rename(path, &staging)?;
    archive(path, &staging, keep)
}

/// Shifts the chain from the oldest, then files `staged` as the last run.
fn archive(path: &Path, staged: &Path, keep: usize) -> io::Result<()> {
    for generation in (1..keep).rev() {
        let from = previous_path(path, generation);
        if from.exists() {
            fs::rename(&from, previous_path(path, generation + 1))?;
        }
    }
    fs::rename(staged, previous_path(path, 1))
}

/// Sends this process's standard error to `path`, after rotating it, unless standard
/// error is a terminal. Returns the log file when it is used. Never fails the start:
/// logging must not keep the agent from running.
pub fn to_file_unless_terminal(path: &Path) -> Option<PathBuf> {
    if io::stderr().is_terminal() {
        return None;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok()?;
    }
    rotate(path, KEEP).ok()?;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .ok()?;
    redirect_stderr(file).ok()?;
    Some(path.to_path_buf())
}

#[cfg(unix)]
fn redirect_stderr(file: fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: both descriptors are valid; dup2 makes 2 a copy of the log's.
    if unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
fn redirect_stderr(file: fs::File) -> io::Result<()> {
    use std::os::windows::io::IntoRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};
    // Rust's standard error asks for the handle at each write: this redirects it. The
    // handle is kept for the life of the process.
    let handle = HANDLE(file.into_raw_handle());
    // SAFETY: the handle is a valid, owned file handle, never closed.
    unsafe { SetStdHandle(STD_ERROR_HANDLE, handle) }.map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One run: rotate what is there, then write this run's log.
    fn run(log: &Path, content: &str) {
        rotate(log, KEEP).unwrap();
        fs::write(log, content).unwrap();
    }

    fn previous(log: &Path, generation: usize) -> String {
        fs::read_to_string(previous_path(log, generation)).unwrap()
    }

    fn files(dir: &Path) -> usize {
        fs::read_dir(dir).unwrap().count()
    }

    /// C#: `LogRotationTests.FirstRun_HasNothingToRotate`.
    #[test]
    fn first_run_has_nothing_to_rotate() {
        let dir = tempfile::tempdir().unwrap();
        rotate(&dir.path().join("agent.log"), KEEP).unwrap();
        assert_eq!(files(dir.path()), 0);
    }

    /// C#: `GenerationsAreNamedPrevThenNumbered`.
    #[test]
    fn generations_are_named_prev_then_numbered() {
        let log = Path::new("/d/agent.log");
        assert_eq!(previous_path(log, 1), Path::new("/d/agent.prev.log"));
        assert_eq!(previous_path(log, 2), Path::new("/d/agent.prev.2.log"));
        assert_eq!(previous_path(log, 5), Path::new("/d/agent.prev.5.log"));
    }

    /// C#: `TheLastRunBecomesPrev`.
    #[test]
    fn the_last_run_becomes_prev() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agent.log");
        run(&log, "run 1");
        run(&log, "run 2");
        assert_eq!(fs::read_to_string(&log).unwrap(), "run 2");
        assert_eq!(previous(&log, 1), "run 1");
        assert!(!previous_path(&log, 2).exists());
    }

    /// C#: `TwoRelaunchesKeepTheRunThatMattered` (#589).
    #[test]
    fn two_relaunches_keep_the_run_that_mattered() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agent.log");
        run(&log, "hot-plug failure");
        run(&log, "boot failed");
        run(&log, "boot failed again");
        assert_eq!(previous(&log, 2), "hot-plug failure");
    }

    /// C#: `KeepsFiveRuns_TheOldestFallsOff`.
    #[test]
    fn keeps_five_runs_the_oldest_falls_off() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agent.log");
        for i in 1..=7 {
            run(&log, &format!("run {i}"));
        }
        assert_eq!(fs::read_to_string(&log).unwrap(), "run 7");
        for generation in 1..=KEEP {
            assert_eq!(
                previous(&log, generation),
                format!("run {}", 7 - generation)
            );
        }
        assert!(!previous_path(&log, KEEP + 1).exists());
        assert_eq!(files(dir.path()), KEEP + 1);
    }

    /// C#: `AnInterruptedRotationIsFoldedIn`.
    #[test]
    fn an_interrupted_rotation_is_folded_in() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agent.log");
        fs::write(dir.path().join("agent.rotating.log"), "interrupted").unwrap();
        fs::write(&log, "current").unwrap();

        rotate(&log, KEEP).unwrap();

        assert_eq!(previous(&log, 1), "current");
        assert_eq!(previous(&log, 2), "interrupted");
        assert!(!dir.path().join("agent.rotating.log").exists());
    }

    /// C#: `ACurrentLogHeldOpen_LeavesTheChainUntouched` — Windows refuses to move a file
    /// another handle holds (POSIX renames open files freely: nothing to prove there).
    #[cfg(windows)]
    #[test]
    fn a_current_log_held_open_leaves_the_chain_untouched() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("agent.log");
        run(&log, "run 1");
        run(&log, "run 2");
        {
            // Held as the running instance holds it: others may read, not delete.
            let _held = OpenOptions::new()
                .write(true)
                .share_mode(1) // FILE_SHARE_READ
                .open(&log)
                .unwrap();
            assert!(rotate(&log, KEEP).is_err());
            assert!(log.exists());
            assert_eq!(previous(&log, 1), "run 1");
            assert!(!previous_path(&log, 2).exists());
        }
    }
}
