//! Focus watcher — Linux counterpart of the WinEvent `EVENT_OBJECT_FOCUS` hook
//! (`hook/windows/win_events.rs`), feeding [`crate::hook::on_focus_changed`] so
//! the exclusion list (`Excluded.txt`) pauses the hook while a game is focused
//! (#515).
//!
//! Source: EWMH `_NET_ACTIVE_WINDOW` on the X root window, watched via
//! `PropertyNotify`. This covers more than native X11 sessions: under KWin
//! Wayland every activation is mirrored to the XWayland root — `Workspace::
//! setActiveWindow` → `RootInfo::setActiveClient` (kwin `activation.cpp` /
//! `netinfo.cpp`) — and Wine/Proton windows are XWayland clients, so the games
//! this exists for are exactly the windows it sees. A Wayland-native window
//! sets the property to a placeholder owned by KWin itself (`nullFocusWindow`),
//! which we detect (owner == the WM's own pid) and report as an empty path
//! → not excluded → unpause: correct, native apps are not games we can name
//! anyway. Excluding a Wayland-native app would need a compositor-specific
//! channel (KWin scripting) — out of scope until someone reports the need.
//!
//! The PID comes from `_NET_WM_PID` (set by Wine, GTK, Qt, SDL...) with an XRes
//! `QueryClientIds` fallback (server-side socket credentials, for clients that
//! do not set the property — Tk...), and the path from /proc via
//! [`crate::platform::process::foreground_path`], which resolves the Wine
//! preloader indirection.
//!
//! Runs on its own thread with its own X connection: the routing pumps stay
//! untouched (see the no-blocking rule in `evdev.rs`), and `on_focus_changed`
//! only flips atomics and broadcasts with a write timeout. The outer loop
//! reconnects (2 s backoff) so an XWayland restart or a daemon started before
//! the session is up just degrades, never kills the watcher.

use std::sync::atomic::Ordering;
use std::time::Duration;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::shared::Shared;

/// Delay between reconnection attempts when no X server is reachable.
const RECONNECT_EVERY: Duration = Duration::from_secs(2);

pub fn spawn(shared: &'static Shared) {
    if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
        // Pure Wayland without XWayland: no games to watch either (Wine needs X).
        eprintln!("[LittleBigMouse.Hook] focus: no DISPLAY, process exclusion inactive");
        return;
    }
    let _ = std::thread::Builder::new()
        .name("lbm-focus".into())
        .spawn(move || {
            let mut announced = false;
            while !shared.want_quit.load(Ordering::SeqCst) {
                match watch(shared, &mut announced) {
                    Ok(()) => return, // want_quit
                    Err(e) => {
                        if !announced {
                            eprintln!("[LittleBigMouse.Hook] focus: X watch failed ({e}), retrying");
                            announced = true;
                        }
                    }
                }
                std::thread::sleep(RECONNECT_EVERY);
            }
        });
}

