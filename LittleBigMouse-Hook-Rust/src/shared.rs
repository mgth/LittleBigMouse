//! Process-global shared state.
//!
//! Replaces the C++ `Hooker::_instance` static. It must be reachable from the
//! IPC threads and the message-pump thread (and, from Phase 3, the low-level
//! mouse-hook callback), so it lives in a `static OnceLock` rather than a
//! `thread_local`.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8};
use std::sync::{Mutex, OnceLock};

use crate::engine::MouseEngine;
use crate::ipc::server::ServerHandle;
use crate::priority::Priority;

pub struct Shared {
    /// The low-level mouse hook is currently installed (C++ `Hooker::Hooked`).
    pub hooked: AtomicBool,
    /// Desired hooking state (C++ `Hooker::_hookMouse`); the pump reconciles the
    /// actual hook to this on each `WM_BREAK_LOOP`.
    pub want_hook: AtomicBool,
    /// Paused by an excluded foreground window
    /// (C++ `LittleBigMouseDaemon::_paused`).
    pub paused: AtomicBool,
    /// The desktop is not being displayed (screen off: sleep / session standby / lock-idle).
    /// Set from the display-state power notification; deduplicates its repeated current-state
    /// pushes (the listener window — and its registration — is recreated every hook/unhook cycle).
    pub suspended: AtomicBool,
    /// Thread id of the message pump, for `PostThreadMessageW`
    /// (C++ `Hooker::_currentThreadId`). Zero until the pump starts.
    pub pump_tid: AtomicU32,
    /// Process priority while hooking / idle (C++ `Hooker::_priority` /
    /// `_priorityUnhooked`), stored as `Priority as u8`. Set from the loaded
    /// layout; read by the pump when (re)installing the hook.
    pub priority: AtomicU8,
    pub priority_unhooked: AtomicU8,
    /// The traversal engine and its zone layout (C++ `MouseEngine`). Behind a
    /// `Mutex` the callback will `try_lock` (Phase 3); `Load` locks it blocking.
    pub engine: Mutex<MouseEngine>,
    /// Foreground-process path substrings that pause the hook (C++
    /// `LittleBigMouseDaemon::_excluded`), loaded from `Excluded.txt` on `Run`.
    pub excluded: Mutex<Vec<String>>,
    /// The IPC server handle, published once the listener is up.
    pub server: OnceLock<ServerHandle>,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            hooked: AtomicBool::new(false),
            want_hook: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
            pump_tid: AtomicU32::new(0),
            // C++ Hooker defaults, until a layout overrides them.
            priority: AtomicU8::new(Priority::Normal.as_u8()),
            priority_unhooked: AtomicU8::new(Priority::Below.as_u8()),
            engine: Mutex::new(MouseEngine::new()),
            excluded: Mutex::new(Vec::new()),
            server: OnceLock::new(),
        }
    }

    /// Broadcast an event to all listening clients, if the server is up.
    pub fn broadcast(&self, msg: &str) {
        if let Some(server) = self.server.get() {
            server.broadcast(msg);
        }
    }

    /// C++ `LittleBigMouseDaemon::Excluded` — is `path` covered by an exclusion
    /// entry (substring match, entries longer than one char)?
    pub fn is_excluded(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let excluded = self.excluded.lock().unwrap();
        excluded
            .iter()
            .any(|line| line.len() > 1 && path.contains(line.as_str()))
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

/// The single process-global instance, initialized in `main`.
pub static SHARED: OnceLock<Shared> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_excluded_matches_substrings() {
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = vec![
            r"\steamapps\".to_string(),
            r"\Epic Games\".to_string(),
            r"\Riot Games\".to_string(),
        ];
        assert!(shared.is_excluded(r"D:\SteamLibrary\steamapps\common\Game\game.exe"));
        assert!(shared.is_excluded(r"C:\Program Files\Epic Games\Launcher\x.exe"));
        assert!(!shared.is_excluded(r"C:\Windows\explorer.exe"));
        assert!(!shared.is_excluded(""));
    }

    #[test]
    fn single_char_entries_are_ignored() {
        // C++ requires entries longer than one char (guards against a stray line).
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = vec![r"\".to_string()];
        assert!(!shared.is_excluded(r"C:\anything\at\all.exe"));
    }
}
