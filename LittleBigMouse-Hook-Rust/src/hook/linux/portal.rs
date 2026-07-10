//! Wayland backend: the `org.freedesktop.portal.InputCapture` portal + libei.
//!
//! The portal model maps almost 1:1 onto LBM's zone graph: every zone-link edge
//! becomes a pointer barrier. When the cursor hits one, the compositor freezes
//! it there, grants us the capture and streams **relative** motions over the ei
//! protocol. We integrate them into a *virtual* cursor position, feed the
//! unchanged engine with it (resistance, Strait/CornerCrossing, travel — all
//! pure logic), and end the capture with `Release(cursor_position)`:
//! - the engine crossed → release at the computed target: the compositor's
//!   release-with-position IS the warp;
//! - the user retreated back into the origin zone → release there, the cursor
//!   never appears to move.
//!
//! The engine's Win32 clip semantics are emulated on the virtual position (the
//! compositor already holds the real cursor at the barrier while captured).
//!
//! Not mapped: ctrl-override (no global keyboard state without capturing the
//! keyboard) and focus-based exclusion — both documented Windows/X11-only.

use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;
use std::time::Instant;

use ashpd::desktop::input_capture::{
    Barrier, BarrierID, BarrierPosition, Capabilities, CreateSessionOptions, InputCapture,
    ReleaseOptions,
};
use ashpd::desktop::Session;
use futures::StreamExt;
use reis::ei;
use reis::event::{DeviceCapability, EiEvent};

use crate::engine::cursor::CursorEnv;
use crate::engine::event::MouseEventArg;
use crate::geometry::{Point, Rect};
use crate::ipc::protocol;
use crate::shared::Shared;

/// True in a Wayland session (where the portal is the only way to capture).
pub fn available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty())
}

/// Run the portal backend. Returns `false` if the portal could not be set up at
/// all (no portal service, no InputCapture support) so the caller can fall back
/// to the X11 backend.
pub fn run(shared: &'static Shared) -> bool {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[LittleBigMouse.Hook] portal: no tokio runtime: {e}");
            return false;
        }
    };
    runtime.block_on(run_async(shared))
}

