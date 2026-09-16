//! Per-OS helpers behind a common facade.
//!
//! Each OS module exposes the same submodules (`cursor`, `display`, `paths`, `process`) and
//! free functions (`init`, `set_process_priority`); the re-exports keep call
//! sites platform-neutral (`crate::platform::paths::...`).

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::{cursor, display, init, paths, process, set_process_priority};

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::{cursor, display, init, paths, process, set_process_priority};
