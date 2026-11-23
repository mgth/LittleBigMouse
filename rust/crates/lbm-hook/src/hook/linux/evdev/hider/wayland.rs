//! The Wayland hider: a layer-shell surface under the frozen pointer that sets no
//! cursor. See the module above for why it is shaped so.

use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};

use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_output::WlOutput,
        wl_pointer::{self, WlPointer},
        wl_registry::{self, WlRegistry},
        wl_seat::{self, WlSeat},
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::{self, ZxdgOutputV1},
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use super::{place, Missing, Placement, SIDE};
use crate::remote::parking::Screen;

const LOG: &str = "[LittleBigMouse.Hook] cursor hider (Wayland)";

struct Output {
    /// The registry name of its `wl_output` global.
    global: u32,
    output: WlOutput,
    xdg: ZxdgOutputV1,
    /// Logical position and size, in the compositor's desktop space: the space the
    /// pump's cursor lives in.
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
}

/// The surface while it hides the cursor.
struct Shown {
    surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    /// The output it is on, by global name, and where on it.
    global: u32,
    placement: Placement,
    /// Configured and given its buffer: the pointer may be moved onto it.
    configured: bool,
    /// The pointer entered it, and it set no cursor.
    entered: bool,
}

#[derive(Default)]
struct State {
    outputs: Vec<Output>,
    xdg_manager: Option<ZxdgOutputManagerV1>,
    pointer: Option<WlPointer>,
    buffer: Option<WlBuffer>,
    shown: Option<Shown>,
    /// The compositor closed the surface: it is to be made again.
    closed: bool,
}

impl State {
    fn add_output(
        &mut self,
        registry: &WlRegistry,
        global: u32,
        version: u32,
        q: &QueueHandle<Self>,
    ) {
        let Some(manager) = &self.xdg_manager else {
            return;
        };
        let output = registry.bind::<WlOutput, _, _>(global, version.min(4), q, ());
        let xdg = manager.get_xdg_output(&output, q, global);
        self.outputs.push(Output {
            global,
            output,
            xdg,
            position: None,
            size: None,
        });
    }
}

pub(super) struct Hider {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
    compositor: WlCompositor,
    shell: ZwlrLayerShellV1,
    /// The output the last request could not be placed on was said; not again.
    told_nowhere: bool,
}