async fn run_async(shared: &'static Shared) -> bool {
    eprintln!("[LittleBigMouse.Hook] portal: connecting...");
    let Ok(input_capture) = InputCapture::new().await else {
        eprintln!("[LittleBigMouse.Hook] portal: InputCapture portal unavailable");
        return false;
    };

    eprintln!("[LittleBigMouse.Hook] portal: proxy ok, creating session...");
    let session = match input_capture
        .create_session(
            None,
            CreateSessionOptions::default().set_capabilities(Capabilities::Pointer.into()),
        )
        .await
    {
        Ok((session, _capabilities)) => session,
        Err(e) => {
            eprintln!("[LittleBigMouse.Hook] portal: CreateSession failed: {e}");
            return false;
        }
    };

    eprintln!("[LittleBigMouse.Hook] portal: session ok, connecting eis...");
    // ei stream: the channel the captured motions arrive on.
    let Ok(fd) = input_capture.connect_to_eis(&session, Default::default()).await else {
        eprintln!("[LittleBigMouse.Hook] portal: ConnectToEIS failed");
        return false;
    };
    let stream = UnixStream::from(fd);
    if stream.set_nonblocking(true).is_err() {
        return false;
    }
    let Ok(context) = ei::Context::new(stream) else {
        return false;
    };
    let _ = context.flush();
    eprintln!("[LittleBigMouse.Hook] portal: eis fd ok, handshaking...");
    let Ok((_connection, mut ei_events)) = context
        .handshake_tokio("littlebigmouse", ei::handshake::ContextType::Receiver)
        .await
    else {
        eprintln!("[LittleBigMouse.Hook] portal: ei handshake failed");
        return false;
    };

    let (Ok(mut activated), Ok(mut deactivated), Ok(mut zones_changed)) = (
        input_capture.receive_activated().await,
        input_capture.receive_deactivated().await,
        input_capture.receive_zones_changed().await,
    ) else {
        return false;
    };

    eprintln!("[LittleBigMouse.Hook] portal backend running");

    let mut reconcile = tokio::time::interval(std::time::Duration::from_millis(100));
    let mut enabled = false;
    let mut capture: Option<Capture> = None;
    let mut env = PortalCursor {
        virtual_pos: Point::default(),
        clip: None,
        pending_warp: None,
        started: Instant::now(),
    };
    // Sub-pixel remainders of the relative motion integration.
    let (mut rem_x, mut rem_y) = (0f64, 0f64);

    loop {
        tokio::select! {
            _ = reconcile.tick() => {
                if shared.want_quit.load(Ordering::SeqCst) {
                    if enabled { let _ = input_capture.disable(&session, Default::default()).await; }
                    return true;
                }
                let want = shared.want_hook.load(Ordering::SeqCst);
                if want && !enabled {
                    match arm(&input_capture, &session, shared).await {
                        Ok(()) => {
                            enabled = true;
                            shared.hooked.store(true, Ordering::SeqCst);
                            shared.broadcast(protocol::RUNNING);
                        }
                        Err(e) => {
                            eprintln!("[LittleBigMouse.Hook] portal: arm failed: {e}");
                            shared.want_hook.store(false, Ordering::SeqCst);
                            shared.broadcast(protocol::STOPPED);
                        }
                    }
                } else if !want && enabled {
                    if let Some(c) = capture.take() {
                        release(&input_capture, &session, c.activation_id, None).await;
                    }
                    let _ = input_capture.disable(&session, Default::default()).await;
                    enabled = false;
                    // Final running=false event so the engine drops its state/clip.
                    let mut e = MouseEventArg::new(Point::default());
                    e.running = false;
                    shared.engine.lock().unwrap_or_else(|p| p.into_inner()).on_mouse_move(&mut env, &mut e);
                    env.clip = None;
                    shared.hooked.store(false, Ordering::SeqCst);
                    shared.broadcast(protocol::STOPPED);
                }
            }

            activation = activated.next() => {
                let Some(activation) = activation else { return true };
                let Some((x, y)) = activation.cursor_position() else { continue };
                let position = Point::new(x as i32, y as i32);
                eprintln!("[LittleBigMouse.Hook] portal: activated at ({},{}) barrier={:?}",
                    position.x(), position.y(), activation.barrier_id());
                capture = Some(Capture {
                    activation_id: activation.activation_id(),
                    origin: position,
                });
                env.virtual_pos = position;
                env.clip = None;
                env.pending_warp = None;
                (rem_x, rem_y) = (0.0, 0.0);
                // Prime the engine at the barrier position.
                feed_engine(shared, &mut env, position);
            }

            _ = deactivated.next() => {
                capture = None;
                env.clip = None;
            }

            _ = zones_changed.next() => {
                // Output topology changed: same semantics as WM_DISPLAYCHANGE —
                // unhook (the UI recomputes and reloads the layout, then re-Starts us).
                capture = None;
                crate::hook::on_display_changed(shared);
            }

            ei_event = ei_events.next() => {
                let Some(Ok(ei_event)) = ei_event else { return true };
                match ei_event {
                    EiEvent::SeatAdded(seat) => {
                        seat.seat.bind_capabilities(DeviceCapability::Pointer.into());
                        let _ = context.flush();
                    }
                    EiEvent::PointerMotion(motion) => {
                        rem_x += motion.dx as f64;
                        rem_y += motion.dy as f64;
                    }
                    EiEvent::Frame(_) => {
                        let Some(c) = &capture else { continue };
                        let (dx, dy) = (rem_x.round() as i32, rem_y.round() as i32);
                        if dx == 0 && dy == 0 { continue; }
                        rem_x -= dx as f64;
                        rem_y -= dy as f64;

                        let mut position = Point::new(env.virtual_pos.x() + dx, env.virtual_pos.y() + dy);
                        if let Some(clip) = env.clip {
                            position = clamp(&clip, position);
                        }
                        env.virtual_pos = position;

                        crate::hook::MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
                        let handled = feed_engine(shared, &mut env, position);

                        if let Some(target) = env.pending_warp.take() {
                            // The engine crossed: release the capture, placing the
                            // cursor at the computed target — this is the warp.
                            crate::hook::CROSSINGS.fetch_add(1, Ordering::Relaxed);
                            eprintln!("[LittleBigMouse.Hook] portal: crossing -> release at ({},{})", target.x(), target.y());
                            let c = capture.take().unwrap();
                            release(&input_capture, &session, c.activation_id, Some(target)).await;
                            env.clip = None;
                        } else if !handled && retreated(shared, c.origin, env.virtual_pos) {
                            eprintln!("[LittleBigMouse.Hook] portal: retreat -> release at ({},{})", env.virtual_pos.x(), env.virtual_pos.y());
                            // The user pulled back into the origin zone: hand the
                            // cursor back where the virtual position ended up.
                            let c = capture.take().unwrap();
                            release(&input_capture, &session, c.activation_id, Some(env.virtual_pos)).await;
                            env.clip = None;
                        }
                    }
                    EiEvent::Disconnected(_) => {
                        eprintln!("[LittleBigMouse.Hook] portal: ei disconnected");
                        return true;
                    }
                    _ => {}
                }
            }
        }
    }
}

