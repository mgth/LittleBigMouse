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
    /// Exit requested (`Quit`). The Linux event loops poll this; the Windows pump
    /// exits through WM_QUIT instead and never reads it.
    pub want_quit: AtomicBool,
    /// Thread id of the message pump, for `PostThreadMessageW`
    /// (C++ `Hooker::_currentThreadId`). Zero until the pump starts.
    pub pump_tid: AtomicU32,
    /// Thread id of the panic-shortcut listener, for `PostThreadMessageW`. Zero
    /// until it starts. Deliberately not the pump's: the rescue must not depend on
    /// the thread it exists to rescue you from.
    pub rescue_tid: AtomicU32,
    /// Whether the panic shortcut is currently registered with the OS. False when
    /// another application already owns the combination — the UI says so rather
    /// than leaving the user with a rescue that does not exist.
    pub rescue_registered: AtomicBool,
    /// The panic shortcut as the layout spells it (`Ctrl+Alt+Shift+M`). Read by the
    /// listener whenever it is asked to reconcile.
    pub rescue_shortcut: Mutex<String>,
    /// How many times a `Load` has asked for the hook to come down. Taking it down
    /// tears the mouse, focus, desktop and display hooks apart and destroys the
    /// display window, so a `Load` that is immediately followed by a `Run` skips it —
    /// see `daemon::load_layout`. Counted because that is the only way to observe,
    /// from a test, a teardown that did not happen.
    pub unhook_requests: AtomicU32,
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
    /// Serialized XML of the last successfully loaded layout. The edge prober
    /// re-parses it into a private engine so a `Probe` never disturbs the live
    /// one (tracking state, clip) nor blocks the hook thread on the engine lock.
    pub last_layout_xml: Mutex<String>,
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
            want_quit: AtomicBool::new(false),
            pump_tid: AtomicU32::new(0),
            rescue_tid: AtomicU32::new(0),
            rescue_registered: AtomicBool::new(false),
            rescue_shortcut: Mutex::new(crate::shortcut::DEFAULT.to_string()),
            unhook_requests: AtomicU32::new(0),
            // C++ Hooker defaults, until a layout overrides them.
            priority: AtomicU8::new(Priority::Normal.as_u8()),
            priority_unhooked: AtomicU8::new(Priority::Below.as_u8()),
            engine: Mutex::new(MouseEngine::new()),
            excluded: Mutex::new(Vec::new()),
            last_layout_xml: Mutex::new(String::new()),
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
    ///
    /// Two liberties over the C++ `find`, driven by the Linux port (#515):
    /// - separators are normalized (`\` == `/`), so the Windows-style defaults
    ///   (`\steamapps\`) match a Wine path (`Z:\...\steamapps\...`) as well as a
    ///   native one (`/home/.../steamapps/...`);
    /// - `*` splits an entry into ordered substrings (`*EscapeFrom*`,
    ///   `Escape*Tarkov`) — users write wildcards spontaneously (#515) and they
    ///   used to silently never match. Without `*` this degrades to the exact
    ///   C++ behavior.
    pub fn is_excluded(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let path = path.replace('\\', "/");
        let excluded = self.excluded.lock().unwrap();
        excluded
            .iter()
            .any(|line| line.len() > 1 && matches(&path, &line.replace('\\', "/")))
    }
}

/// Unanchored ordered-substring match: every non-empty `*`-separated segment of
/// `pattern` must occur in `path`, in order, without overlap. A pattern with no
/// non-empty segment (`**`) matches nothing — never silently exclude everything.
fn matches(path: &str, pattern: &str) -> bool {
    let mut pos = 0;
    let mut has_segment = false;
    for segment in pattern.split('*') {
        if segment.is_empty() {
            continue;
        }
        has_segment = true;
        match path[pos..].find(segment) {
            Some(i) => pos += i + segment.len(),
            None => return false,
        }
    }
    has_segment
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

    #[test]
    fn separators_are_normalized() {
        // The Windows-style defaults must cover native and Wine paths (#515).
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = vec![r"\steamapps\".to_string()];
        assert!(shared.is_excluded("/home/u/.local/share/Steam/steamapps/common/G/g.x86_64"));
        assert!(shared.is_excluded(r"Z:\home\u\Steam\steamapps\common\G\g.exe"));

        // ...and the slash-style Linux defaults (ExcludedProcessDefaults.Linux)
        // must cover the Windows-style command lines Wine games expose.
        *shared.excluded.lock().unwrap() = vec!["/steamapps/".to_string(), "/Games/".to_string()];
        assert!(shared.is_excluded(r"Z:\home\u\Steam\steamapps\common\G\g.exe"));
        assert!(shared.is_excluded(r"Z:\home\u\Games\Heroic\G\g.exe"));
        assert!(!shared.is_excluded(r"C:\Riot Games\League of Legends\lol.exe"));
    }

    #[test]
    fn wildcard_entries_match_ordered_segments() {
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = vec![
            "*EscapeFrom*".to_string(), // the exact entry from #515
            "Riot*League".to_string(),
        ];
        assert!(shared.is_excluded(r"Z:\games\EFT\EscapeFromTarkov.exe"));
        assert!(shared.is_excluded(r"C:\Riot Games\League of Legends\lol.exe"));
        // segments must appear in order
        assert!(!shared.is_excluded(r"C:\League Games\Riot of Legends"));
    }

    #[test]
    fn wildcard_only_entries_match_nothing() {
        // "**" passes the len > 1 guard but must not exclude everything.
        let shared = Shared::new();
        *shared.excluded.lock().unwrap() = vec!["**".to_string()];
        assert!(!shared.is_excluded(r"C:\anything\at\all.exe"));
    }
}
