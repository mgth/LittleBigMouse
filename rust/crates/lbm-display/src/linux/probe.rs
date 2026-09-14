//! Asking the session a question, with a deadline.
//!
//! The display probes are external commands — `kscreen-doctor`, `xrandr` — and an external
//! command can decline to answer. `kscreen-doctor` does exactly that when it cannot reach
//! the session it was pointed at: it neither prints nor exits.
//!
//! In the C# UI that was a window that did not open, which the user could see and kill. In
//! the agent it is worse: the agent is resident, it is started by the session, it holds the
//! instance lock while it hangs, and it has not written a line of its log yet — so every
//! later launch finds the lock, leaves, and the user sees nothing at all (the shape of
//! #589). Every probe therefore gets a deadline, and a probe that runs out is a session
//! that said nothing: the next source is tried, which is what happens for a session
//! without `kscreen-doctor` anyway.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Long enough for a busy session to answer, short enough that a session which never will
/// does not hold up the agent's whole startup. C#'s own `WaitForExit(5000)`.
pub const PATIENCE: Duration = Duration::from_secs(5);

/// What `program args…` printed, when it exits successfully inside `patience`.
///
/// Nothing otherwise — a command that is not there, one that failed, and one that ran out
/// of time are all "this source has no answer", which is what the caller does with them.
/// The same, with the error stream folded into the answer.
///
/// For a command whose failure is something it *says* rather than something it exits
/// with: `kscreen-doctor` exits 0 even when the compositor rejects the configuration, and
/// the only signal is the word "failed" on its output — dropping stderr would drop the
/// only evidence that a topology change did not take.
pub fn run_with_stderr(program: &str, args: &[&str], patience: Duration) -> Option<String> {
    run_inner(program, args, patience, Stdio::piped(), true)
}

pub fn run(program: &str, args: &[&str], patience: Duration) -> Option<String> {
    run_inner(program, args, patience, Stdio::null(), false)
}

fn run_inner(
    program: &str,
    args: &[&str],
    patience: Duration,
    stderr: Stdio,
    keep_failure: bool,
) -> Option<String> {
    let started = Instant::now();
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .ok()?;

    // Read on a thread of its own rather than after waiting: a child that fills the pipe
    // blocks until someone drains it, so waiting first can deadlock on a long answer.
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    // Drained on its own thread too, when it was asked for. Redirecting the error stream
    // and then not reading it would be worse than not redirecting it: the child blocks on
    // a full pipe, and what it was trying to say is exactly the failure signal.
    let errors = child.stderr.take().map(|mut stderr| {
        let (said, hearing) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            let _ = said.send(text);
        });
        hearing
    });
    let (printed, reading) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = printed.send(text);
    });

    let Ok(mut text) = reading.recv_timeout(patience) else {
        return give_up(program, &mut child);
    };
    if let Some(errors) = errors {
        if let Ok(said) = errors.recv_timeout(patience) {
            text.push_str(&said);
        }
    }

    // The pipe is closed; the process itself usually follows at once, but "usually" is
    // what this whole module is about.
    loop {
        match child.try_wait() {
            // `keep_failure` is for a command that reports failure in words: its output
            // is the evidence, so it comes back whatever the exit code was.
            Ok(Some(status)) => return (status.success() || keep_failure).then_some(text),
            Ok(None) if started.elapsed() < patience => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => return give_up(program, &mut child),
            Err(_) => return None,
        }
    }
}

/// Out of time: end it, and say so once. Silence here would be the bug all over again.
fn give_up(program: &str, child: &mut std::process::Child) -> Option<String> {
    eprintln!("[lbm-display] {program} did not answer in time; ignoring this source");
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// Run on Linux only: every case here spawns a POSIX shell to play the part of a probe
/// that prints, fails, hangs, lingers or answers at length. The module is compiled on
/// Windows — the Linux enumeration is, for the fixtures the parity tests read — but what
/// it guards (kscreen-doctor, xrandr) only ever runs here, and `sh` on a Windows runner
/// is whatever Git for Windows happens to ship, slow enough under load to trip a
/// deadline this test is not about.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn what_a_command_prints_comes_back() {
        assert_eq!(
            run("sh", &["-c", "printf hello"], PATIENCE).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn a_command_that_fails_has_no_answer() {
        assert_eq!(run("sh", &["-c", "printf out; exit 3"], PATIENCE), None);
    }

    #[test]
    fn a_command_that_is_not_there_has_no_answer() {
        assert_eq!(run("lbm-no-such-program", &[], PATIENCE), None);
    }

    #[test]
    fn a_command_that_never_answers_is_given_up_on() {
        // The case that hangs the agent: a probe that neither prints nor exits. Bounded,
        // it costs the deadline and nothing else.
        let started = Instant::now();

        let answer = run("sh", &["-c", "sleep 30"], Duration::from_millis(200));

        assert_eq!(answer, None);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "gave up after {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_command_that_prints_and_then_lingers_is_given_up_on_too() {
        // Stdout closed, the process still there: the answer is not taken, because a
        // source that cannot finish answering is not one to trust.
        let started = Instant::now();

        let answer = run(
            "sh",
            &["-c", "printf half; exec 1>&-; sleep 30"],
            Duration::from_millis(200),
        );

        assert_eq!(answer, None);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn an_answer_longer_than_a_pipe_still_arrives() {
        // 64 KiB is where a pipe stops taking; a many-output kscreen-doctor document can
        // pass it, and draining only after waiting would deadlock.
        let answer = run("sh", &["-c", "yes lbm | head -c 200000"], PATIENCE);

        assert_eq!(answer.map(|text| text.len()), Some(200_000));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod stderr_tests {
    use super::*;

    /// A command that reports failure in words needs both streams and its exit code
    /// ignored — otherwise the evidence of a rejected topology is thrown away.
    #[test]
    fn what_a_failing_command_says_on_both_streams_comes_back() {
        let said = run_with_stderr(
            "sh",
            &[
                "-c",
                "printf out; printf ' applying config failed!' >&2; exit 1",
            ],
            PATIENCE,
        )
        .expect("a command that fails in words still has something to say");
        assert!(said.contains("out"), "stdout was dropped: {said}");
        assert!(
            said.contains("applying config failed!"),
            "stderr was dropped, which is the only failure signal kscreen-doctor gives: {said}"
        );
    }

    /// The plain runner keeps its old contract: a failure is no answer at all.
    #[test]
    fn the_quiet_runner_still_treats_a_failure_as_no_answer() {
        assert_eq!(run("sh", &["-c", "printf out; exit 1"], PATIENCE), None);
    }
}
