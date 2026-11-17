//! Output-bound layer-shell surfaces, logical placement and fractional-scale buffers.
use crate::{raster, scene::Band, Backend};
use std::{
    collections::HashMap,
    io::Write,
    os::fd::{AsFd, AsRawFd, FromRawFd},
};
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_compositor::WlCompositor,
        wl_output::{self, WlOutput},
        wl_region::WlRegion,
        wl_registry::{self, WlRegistry},
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols::{
    wp::{
        fractional_scale::v1::client::{
            wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
            wp_fractional_scale_v1::{self, WpFractionalScaleV1},
        },
        viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
    },
    xdg::xdg_output::zv1::client::{
        zxdg_output_manager_v1::ZxdgOutputManagerV1,
        zxdg_output_v1::{self, ZxdgOutputV1},
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

fn err(e: impl std::fmt::Display) -> String {
    format!("Wayland overlay: {e}")
}
#[derive(Default)]
struct Output {
    name: Option<String>,
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    scale: i32,
}
#[derive(Clone, Copy)]
struct Tag {
    generation: u64,
    index: usize,
}
#[derive(Default)]
struct SurfaceState {
    size: Option<(u32, u32)>,
    scale: Option<u32>,
}
#[derive(Default)]
struct State {
    outputs: HashMap<u32, Output>,
    generation: u64,
    surfaces: Vec<SurfaceState>,
    buffers: Vec<(WlBuffer, Tag)>,
    armed: bool,
    changed: bool,
    closed: bool,
}
struct Surface {
    surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    viewport: WpViewport,
    fractional: Option<WpFractionalScaleV1>,
    logical: Band,
    painted: Option<Band>,
}
pub(super) struct Host {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
    compositor: WlCompositor,
    shm: WlShm,
    shell: ZwlrLayerShellV1,
    viewporter: WpViewporter,
    fractional: Option<WpFractionalScaleManagerV1>,
    outputs: HashMap<u32, (WlOutput, ZxdgOutputV1)>,
    surfaces: Vec<Surface>,
    desired: Vec<Band>,
}
impl Host {
    pub fn connect() -> Result<Self, String> {
        let connection = Connection::connect_to_env().map_err(err)?;
        let (globals, mut queue) = registry_queue_init::<State>(&connection).map_err(err)?;
        let q = queue.handle();
        let compositor = globals
            .bind::<WlCompositor, _, _>(&q, 4..=6, ())
            .map_err(err)?;
        let shm = globals.bind::<WlShm, _, _>(&q, 1..=1, ()).map_err(err)?;
        let shell = globals
            .bind::<ZwlrLayerShellV1, _, _>(&q, 1..=4, ())
            .map_err(|e| {
                err(format!(
                    "This compositor cannot place screen overlays (layer-shell unavailable: {e})."
                ))
            })?;
        let manager = globals
            .bind::<ZxdgOutputManagerV1, _, _>(&q, 2..=3, ())
            .map_err(err)?;
        let viewporter = globals
            .bind::<WpViewporter, _, _>(&q, 1..=1, ())
            .map_err(err)?;
        let fractional = globals
            .bind::<WpFractionalScaleManagerV1, _, _>(&q, 1..=1, ())
            .ok();
        let mut state = State::default();
        let mut outputs = HashMap::new();
        for global in globals.contents().clone_list() {
            if global.interface == "wl_output" && global.version >= 2 {
                let output = globals.registry().bind::<WlOutput, _, _>(
                    global.name,
                    global.version.min(4),
                    &q,
                    global.name,
                );
                let xdg = manager.get_xdg_output(&output, &q, global.name);
                state.outputs.insert(
                    global.name,
                    Output {
                        scale: 1,
                        ..Default::default()
                    },
                );
                outputs.insert(global.name, (output, xdg));
            }
        }
        queue.roundtrip(&mut state).map_err(err)?;
        state.armed = true;
        Ok(Self {
            connection,
            queue,
            state,
            compositor,
            shm,
            shell,
            viewporter,
            fractional,
            outputs,
            surfaces: Vec::new(),
            desired: Vec::new(),
        })
    }
    fn output(&self, band: &Band) -> Result<(u32, u32), String> {
        let (id, output) = self
            .state
            .outputs
            .iter()
            .find(|(_, o)| o.name.as_deref() == Some(&band.connector))
            .ok_or_else(|| err(format!("Output {} is unavailable.", band.connector)))?;
        let s = band.screen;
        if output.position != Some((s.x, s.y)) || output.size != Some((s.width, s.height)) {
            return Err(err("The logical output geometry changed. Reload the displays before showing overlays again."));
        }
        Ok((*id, (output.scale.max(1) as u32) * 120))
    }
    fn create(&mut self, band: &Band) -> Result<(), String> {
        let (id, _) = self.output(band)?;
        // Initial placement does not depend on the compositor's preferred buffer scale.
        let (logical, _) = super::wayland_geometry(band, (band.scale * 120.).round() as u32)?;
        let q = self.queue.handle();
        let tag = Tag {
            generation: self.state.generation,
            index: self.surfaces.len(),
        };
        let surface = self.compositor.create_surface(&q, ());
        let layer = self.shell.get_layer_surface(
            &surface,
            Some(&self.outputs[&id].0),
            Layer::Overlay,
            "littlebigmouse-overlays".into(),
            &q,
            tag,
        );
        let viewport = self.viewporter.get_viewport(&surface, &q, ());
        let fractional = self
            .fractional
            .as_ref()
            .map(|manager| manager.get_fractional_scale(&surface, &q, tag));
        let r = logical.rect;
        let s = logical.screen;
        layer.set_anchor(Anchor::Top | Anchor::Left);
        layer.set_margin(r.y - s.y, 0, 0, r.x - s.x);
        layer.set_size(r.width as u32, r.height as u32);
        // -1 includes panel-exclusive areas: these instruments follow physical screen edges.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let region = self.compositor.create_region(&q, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        surface.set_buffer_scale(1);
        viewport.set_destination(r.width, r.height);
        self.state.surfaces.push(SurfaceState::default());
        surface.commit();
        self.surfaces.push(Surface {
            surface,
            layer,
            viewport,
            fractional,
            logical,
            painted: None,
        });
        Ok(())
    }
    fn check(&self) -> Result<(), String> {
        if self.state.closed {
            return Err(err("The compositor closed an overlay surface."));
        }
        if self.state.changed {
            return Err(err("Displays or surface scale changed. Reload the screens before showing overlays again."));
        }
        Ok(())
    }
    fn draw(&mut self) -> Result<(), String> {
        for index in 0..self.desired.len() {
            let band = &self.desired[index];
            if self.surfaces[index].painted.as_ref() == Some(band) {
                continue;
            }
            // Never write into a buffer owned by the compositor. Keep at most three per surface,
            // and retain only the newest desired scene while waiting for a release event.
            if self
                .state
                .buffers
                .iter()
                .filter(|(_, t)| t.index == index && t.generation == self.state.generation)
                .count()
                >= 3
            {
                continue;
            }
            let (_, integer_scale) = self.output(band)?;
            let state = &self.state.surfaces[index];
            let preferred = if self.fractional.is_some() {
                state.scale.ok_or_else(|| {
                    err("The compositor did not provide a preferred buffer scale.")
                })?
            } else {
                integer_scale
            };
            let (logical, buffer_band) = super::wayland_geometry(band, preferred)?;
            let rect = logical.rect;
            let configured = state
                .size
                .ok_or_else(|| err("The compositor did not configure the overlay."))?;
            // A zero configure dimension explicitly lets the client choose that dimension.
            if (configured.0 != 0 && configured.0 != rect.width as u32)
                || (configured.1 != 0 && configured.1 != rect.height as u32)
            {
                return Err(err("The compositor changed the requested overlay size."));
            }
            let image = raster::render(&buffer_band)?;
            let tag = Tag {
                generation: self.state.generation,
                index,
            };
            let buffer = self.buffer(image, tag)?;
            let surface = &self.surfaces[index].surface;
            surface.attach(Some(&buffer), 0, 0);
            surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            surface.commit();
            self.state.buffers.push((buffer, tag));
            self.surfaces[index].painted = Some(band.clone());
        }
        self.connection.flush().map_err(err)
    }
    fn buffer(&self, mut image: raster::Image, tag: Tag) -> Result<WlBuffer, String> {
        // wl_shm ARGB8888 is native-endian; the shared renderer emits little-endian BGRA.
        if cfg!(target_endian = "big") {
            for pixel in image.bgra.as_chunks_mut::<4>().0 {
                pixel.reverse();
            }
        }
        // SAFETY: constant NUL-terminated name; on success ownership transfers to File exactly once.
        let fd = unsafe { libc::memfd_create(c"lbm-overlay".as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            return Err(err(std::io::Error::last_os_error()));
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        file.write_all(&image.bgra).map_err(err)?;
        let q = self.queue.handle();
        let pool = self
            .shm
            .create_pool(file.as_fd(), image.bgra.len() as i32, &q, ());
        let buffer = pool.create_buffer(
            0,
            image.width as i32,
            image.height as i32,
            (image.width * 4) as i32,
            Format::Argb8888,
            &q,
            tag,
        );
        pool.destroy();
        Ok(buffer)
    }
}
impl Backend for Host {
    fn present(&mut self, bands: &[Band]) -> Result<(), String> {
        super::validate(bands)?;
        self.check()?;
        for band in bands {
            self.output(band)?;
        }
        let geometry = bands
            .iter()
            .map(|b| super::wayland_geometry(b, (b.scale * 120.).round() as u32).map(|(b, _)| b))
            .collect::<Result<Vec<_>, _>>()?;
        let rebuild = geometry.len() != self.surfaces.len()
            || geometry
                .iter()
                .zip(&self.surfaces)
                .any(|(b, s)| b.rect != s.logical.rect || b.connector != s.logical.connector);
        if rebuild {
            self.clear();
            for band in bands {
                self.create(band)?;
            }
            // Configuration may be deferred until the compositor's next frame. The host's
            // watchdog bounds this wait without ever blocking the editor's event loop.
            self.queue.roundtrip(&mut self.state).map_err(err)?;
            while self
                .state
                .surfaces
                .iter()
                .any(|s| s.size.is_none() || (self.fractional.is_some() && s.scale.is_none()))
            {
                self.check()?;
                self.queue.blocking_dispatch(&mut self.state).map_err(err)?;
            }
            self.check()?;
        }
        self.desired = bands.to_vec();
        self.draw()
    }
    fn clear(&mut self) {
        for s in self.surfaces.drain(..) {
            if let Some(f) = s.fractional {
                f.destroy();
            }
            s.viewport.destroy();
            s.layer.destroy();
            s.surface.destroy();
        }
        for (buffer, _) in self.state.buffers.drain(..) {
            buffer.destroy();
        }
        self.state.generation += 1;
        self.state.surfaces.clear();
        self.desired.clear();
        let _ = self.connection.flush();
    }
    fn poll(&mut self) -> Result<(), String> {
        self.queue.dispatch_pending(&mut self.state).map_err(err)?;
        self.connection.flush().map_err(err)?;
        if let Some(read) = self.queue.prepare_read() {
            let mut fd = libc::pollfd {
                fd: self.connection.as_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: one valid pollfd, no timeout; the read guard excludes competing readers.
            let ready = unsafe { libc::poll(&mut fd, 1, 0) };
            if ready < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() != std::io::ErrorKind::Interrupted {
                    return Err(err(e));
                }
            }
            if ready > 0 {
                read.read().map_err(err)?;
            }
        }
        self.queue.dispatch_pending(&mut self.state).map_err(err)?;
        self.check()?;
        self.draw()
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        s: &mut Self,
        _: &WlRegistry,
        e: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if !s.armed {
            return;
        }
        match e {
            wl_registry::Event::Global { interface, .. } if interface == "wl_output" => {
                s.changed = true
            }
            wl_registry::Event::GlobalRemove { name } if s.outputs.contains_key(&name) => {
                s.changed = true
            }
            _ => {}
        }
    }
}
impl Dispatch<WlOutput, u32> for State {
    fn event(
        s: &mut Self,
        _: &WlOutput,
        e: wl_output::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(o) = s.outputs.get_mut(id) else {
            return;
        };
        let changed = match e {
            wl_output::Event::Name { name } => {
                let c = o.name.as_ref() != Some(&name);
                o.name = Some(name);
                c
            }
            wl_output::Event::Scale { factor } => {
                let c = o.scale != factor;
                o.scale = factor;
                c
            }
            // Mode/transform changes invalidate even when their logical dimensions stay the same.
            wl_output::Event::Geometry { .. } | wl_output::Event::Mode { .. } => true,
            _ => false,
        };
        s.changed |= s.armed && changed;
    }
}
impl Dispatch<ZxdgOutputV1, u32> for State {
    fn event(
        s: &mut Self,
        _: &ZxdgOutputV1,
        e: zxdg_output_v1::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(o) = s.outputs.get_mut(id) else {
            return;
        };
        let changed = match e {
            zxdg_output_v1::Event::Name { name } => {
                let c = o.name.as_ref() != Some(&name);
                o.name = Some(name);
                c
            }
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                let c = o.position != Some((x, y));
                o.position = Some((x, y));
                c
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                let c = o.size != Some((width, height));
                o.size = Some((width, height));
                c
            }
            _ => false,
        };
        s.changed |= s.armed && changed;
    }
}
impl Dispatch<ZwlrLayerSurfaceV1, Tag> for State {
    fn event(
        s: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        e: zwlr_layer_surface_v1::Event,
        tag: &Tag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if tag.generation != s.generation {
            return;
        }
        let Some(surface) = s.surfaces.get_mut(tag.index) else {
            return;
        };
        match e {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                s.changed |= surface.size.is_some_and(|old| old != (width, height));
                surface.size = Some((width, height));
            }
            zwlr_layer_surface_v1::Event::Closed => s.closed = true,
            _ => {}
        }
    }
}
impl Dispatch<WpFractionalScaleV1, Tag> for State {
    fn event(
        s: &mut Self,
        _: &WpFractionalScaleV1,
        e: wp_fractional_scale_v1::Event,
        tag: &Tag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if tag.generation != s.generation {
            return;
        }
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = e {
            if let Some(surface) = s.surfaces.get_mut(tag.index) {
                s.changed |= surface.scale.is_some_and(|old| old != scale);
                surface.scale = Some(scale);
            }
        }
    }
}
impl Dispatch<WlBuffer, Tag> for State {
    fn event(
        s: &mut Self,
        buffer: &WlBuffer,
        e: wl_buffer::Event,
        _: &Tag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = e {
            if let Some(index) = s.buffers.iter().position(|(b, _)| b == buffer) {
                let (b, _) = s.buffers.swap_remove(index);
                b.destroy();
            }
        }
    }
}
delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore ZwlrLayerShellV1);
delegate_noop!(State: ignore ZxdgOutputManagerV1);
delegate_noop!(State: ignore WpViewporter);
delegate_noop!(State: ignore WpViewport);
delegate_noop!(State: ignore WpFractionalScaleManagerV1);
