//! X11 backend: XInput2 raw motion events feed the engine; warping and clip
//! emulation reproduce the Win32 semantics the engine expects.
//!
//! Faithful mapping of the Windows behaviors:
//! - `WH_MOUSE_LL`            → XI2 `RawMotion` on the root window (all master devices)
//! - `SetCursorPos`           → `WarpPointer` to root coordinates
//! - `ClipCursor(rect)`       → software clamp: setting the clip warps the cursor
//!                              inside (Win32 does), and every later motion outside
//!                              is warped back before the engine sees it
//! - `GetAsyncKeyState(CTRL)` → `QueryKeymap` against the Control modifier keycodes
//! - `WM_DISPLAYCHANGE`       → RandR `ScreenChangeNotify` → `on_display_changed`
//!
//! Not mapped (yet): focus-based exclusion (EWMH `_NET_ACTIVE_WINDOW`) and the
//! freelook signals — `cursor_hidden` and `clip_is_subrect_of_virtual_screen`
//! report "no freelook".

use std::os::fd::AsRawFd;
use std::sync::atomic::Ordering;
use std::time::Instant;

use x11rb::connection::Connection;
use x11rb::protocol::randr::{self, ConnectionExt as _};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{ConnectionExt as _, GetModifierMappingReply};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::engine::cursor::CursorEnv;
use crate::engine::event::MouseEventArg;
use crate::geometry::{Point, Rect};
use crate::ipc::protocol;
use crate::shared::Shared;

/// True when an X display is reachable (native X11 or XWayland).
pub fn available() -> bool {
    std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty())
}

pub fn run(shared: &'static Shared) {
    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("[LittleBigMouse.Hook] x11: cannot connect: {e}");
            return;
        }
    };
    let setup_root = conn.setup().roots[screen_num].root;

    // Subscribe to display-configuration changes for the daemon's DisplayChanged event.
    let _ = conn.randr_select_input(setup_root, randr::NotifyMask::SCREEN_CHANGE);

    // Control keycodes for ctrl_down (row 2 of the modifier mapping = Control).
    let control_keycodes = control_keycodes(&conn);

    let mut env = X11Cursor {
        conn: &conn,
        root: setup_root,
        clip: None,
        control_keycodes,
        started: Instant::now(),
    };

    let mut selected = false;
    let mut prev: Option<(i32, i32)> = None;

    eprintln!("[LittleBigMouse.Hook] x11 backend running");

    while !shared.want_quit.load(Ordering::SeqCst) {
        // Reconcile the RawMotion subscription with the desired hook state.
        let want = shared.want_hook.load(Ordering::SeqCst);
        if want != selected {
            if select_raw_motion(&conn, setup_root, want) {
                selected = want;
                shared.hooked.store(want, Ordering::SeqCst);
                if want {
                    crate::platform::set_process_priority(crate::priority::Priority::from_u8(
                        shared.priority.load(Ordering::SeqCst),
                    ));
                    shared.broadcast(protocol::RUNNING);
                } else {
                    // Feed the engine a final running=false event so it restores
                    // any clip it holds, then drop ours.
                    let mut e = MouseEventArg::new(Point::default());
                    e.running = false;
                    let mut engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
                    engine.on_mouse_move(&mut env, &mut e);
                    drop(engine);
                    env.clip = None;
                    prev = None;
                    crate::platform::set_process_priority(crate::priority::Priority::from_u8(
                        shared.priority_unhooked.load(Ordering::SeqCst),
                    ));
                    shared.broadcast(protocol::STOPPED);
                }
            } else {
                shared.hooked.store(false, Ordering::SeqCst);
                shared.broadcast(protocol::STOPPED);
            }
        }

        // Wait on the X socket with a timeout so the reconcile above stays responsive.
        wait_readable(&conn, 100);

        let mut motion = false;
        while let Ok(Some(event)) = conn.poll_for_event() {
            match event {
                Event::XinputRawMotion(_) => motion = true,
                Event::RandrScreenChangeNotify(_) => {
                    crate::hook::on_display_changed(shared);
                }
                _ => {}
            }
        }

        // Raw motions carry device deltas; the engine wants the absolute cursor.
        // One QueryPointer per batch (the pump-side dedup the Windows hook does
        // per event happens here per wakeup, which is even cheaper).
        if motion && selected {
            let pos = env.query_pointer();

            // Win32 ClipCursor semantics: the OS never lets the cursor leave the
            // clip. X11 has no such rect, so enforce it before the engine looks.
            let pos = match env.clip {
                Some(clip) if !contains(&clip, pos) => {
                    let clamped = clamp(&clip, pos);
                    env.warp(clamped);
                    clamped
                }
                _ => pos,
            };

            let loc = (pos.x(), pos.y());
            if prev != Some(loc) {
                prev = Some(loc);
                crate::hook::MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);

                let mut engine = match shared.engine.try_lock() {
                    Ok(g) => g,
                    Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => continue,
                };
                let mut e = MouseEventArg::new(pos);
                engine.on_mouse_move(&mut env, &mut e);
                if e.handled {
                    crate::hook::CROSSINGS.fetch_add(1, Ordering::Relaxed);
                    // The engine warped the cursor: resync the dedup state.
                    prev = None;
                }
            }
        }
    }
}

