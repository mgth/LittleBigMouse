//! A counting global allocator, shared by the benchmarks that report allocation
//! figures (`alloc_profile`, `evdev_pump`).
//!
//! Unlike a timing, an allocation count is *not* machine-dependent: it is an
//! exact number, identical on any host for a given build. That makes it the half
//! of a baseline worth diffing — a change that starts allocating on a per-event
//! path shows up here as a whole number, with no statistics to argue about.
//!
//! Registering it is one line per bench binary:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOCATOR: support::alloc::Counting = support::alloc::Counting;
//! ```
//!
//! It applies to that bench binary only — nothing about the daemon's allocator
//! changes. Counting itself costs two relaxed atomics per allocation, so a run
//! that allocates heavily is a few percent slower than it would otherwise be;
//! the code paths that allocate nothing pay nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

pub struct Counting;

/// Delegates everything to the system allocator, counting on the way through.
/// `realloc` and `alloc_zeroed` are forwarded rather than left to the trait's
/// default implementations so the allocator behaves exactly like the real one —
/// the defaults would turn every `realloc` into an alloc/copy/free.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(
            new_size.saturating_sub(layout.size()) as u64,
            Ordering::Relaxed,
        );
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Start counting from zero. Call it after the warm-up lap: lazily initialised
/// statics (a hash state, stdout's buffer) allocate exactly once and would
/// otherwise be charged to the first measured iteration.
pub fn reset() {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
}

/// `(allocations, bytes)` since the last [`reset`].
pub fn counts() -> (u64, u64) {
    (
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}
