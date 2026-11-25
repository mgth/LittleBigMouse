//! Linux platform helpers — the counterpart of [`crate::platform::windows`].

pub mod cursor;
pub mod paths;
pub mod process;

use std::collections::BTreeMap;
use std::sync::Mutex;

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

/// The nice each thread had when first seen, by TID: where a lowered priority
/// must be able to come back to. Entries of threads that are gone are dropped
/// at each call.
static BASE_NICE: Mutex<BTreeMap<libc::id_t, i32>> = Mutex::new(BTreeMap::new());

/// Best-effort niceness mapping of the Windows priority classes, applied to
/// every thread of the process.
///
/// Two Linux facts shape it:
/// - nice is per thread: `setpriority(PRIO_PROCESS, 0, …)` only touches the
///   calling thread, and a new thread inherits its creator's. So each thread of
///   `/proc/self/task` is set, not only the caller.
/// - without `CAP_SYS_NICE`, a thread may raise its nice but never lower it
///   again below `20 - RLIMIT_NICE` (0 by default: no way back at all). Dropping
///   to `Below` when unhooked would then stick, and the next hooked session
///   would run its pointer threads at nice 10. A change that could not be undone
///   is therefore skipped, thread by thread: each keeps the nice it had when
///   first seen.
///
/// Failures are ignored — priority is an optimization, never a functional
/// requirement.
pub fn set_process_priority(priority: Priority) {
    let target = nice_of(priority);
    let lowest = lowest_allowed_nice();
    let tids: Vec<libc::id_t> = match std::fs::read_dir("/proc/self/task") {
        Ok(tasks) => tasks
            .flatten()
            .filter_map(|t| t.file_name().to_str()?.parse().ok())
            .collect(),
        Err(_) => vec![unsafe { libc::gettid() } as libc::id_t],
    };
    let mut bases = BASE_NICE.lock().unwrap_or_else(|e| e.into_inner());
    bases.retain(|tid, _| tids.contains(tid));
    for tid in tids {
        let Some(current) = nice_of_thread(tid) else {
            continue;
        };
        let base = *bases.entry(tid).or_insert(current);
        let nice = reachable_nice(target, base, lowest);
        if nice != current {
            unsafe { libc::setpriority(libc::PRIO_PROCESS, tid, nice) };
        }
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

/// The nice of one thread, `None` if it cannot be read (gone already).
fn nice_of_thread(tid: libc::id_t) -> Option<i32> {
    // getpriority returns -1 both for an error and for a nice of -1: errno tells.
    unsafe {
        *libc::__errno_location() = 0;
        let nice = libc::getpriority(libc::PRIO_PROCESS, tid);
        (nice != -1 || *libc::__errno_location() == 0).then_some(nice)
    }
}

/// The lowest nice this process may set (`man 2 setpriority`): anything with
/// the effective `CAP_SYS_NICE` capability, else `20 - RLIMIT_NICE` (soft
/// limit) — for root without the capability too.
fn lowest_allowed_nice() -> i32 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    lowest_nice_for(cap_eff(&status), rlimit_nice())
}

/// [`lowest_allowed_nice`] from its two inputs: the effective capability set
/// (`None` if unreadable) and the soft `RLIMIT_NICE` (`None` if unreadable).
fn lowest_nice_for(cap_eff: Option<u64>, rlimit: Option<libc::rlim_t>) -> i32 {
    const CAP_SYS_NICE: u32 = 23;
    if cap_eff.is_some_and(|caps| caps & (1 << CAP_SYS_NICE) != 0) {
        return -20;
    }
    match rlimit {
        None => 20,
        Some(libc::RLIM_INFINITY) => -20,
        Some(limit) => (20 - limit.min(40) as i32).clamp(-20, 20),
    }
}

/// The `CapEff:` mask of a `/proc/<pid>/status` text.
fn cap_eff(status: &str) -> Option<u64> {
    let hex = status.lines().find_map(|l| l.strip_prefix("CapEff:"))?;
    u64::from_str_radix(hex.trim(), 16).ok()
}

/// The soft `RLIMIT_NICE` of this process.
fn rlimit_nice() -> Option<libc::rlim_t> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    (unsafe { libc::getrlimit(libc::RLIMIT_NICE, &mut limit) } == 0).then_some(limit.rlim_cur)
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

    #[test]
    fn the_capability_not_the_uid_decides_what_is_reachable() {
        let with_cap = Some(1u64 << 23);
        let without = Some(0x1ff_feff_ffffu64 & !(1 << 23)); // root's set minus CAP_SYS_NICE
        assert_eq!(lowest_nice_for(with_cap, Some(0)), -20);
        assert_eq!(lowest_nice_for(without, Some(0)), 20);
        assert_eq!(lowest_nice_for(without, Some(20)), 0);
        assert_eq!(lowest_nice_for(None, Some(libc::RLIM_INFINITY)), -20);
        assert_eq!(lowest_nice_for(None, None), 20);
    }

    #[test]
    fn cap_eff_is_read_from_the_status_text() {
        let status = "Name:\tlbm-hook\nCapPrm:\t0000000000000000\nCapEff:\t0000000000800000\n";
        assert_eq!(cap_eff(status), Some(1 << 23));
        assert_eq!(cap_eff("Name:\tx\n"), None);
    }

    /// The reviewer's case, in a child process so the niceness changes (which
    /// cannot be undone) stay there: with `RLIMIT_NICE` 0, a caller at nice 10
    /// and another thread at 0. After `Below` then `Normal`, the other thread
    /// must still be at 0, and the caller at 10.
    #[test]
    fn each_thread_keeps_the_nice_it_could_not_get_back() {
        const CHILD: &str = "LBM_TEST_NICE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "platform::linux::tests::each_thread_keeps_the_nice_it_could_not_get_back",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr);
            assert!(out.status.success(), "{text}");
            assert!(text.contains("1 passed"), "the child ran no test: {text}");
            return;
        }
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        if lowest_nice_for(cap_eff(&status), Some(0)) < 20 {
            eprintln!("CAP_SYS_NICE held: nothing is one-way here, skipped");
            return;
        }
        let none = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NICE, &none) }, 0);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let other = std::thread::spawn(move || {
            ready_tx
                .send(unsafe { libc::gettid() } as libc::id_t)
                .unwrap();
            done_rx.recv().unwrap();
        });
        let other_tid = ready_rx.recv().unwrap();
        let start = nice_of_thread(other_tid).unwrap();
        if start > 9 {
            // Started under `nice -n 10` or more: no room for the caller at start + 10.
            eprintln!("started at nice {start}: no room above, skipped");
            done_tx.send(()).unwrap();
            other.join().unwrap();
            return;
        }
        let me = unsafe { libc::gettid() } as libc::id_t;
        assert_eq!(
            unsafe { libc::setpriority(libc::PRIO_PROCESS, me, start + 10) },
            0
        );

        set_process_priority(Priority::Below);
        set_process_priority(Priority::Normal);

        assert_eq!(
            nice_of_thread(other_tid),
            Some(start),
            "the other thread lost its nice"
        );
        assert_eq!(nice_of_thread(me), Some(start + 10));
        done_tx.send(()).unwrap();
        other.join().unwrap();
    }
}