fn select_raw_motion(conn: &RustConnection, root: u32, on: bool) -> bool {
    let mask = if on {
        xinput::XIEventMask::RAW_MOTION
    } else {
        xinput::XIEventMask::default()
    };
    let result = conn.xinput_xi_select_events(
        root,
        &[xinput::EventMask {
            deviceid: xinput::Device::ALL_MASTER.into(),
            mask: vec![mask],
        }],
    );
    match result.map(|c| c.check()) {
        Ok(Ok(())) => true,
        _ => {
            eprintln!("[LittleBigMouse.Hook] x11: XISelectEvents failed (no XInput2?)");
            false
        }
    }
}

/// Block until the X socket is readable or `timeout_ms` elapses.
fn wait_readable(conn: &RustConnection, timeout_ms: i32) {
    let fd = conn.stream().as_raw_fd();
    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    unsafe {
        libc::poll(&mut pfd, 1, timeout_ms);
    }
}

/// Keycodes bound to the Control modifier (row 2 of the 8x`keycodes_per_modifier`
/// modifier mapping: Shift, Lock, Control, Mod1..Mod5).
fn control_keycodes(conn: &RustConnection) -> Vec<u8> {
    let Ok(cookie) = conn.get_modifier_mapping() else {
        return Vec::new();
    };
    let Ok(reply): Result<GetModifierMappingReply, _> = cookie.reply() else {
        return Vec::new();
    };
    let per = reply.keycodes_per_modifier() as usize;
    reply.keycodes[2 * per..3 * per]
        .iter()
        .copied()
        .filter(|k| *k != 0)
        .collect()
}

fn contains(r: &Rect<i32>, p: Point<i32>) -> bool {
    p.x() >= r.left() && p.x() < r.right() && p.y() >= r.top() && p.y() < r.bottom()
}

fn clamp(r: &Rect<i32>, p: Point<i32>) -> Point<i32> {
    Point::new(
        p.x().clamp(r.left(), r.right() - 1),
        p.y().clamp(r.top(), r.bottom() - 1),
    )
}

struct X11Cursor<'c> {
    conn: &'c RustConnection,
    root: u32,
    /// Emulated Win32 clip; `None` = unclipped (the whole virtual screen).
    clip: Option<Rect<i32>>,
    control_keycodes: Vec<u8>,
    started: Instant,
}

impl X11Cursor<'_> {
    fn query_pointer(&self) -> Point<i32> {
        if let Ok(cookie) = self.conn.query_pointer(self.root) {
            if let Ok(reply) = cookie.reply() {
                return Point::new(reply.root_x as i32, reply.root_y as i32);
            }
        }
        Point::empty()
    }

    fn warp(&self, p: Point<i32>) {
        let _ = self
            .conn
            .warp_pointer(x11rb::NONE, self.root, 0, 0, 0, 0, p.x() as i16, p.y() as i16);
        let _ = self.conn.flush();
    }

    fn virtual_screen(&self) -> Rect<i32> {
        if let Ok(cookie) = self.conn.get_geometry(self.root) {
            if let Ok(geometry) = cookie.reply() {
                return Rect::new(
                    geometry.x as i32,
                    geometry.y as i32,
                    geometry.width as i32,
                    geometry.height as i32,
                );
            }
        }
        Rect::empty()
    }
}

impl CursorEnv for X11Cursor<'_> {
    fn get_mouse_location(&self) -> Point<i32> {
        let pos = self.query_pointer();
        // Under an emulated clip Win32 would never report an outside position.
        match self.clip {
            Some(clip) if !contains(&clip, pos) => clamp(&clip, pos),
            _ => pos,
        }
    }

    fn set_mouse_location(&mut self, location: Point<i32>) {
        self.warp(location);
    }

    fn get_clip(&self) -> Rect<i32> {
        // Win32 GetClipCursor returns the virtual screen when unclipped.
        self.clip.unwrap_or_else(|| self.virtual_screen())
    }

    fn set_clip(&mut self, r: Rect<i32>) {
        // Win32 ClipCursor clamps the current cursor into the new clip at once —
        // the engine's travel path relies on exactly that (set_clip then read).
        if r.is_empty() || r == self.virtual_screen() {
            self.clip = None;
            return;
        }
        self.clip = Some(r);
        let pos = self.query_pointer();
        if !contains(&r, pos) {
            self.warp(clamp(&r, pos));
        }
    }

    fn ctrl_down(&self) -> bool {
        let Ok(cookie) = self.conn.query_keymap() else {
            return false;
        };
        let Ok(reply) = cookie.reply() else {
            return false;
        };
        self.control_keycodes
            .iter()
            .any(|k| reply.keys[(*k as usize) / 8] & (1 << (*k as usize % 8)) != 0)
    }

    fn cursor_hidden(&self) -> bool {
        false // freelook detection not implemented on X11 yet
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        false // freelook detection not implemented on X11 yet
    }

    fn tick_count(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}
