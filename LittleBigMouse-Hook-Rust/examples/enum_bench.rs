//! Micro-bench: cost of the hot-plug rescan enumeration on this machine.
//! The pump runs this every 2 s on the routing thread — if it is slow, the
//! cursor hitches at the same cadence.

#[cfg(target_os = "linux")]
use std::time::Instant;

#[cfg(target_os = "linux")]
fn main() {
    // Warm-up (dentry/inode caches).
    let _ = evdev::enumerate().count();

    let mut worst = 0u128;
    let mut total = 0u128;
    const N: u32 = 50;
    for _ in 0..N {
        let t = Instant::now();
        let mice = evdev::enumerate().count();
        let kbds = evdev::enumerate().count(); // pump does two full scans
        let us = t.elapsed().as_micros();
        worst = worst.max(us);
        total += us;
        std::hint::black_box((mice, kbds));
    }
    println!(
        "two enumerates: avg {}us, worst {}us over {N} runs",
        total / N as u128,
        worst
    );
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("enum_bench is available on Linux only");
}