struct Capture {
    activation_id: Option<u32>,
    origin: Point<i32>,
}

/// Recompute the barriers from the currently loaded layout and enable capture.
async fn arm(
    input_capture: &InputCapture,
    session: &Session<InputCapture>,
    shared: &Shared,
) -> Result<(), ashpd::Error> {
    let zones = input_capture.zones(session, Default::default()).await?.response()?;

    let barriers = {
        let engine = shared.engine.lock().unwrap_or_else(|p| p.into_inner());
        layout_barriers(&engine.layout)
    };
    if barriers.is_empty() {
        eprintln!("[LittleBigMouse.Hook] portal: no barriers (layout empty?)");
    }

    for b in &barriers {
        eprintln!("[LittleBigMouse.Hook] portal: barrier {:?}", b);
    }
    let failed = input_capture
        .set_pointer_barriers(session, &barriers, zones.zone_set(), Default::default())
        .await?
        .response()?;
    if !failed.failed_barriers().is_empty() {
        eprintln!(
            "[LittleBigMouse.Hook] portal: {} of {} barriers denied by the compositor: {:?}",
            failed.failed_barriers().len(),
            barriers.len(),
            failed.failed_barriers(),
        );
    }

    input_capture.enable(session, Default::default()).await?;
    Ok(())
}

/// One barrier per zone edge that carries at least one crossable link,
/// spanning the FULL edge: KWin's validator denies partial-edge segments
/// (observed live: full-edge barriers accepted, link-range segments denied).
/// The engine still decides per-position what actually crosses — an
/// activation outside any link range just holds and releases on retreat.
/// Twin edges of adjacent zones produce identical lines: dedup them.
fn layout_barriers(layout: &crate::zones::ZonesLayout) -> Vec<Barrier> {
    let mut barriers = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut next_id = 1u32;

    for &zone_id in &layout.main_zones {
        let zone = &layout.arena[zone_id];
        let bounds = zone.pixels_bounds();

        // (has crossable links, x1, y1, x2, y2) — portal convention: left/top
        // edge at the coordinate, right/bottom at one past the last pixel,
        // inclusive endpoints along the edge.
        let edges = [
            (&zone.left, bounds.left(), bounds.top(), bounds.left(), bounds.bottom() - 1),
            (&zone.right, bounds.right(), bounds.top(), bounds.right(), bounds.bottom() - 1),
            (&zone.top, bounds.left(), bounds.top(), bounds.right() - 1, bounds.top()),
            (&zone.bottom, bounds.left(), bounds.bottom(), bounds.right() - 1, bounds.bottom()),
        ];

        for (links, x1, y1, x2, y2) in edges {
            if !links.iter().any(|l| l.target.is_some()) {
                continue;
            }
            if !seen.insert((x1, y1, x2, y2)) {
                continue; // the twin edge of the adjacent zone already emitted it
            }
            if let Some(id) = BarrierID::new(next_id) {
                barriers.push(Barrier::new(id, BarrierPosition::new(x1, y1, x2, y2)));
                next_id += 1;
            }
        }
    }

    barriers
}