/// One connection lifetime: subscribe, replay the current state (a game may
/// already be focused when the daemon starts), then follow `PropertyNotify`.
/// Returns `Err` on any connection-level failure so the caller reconnects.
fn watch(
    shared: &'static Shared,
    announced: &mut bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;

    let net_active_window = intern(&conn, "_NET_ACTIVE_WINDOW")?;
    let net_wm_pid = intern(&conn, "_NET_WM_PID")?;
    let net_supporting_wm_check = intern(&conn, "_NET_SUPPORTING_WM_CHECK")?;

    conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?
    .check()?;

    let mut watcher = Watcher {
        conn: &conn,
        root,
        net_active_window,
        net_wm_pid,
        // The WM's own pid, resolved through its EWMH check window: a focus
        // whose owner is the WM itself is a placeholder (KWin parks the X
        // active window on its `nullFocusWindow` while a Wayland-native window
        // is active), not a foreground app.
        wm_pid: window_property(&conn, root, net_supporting_wm_check, AtomEnum::WINDOW)
            .and_then(|w| xres_pid(&conn, w)),
        last: None,
    };

    eprintln!("[LittleBigMouse.Hook] focus: watching _NET_ACTIVE_WINDOW");
    *announced = false;

    watcher.report(shared);

    while !shared.want_quit.load(Ordering::SeqCst) {
        super::x11::wait_readable(&conn, 500);

        let mut changed = false;
        loop {
            match conn.poll_for_event() {
                Ok(Some(Event::PropertyNotify(e)))
                    if e.window == root && e.atom == net_active_window =>
                {
                    changed = true;
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => return Err(e.into()), // connection lost: reconnect
            }
        }
        if changed {
            watcher.report(shared);
        }
    }
    Ok(())
}

struct Watcher<'c> {
    conn: &'c RustConnection,
    root: u32,
    net_active_window: Atom,
    net_wm_pid: Atom,
    wm_pid: Option<u32>,
    /// C++ `static lastHwnd` — dedup repeated events for the same window.
    last: Option<u32>,
}

impl Watcher<'_> {
    /// Resolve the active window to a process path and feed `on_focus_changed`.
    ///
    /// An unresolvable focus (WM placeholder, no pid, dead pid) is reported as
    /// an EMPTY path — never skipped: leaving an excluded game for a
    /// Wayland-native window must unpause. The Windows hook can afford to skip
    /// (every hwnd resolves there); here the empty broadcast is harmless, the
    /// UI's `ProcessesCollector.AddProcess` ignores empty payloads.
    fn report(&mut self, shared: &Shared) {
        let window =
            window_property(self.conn, self.root, self.net_active_window, AtomEnum::WINDOW)
                .unwrap_or(0);
        if self.last == Some(window) {
            return;
        }
        self.last = Some(window);

        let path = self
            .window_pid(window)
            .filter(|pid| self.wm_pid != Some(*pid))
            .and_then(crate::platform::process::foreground_path)
            .unwrap_or_default();
        // Focus changes are user-paced (low volume) and this line is the one
        // that lets an issue reporter see what string their entry must match.
        eprintln!("[LittleBigMouse.Hook] focus: {}", if path.is_empty() { "<none>" } else { &path });
        crate::hook::on_focus_changed(shared, path);
    }

    /// Owning pid of `window`: `_NET_WM_PID` (client-declared — Wine, GTK, Qt
    /// set it), else XRes `QueryClientIds` (server-side, from the client
    /// socket's credentials — covers Tk and friends; XWayland implements it
    /// the same way).
    fn window_pid(&self, window: u32) -> Option<u32> {
        window_property(self.conn, window, self.net_wm_pid, AtomEnum::CARDINAL)
            .or_else(|| xres_pid(self.conn, window))
    }
}

fn xres_pid(conn: &RustConnection, window: u32) -> Option<u32> {
    use x11rb::protocol::res::{self, ConnectionExt as _};
    let specs = [res::ClientIdSpec {
        client: window,
        mask: res::ClientIdMask::LOCAL_CLIENT_PID,
    }];
    let reply = conn.res_query_client_ids(&specs).ok()?.reply().ok()?;
    reply
        .ids
        .iter()
        .find_map(|id| id.value.first().copied().filter(|pid| *pid != 0))
}

/// First CARDINAL/WINDOW value of `property` on `window`; `None` when unset,
/// zero, or on any request error (a destroyed window is not a failure).
fn window_property(
    conn: &RustConnection,
    window: u32,
    property: Atom,
    kind: AtomEnum,
) -> Option<u32> {
    if window == 0 {
        return None;
    }
    let reply = conn
        .get_property(false, window, property, kind, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let value = reply.value32()?.next();
    value.filter(|v| *v != 0)
}

fn intern(conn: &RustConnection, name: &str) -> Result<Atom, Box<dyn std::error::Error>> {
    Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
}