fn lost(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Hider {
    pub(super) fn connect() -> Result<Hider, Missing> {
        let unreachable = |e: &dyn std::fmt::Display| {
            Missing::Unreachable(format!("cannot reach the Wayland compositor ({e})"))
        };
        let connection = Connection::connect_to_env().map_err(|e| unreachable(&e))?;
        let (globals, mut queue) =
            registry_queue_init::<State>(&connection).map_err(|e| unreachable(&e))?;
        let q = queue.handle();
        let missing = |what: &str| Missing::Unsupported(format!("the compositor has no {what}"));
        let compositor = globals
            .bind::<WlCompositor, _, _>(&q, 4..=6, ())
            .map_err(|_| missing("wl_compositor"))?;
        let shm = globals
            .bind::<WlShm, _, _>(&q, 1..=1, ())
            .map_err(|_| missing("wl_shm"))?;
        let shell = globals
            .bind::<ZwlrLayerShellV1, _, _>(&q, 1..=4, ())
            .map_err(|_| missing("zwlr_layer_shell_v1"))?;
        let xdg_manager = globals
            .bind::<ZxdgOutputManagerV1, _, _>(&q, 2..=3, ())
            .map_err(|_| missing("zxdg_output_manager_v1"))?;
        globals
            .bind::<WlSeat, _, _>(&q, 1..=8, ())
            .map_err(|_| missing("wl_seat"))?;

        let mut state = State {
            xdg_manager: Some(xdg_manager),
            ..State::default()
        };
        for global in globals.contents().clone_list() {
            if global.interface == "wl_output" {
                state.add_output(globals.registry(), global.name, global.version, &q);
            }
        }
        state.buffer = Some(transparent_buffer(&shm, &q).map_err(|e| {
            Missing::Unreachable(format!("no shared memory for the hiding surface ({e})"))
        })?);
        // The outputs' logical geometry and the seat's pointer.
        queue.roundtrip(&mut state).map_err(|e| unreachable(&e))?;
        queue.roundtrip(&mut state).map_err(|e| unreachable(&e))?;
        if state.pointer.is_none() {
            eprintln!(
                "{LOG}: the seat has no pointer yet; the cursor cannot be hidden until it has"
            );
        }
        eprintln!("{LOG}: connected, {} output(s)", state.outputs.len());
        Ok(Hider {
            connection,
            queue,
            state,
            compositor,
            shell,
            told_nowhere: false,
        })
    }

    /// The outputs whose geometry is known, as screens, with their globals.
    fn screens(&self) -> (Vec<Screen>, Vec<u32>) {
        self.state
            .outputs
            .iter()
            .filter_map(|o| {
                let ((x, y), (w, h)) = (o.position?, o.size?);
                Some((
                    Screen {
                        left: x,
                        top: y,
                        right: x.saturating_add(w),
                        bottom: y.saturating_add(h),
                    },
                    o.global,
                ))
            })
            .unzip()
    }

    pub(super) fn reconcile(
        &mut self,
        hide: Option<(i32, i32)>,
    ) -> Result<Option<(i32, i32)>, String> {
        let target = hide.and_then(|from| {
            let (screens, globals) = self.screens();
            let placed = place(from, &screens).map(|p| (globals[p.output], p));
            if placed.is_none() && !self.told_nowhere {
                self.told_nowhere = true;
                eprintln!(
                    "{LOG}: no output to hide the cursor on, for ({},{})",
                    from.0, from.1
                );
            }
            placed
        });
        let current = self
            .state
            .shown
            .as_ref()
            .filter(|_| !self.state.closed)
            .map(|s| (s.global, s.placement));
        if current != target {
            self.destroy(if target.is_some() {
                "moved"
            } else {
                "the pointer is home"
            });
            if let Some((global, placement)) = target {
                self.create(global, placement);
            }
        }
        self.connection.flush().or_else(would_block).map_err(lost)?;
        Ok(self
            .state
            .shown
            .as_ref()
            .filter(|s| s.configured)
            .map(|s| s.placement.spot))
    }

    fn create(&mut self, global: u32, placement: Placement) {
        let Some(output) = self.state.outputs.iter().find(|o| o.global == global) else {
            return;
        };
        let q = self.queue.handle();
        let surface = self.compositor.create_surface(&q, ());
        // The namespace KWin reads as an on-screen display, as lbm-overlay's surfaces
        // do: no open animation, and pointer focus like any surface with an input
        // region. The default input region is the whole surface, which is what is
        // wanted; keyboard: none, so the focused window keeps typing.
        let layer = self.shell.get_layer_surface(
            &surface,
            Some(&output.output),
            Layer::Overlay,
            "on-screen-display".into(),
            &q,
            (),
        );
        layer.set_anchor(Anchor::Top | Anchor::Left);
        layer.set_margin(placement.top, 0, 0, placement.left);
        layer.set_size(SIDE as u32, SIDE as u32);
        // Margins from the output's edges, not from what panels leave free.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        surface.commit();
        self.state.closed = false;
        self.state.shown = Some(Shown {
            surface,
            layer,
            global,
            placement,
            configured: false,
            entered: false,
        });
    }

    fn destroy(&mut self, why: &str) {
        if let Some(shown) = self.state.shown.take() {
            shown.layer.destroy();
            shown.surface.destroy();
            eprintln!("{LOG}: surface destroyed ({why}), the compositor draws the cursor again");
        }
    }

    pub(super) fn wait(&mut self, wake: RawFd) -> Result<(), String> {
        self.queue.dispatch_pending(&mut self.state).map_err(lost)?;
        self.connection.flush().or_else(would_block).map_err(lost)?;
        // Events already queued: handled on the next turn, without sleeping.
        let Some(guard) = self.queue.prepare_read() else {
            return Ok(());
        };
        let mut fds = [
            libc::pollfd {
                fd: guard.connection_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        super::poll(&mut fds);
        if fds[0].revents != 0 {
            guard
                .read()
                .map(|_| ())
                .or_else(would_block)
                .map_err(lost)?;
        }
        self.queue.dispatch_pending(&mut self.state).map_err(lost)?;
        // A configure handled above attached a buffer.
        self.connection.flush().or_else(would_block).map_err(lost)?;
        Ok(())
    }
}

impl Drop for Hider {
    fn drop(&mut self) {
        self.destroy("disconnecting");
        let _ = self.connection.flush();
    }
}

fn would_block(
    e: wayland_client::backend::WaylandError,
) -> Result<(), wayland_client::backend::WaylandError> {
    match e {
        wayland_client::backend::WaylandError::Io(io)
            if io.kind() == std::io::ErrorKind::WouldBlock =>
        {
            Ok(())
        }
        e => Err(e),
    }
}

/// One fully transparent buffer of the surface's size, shared by every surface the
/// connection makes: the compositor never reads anything else from it.
fn transparent_buffer(shm: &WlShm, q: &QueueHandle<State>) -> std::io::Result<WlBuffer> {
    let stride = SIDE * 4;
    let len = stride * SIDE;
    let fd = unsafe { libc::memfd_create(c"lbm-cursor-hider".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    // Zero-filled: ARGB 0, transparent.
    if unsafe { libc::ftruncate(fd.as_raw_fd(), libc::off_t::from(len)) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let pool = shm.create_pool(fd.as_fd(), len, q, ());
    let buffer = pool.create_buffer(0, SIDE, SIDE, stride, Format::Argb8888, q, ());
    pool.destroy();
    Ok(buffer)
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        q: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "wl_output" => state.add_output(registry, name, version, q),
            // A surface on that output is closed by the compositor, and made again on
            // another one.
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(i) = state.outputs.iter().position(|o| o.global == name) {
                    let gone = state.outputs.swap_remove(i);
                    gone.xdg.destroy();
                    if gone.output.version() >= 3 {
                        gone.output.release();
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputV1, u32> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state.outputs.iter_mut().find(|o| o.global == *global) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => output.position = Some((x, y)),
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                output.size = Some((width, height))
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(shown) = state.shown.as_mut().filter(|s| &s.layer == layer) else {
            return;
        };
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                if !shown.configured {
                    shown.surface.attach(state.buffer.as_ref(), 0, 0);
                    shown.surface.damage_buffer(0, 0, SIDE, SIDE);
                    shown.configured = true;
                    let p = shown.placement;
                    eprintln!(
                        "{LOG}: surface configured {width}x{height} at +{},+{} on its output, for the pointer at ({},{})",
                        p.left, p.top, p.spot.0, p.spot.1
                    );
                }
                shown.surface.commit();
            }
            zwlr_layer_surface_v1::Event::Closed => {
                eprintln!("{LOG}: the compositor closed the surface");
                state.closed = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        q: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            let has = caps.contains(wl_seat::Capability::Pointer);
            if has && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(q, ()));
            } else if !has {
                if let Some(pointer) = state.pointer.take().filter(|p| p.version() >= 3) {
                    pointer.release();
                }
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        pointer: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Enter {
            serial,
            surface,
            surface_x,
            surface_y,
        } = event
        {
            let Some(shown) = state.shown.as_mut().filter(|s| s.surface == surface) else {
                return;
            };
            // No cursor surface: the compositor draws none while the pointer is here.
            pointer.set_cursor(serial, None, 0, 0);
            if !shown.entered {
                shown.entered = true;
                eprintln!(
                    "{LOG}: the pointer entered the surface at {surface_x:.1},{surface_y:.1}, cursor hidden"
                );
            }
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore WlOutput);
delegate_noop!(State: ignore ZwlrLayerShellV1);
delegate_noop!(State: ignore ZxdgOutputManagerV1);
