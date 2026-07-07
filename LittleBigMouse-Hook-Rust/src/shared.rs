//! Process-global shared state.
//!
//! Replaces the C++ `Hooker::_instance` static. It must be reachable from the
//! IPC threads and — from Phase 1 — the message-pump thread and the low-level
//! mouse-hook callback, so it lives in a `static OnceLock` rather than a
//! `thread_local`.

use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

use crate::ipc::server::ServerHandle;

pub struct Shared {
    /// Whether the low-level hook is currently installed (C++ `Hooker::Hooked`).
    /// In Phase 0 this is a pure state flag toggled by `Run`/`Stop`.
    pub hooked: AtomicBool,
    /// Whether the daemon is paused by an excluded foreground window
    /// (C++ `LittleBigMouseDaemon::_paused`).
    pub paused: AtomicBool,
    /// The IPC server handle, published once the listener is up.
    pub server: OnceLock<ServerHandle>,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            hooked: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            server: OnceLock::new(),
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

/// The single process-global instance, initialized in `main`.
pub static SHARED: OnceLock<Shared> = OnceLock::new();
