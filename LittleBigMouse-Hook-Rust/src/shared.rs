//! Process-global shared state.
//!
//! Replaces the C++ `Hooker::_instance` static. It must be reachable from the
//! IPC threads and the message-pump thread (and, from Phase 3, the low-level
//! mouse-hook callback), so it lives in a `static OnceLock` rather than a
//! `thread_local`.

use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::OnceLock;

use crate::ipc::server::ServerHandle;

pub struct Shared {
    /// The low-level mouse hook is currently installed (C++ `Hooker::Hooked`).
    pub hooked: AtomicBool,
    /// Desired hooking state (C++ `Hooker::_hookMouse`); the pump reconciles the
    /// actual hook to this on each `WM_BREAK_LOOP`.
    pub want_hook: AtomicBool,
    /// Paused by an excluded foreground window
    /// (C++ `LittleBigMouseDaemon::_paused`).
    pub paused: AtomicBool,
    /// Thread id of the message pump, for `PostThreadMessageW`
    /// (C++ `Hooker::_currentThreadId`). Zero until the pump starts.
    pub pump_tid: AtomicU32,
    /// The IPC server handle, published once the listener is up.
    pub server: OnceLock<ServerHandle>,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            hooked: AtomicBool::new(false),
            want_hook: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            pump_tid: AtomicU32::new(0),
            server: OnceLock::new(),
        }
    }

    /// Broadcast an event to all listening clients, if the server is up.
    pub fn broadcast(&self, msg: &str) {
        if let Some(server) = self.server.get() {
            server.broadcast(msg);
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
