//! Linux platform helpers — the counterpart of [`crate::platform::windows`].

pub mod cursor;
pub mod paths;
pub mod process;

use std::sync::OnceLock;

use crate::priority::Priority;

/// Nothing to set up process-wide on Linux (the Win32 side sets DPI awareness).
pub fn init() {}

/// The niceness mapping of the Windows priority classes.
fn nice_of(priority: Priority) -> i32 {
    match priority {
        Priority::Idle => 19,
        Priority::Below => 10,
        Priority::Normal => 0,
        Priority::Above => -5,
        Priority::High => -10,
        Priority::Realtime => -15,
    }
}

/// The nice the hook started with: where a lowered priority must be able to
/// come back to.
static BASE_NICE: OnceLock<i32> = OnceLock::new();

/// Best-effort niceness mapping of the Windows priority classes, applied to
/// every thread of the process.
///
/// Two Linux facts shape it:
/// - nice is per thread: `setpriority(PRIO_PROCESS, 0, …)` only touches the
///   calling thread, and a new thread inherits its creator's. So each thread of
///   `/proc/self/task` is set, not only the caller.
/// - without privileges, a thread may raise its nice but never lower it again
///   below `20 - RLIMIT_NICE` (0 by default: no way back at all). Dropping to
///   `Below` when unhooked would then stick, and the next hooked session would
///   run its pointer threads at nice 10. A change that could not be undone is
///   therefore skipped: the priority stays where the hook started.
///
/// Failures are ignored — priority is an optimization, never a functional
/// requirement.
pub fn set_process_priority(priority: Priority) {
    let base = *BASE_NICE.get_or_init(current_nice);
    let nice = reachable_nice(nice_of(priority), base, lowest_allowed_nice());
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
        return;
    };
    for tid in tasks
        .flatten()
        .filter_map(|t| t.file_name().to_str()?.parse::<libc::id_t>().ok())
    {
        unsafe { libc::setpriority(libc::PRIO_PROCESS, tid, nice) };
    }
}

/// The nice to set for `target`, given the nice the hook started with and the
/// lowest nice this process is allowed to set.
///
/// - Never past what is allowed: asking for more priority than permitted gets
///   the most that is permitted, without going below `base`'s reach.
/// - Never a lowering with no way back: if `base` could not be set again once
///   left, the priority stays at `base`.
fn reachable_nice(target: i32, base: i32, lowest: i32) -> i32 {
    if target > base && lowest > base {
        return base;
    }
    target.max(lowest.min(base))
}

/// The nice of the main thread (the process's), read before anything changes it.
fn current_nice() -> i32 {
    // getpriority returns -1 both for an error and for a nice of -1: errno tells.
    unsafe {
        *libc::__errno_location() = 0;
        let nice = libc::getpriority(libc::PRIO_PROCESS, 0);
        if nice == -1 && *libc::__errno_location() != 0 {
            0
        } else {
            nice
        }
    }
}

/// The lowest nice this process may set: anything for root, else
/// `20 - RLIMIT_NICE` (`man 2 setpriority`). `CAP_SYS_NICE` without root is not
/// looked at: it only makes this answer cautious, never wrong.
fn lowest_allowed_nice() -> i32 {
    if unsafe { libc::geteuid() } == 0 {
        return -20;
    }
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NICE, &mut limit) } != 0 {
        return 20;
    }
    if limit.rlim_cur == libc::RLIM_INFINITY {
        return -20;
    }
    (20 - limit.rlim_cur.min(40) as i32).clamp(-20, 20)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lowering_with_no_way_back_is_skipped() {
        // Default desktop: RLIMIT_NICE 0, so nothing below 20 can be set again.
        assert_eq!(reachable_nice(nice_of(Priority::Below), 0, 20), 0);
        assert_eq!(reachable_nice(nice_of(Priority::Idle), 0, 20), 0);
        // And coming back is a no-op, not a failed call.
        assert_eq!(reachable_nice(nice_of(Priority::Normal), 0, 20), 0);
    }

    #[test]
    fn a_lowering_that_can_be_undone_is_applied() {
        // RLIMIT_NICE 20: nice 0 can be set again after 10.
        assert_eq!(reachable_nice(nice_of(Priority::Below), 0, 0), 10);
        assert_eq!(reachable_nice(nice_of(Priority::Normal), 0, 0), 0);
        // Root.
        assert_eq!(reachable_nice(nice_of(Priority::Idle), 0, -20), 19);
    }

    #[test]
    fn more_priority_than_allowed_gets_what_is_allowed() {
        // Unprivileged: Above stays at the start nice.
        assert_eq!(reachable_nice(nice_of(Priority::Above), 0, 20), 0);
        // RLIMIT_NICE 25 (lowest -5): High gets -5.
        assert_eq!(reachable_nice(nice_of(Priority::High), 0, -5), -5);
        // Root: what was asked.
        assert_eq!(reachable_nice(nice_of(Priority::Realtime), 0, -20), -15);
    }

    #[test]
    fn a_hook_started_niced_keeps_its_start_as_the_floor_of_its_reach() {
        // Started at nice 5 by its launcher, unprivileged: Normal cannot go below 5.
        assert_eq!(reachable_nice(nice_of(Priority::Normal), 5, 20), 5);
        assert_eq!(reachable_nice(nice_of(Priority::Below), 5, 20), 5);
    }
}