fn feed_engine(shared: &Shared, env: &mut PortalCursor, position: Point<i32>) -> bool {
    let mut engine = match shared.engine.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return false,
    };
    let mut e = MouseEventArg::new(position);
    engine.on_mouse_move(env, &mut e);
    e.handled
}

/// The user moved back into the zone they hit the barrier from: more than a few
/// pixels away from every edge means the crossing attempt is over.
fn retreated(shared: &Shared, origin: Point<i32>, position: Point<i32>) -> bool {
    const MARGIN: i32 = 8;
    let engine = match shared.engine.try_lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    for &zone_id in &engine.layout.main_zones {
        let bounds = engine.layout.arena[zone_id].pixels_bounds();
        if !bounds.contains(origin) {
            continue;
        }
        return position.x() >= bounds.left() + MARGIN
            && position.x() < bounds.right() - MARGIN
            && position.y() >= bounds.top() + MARGIN
            && position.y() < bounds.bottom() - MARGIN;
    }
    false
}

async fn release(
    input_capture: &InputCapture,
    session: &Session<InputCapture>,
    activation_id: Option<u32>,
    position: Option<Point<i32>>,
) {
    let mut options = ReleaseOptions::default().set_activation_id(activation_id);
    if let Some(p) = position {
        options = options.set_cursor_position((p.x() as f64, p.y() as f64));
    }
    if let Err(e) = input_capture.release(session, options).await {
        eprintln!("[LittleBigMouse.Hook] portal: release failed: {e}");
    }
}

fn clamp(r: &Rect<i32>, p: Point<i32>) -> Point<i32> {
    Point::new(
        p.x().clamp(r.left(), r.right() - 1),
        p.y().clamp(r.top(), r.bottom() - 1),
    )
}

/// The engine's cursor environment over the *virtual* position tracked from the
/// ei relative motions. `set_mouse_location` becomes the pending release
/// position (the compositor does the actual warp); the Win32 clip semantics are
/// emulated on the virtual position, including the clamp-on-set the engine's
/// travel path relies on.
struct PortalCursor {
    virtual_pos: Point<i32>,
    clip: Option<Rect<i32>>,
    pending_warp: Option<Point<i32>>,
    started: Instant,
}

impl CursorEnv for PortalCursor {
    fn get_mouse_location(&self) -> Point<i32> {
        self.virtual_pos
    }

    fn set_mouse_location(&mut self, location: Point<i32>) {
        self.virtual_pos = location;
        self.pending_warp = Some(location);
    }

    fn get_clip(&self) -> Rect<i32> {
        self.clip.unwrap_or_else(Rect::empty)
    }

    fn set_clip(&mut self, r: Rect<i32>) {
        if r.is_empty() {
            self.clip = None;
            return;
        }
        self.clip = Some(r);
        // Win32 ClipCursor clamps the cursor into the new clip immediately.
        self.virtual_pos = clamp(&r, self.virtual_pos);
    }

    fn ctrl_down(&self) -> bool {
        false // not observable on Wayland without capturing the keyboard
    }

    fn cursor_hidden(&self) -> bool {
        false
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        false
    }

    fn tick_count(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}
