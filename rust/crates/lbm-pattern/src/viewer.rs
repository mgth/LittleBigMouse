use std::fs::File;
use std::io::Write;
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_output::{self, WlOutput},
        wl_pointer::{self, WlPointer},
        wl_registry::WlRegistry,
        wl_seat::{self, WlSeat},
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};
#[derive(Default)]
struct OutputInfo {
    name: Option<String>,
    current_mode: Option<(i32, i32)>,
    rotated: bool,
}

struct State {
    outputs: Vec<(WlOutput, OutputInfo)>,
    outputs_done: bool,

    // display mode
    viewport: Option<WpViewport>,
    surface: Option<WlSurface>,
    buffer: Option<WlBuffer>,
    alternate: Option<WlBuffer>,
    phase: bool,
    logical: Option<(i32, i32)>,
    drawn: bool,
    layer: Option<ZwlrLayerSurfaceV1>,
    xdg: Option<(XdgSurface, XdgToplevel)>,
    /// A revision received before the first frame, acknowledged once it is shown.
    unshown: Option<u64>,
    exit: bool,
}

impl State {
    fn new() -> Self {
        State {
            outputs: Vec::new(),
            outputs_done: false,
            viewport: None,
            surface: None,
            buffer: None,
            alternate: None,
            phase: false,
            logical: None,
            drawn: false,
            layer: None,
            xdg: None,
            unshown: None,
            exit: false,
        }
    }

    /// Attach the buffer once we know the logical size from configure.
    fn commit_content(&mut self) {
        if self.drawn {
            // still re-assert the destination on later configures
            if let (Some(viewport), Some((w, h))) = (&self.viewport, self.logical) {
                viewport.set_destination(w, h);
            }
            if let Some(surface) = &self.surface {
                surface.commit();
            }
            return;
        }

        let (Some(surface), Some(buffer), Some((w, h))) =
            (&self.surface, &self.buffer, self.logical)
        else {
            return;
        };

        if let Some(viewport) = &self.viewport {
            viewport.set_destination(w, h);
        }
        surface.attach(
            Some(if self.phase {
                self.alternate.as_ref().unwrap_or(buffer)
            } else {
                buffer
            }),
            0,
            0,
        );
        surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        surface.commit();
        self.drawn = true;
        println!("READY {}", self.first_revision());
        let _ = std::io::stdout().flush();
    }

    /// What the first frame acknowledges: the settings it was drawn with.
    fn first_revision(&mut self) -> u64 {
        self.unshown.take().unwrap_or(0)
    }

    /// The revision to acknowledge now, or `None` when nothing is on screen yet: an
    /// owner told READY before the surface is mapped would take the pattern as shown.
    fn acknowledge(&mut self, revision: u64) -> Option<u64> {
        if self.drawn {
            Some(revision)
        } else {
            self.unshown = Some(revision);
            None
        }
    }
}

pub fn run(args: impl Iterator<Item = String>) -> i32 {
    match main_inner(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lbm-pattern: {e}");
            1
        }
    }
}

fn main_inner(mut args: impl Iterator<Item = String>) -> Result<i32, Box<dyn std::error::Error>> {
    let mut list = false;
    let mut output_name = None;
    let mut png_path = None;

    let mut pattern = None;
    let mut settings: Option<crate::Settings> = None;
    let mut behind = false;
    let mut with_editor = false;
    let mut owned = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => list = true,
            "--behind" => behind = true,
            "--with-editor" => with_editor = true,
            "--settings" => {
                let value: crate::Settings =
                    serde_json::from_str(&args.next().ok_or("missing settings")?)?;
                pattern = Some(value.pattern);
                settings = Some(value);
            }
            "--owned" => owned = true,
            "--pattern" => {
                pattern = Some(
                    crate::Pattern::from_id(&args.next().ok_or("missing pattern")?)
                        .ok_or("unknown pattern")?,
                )
            }
            "--output" => output_name = args.next(),
            "--png" => png_path = args.next(),
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    let (updates_tx, updates) = std::sync::mpsc::channel();
    if owned {
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                let Ok(update) = serde_json::from_str::<crate::Update>(&line) else {
                    break;
                };
                if updates_tx.send(update).is_err() {
                    break;
                }
            }
            std::process::exit(0);
        });
    }
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();

    let mut state = State::new();

    // wl_output v4 delivers the connector name; bind them all.
    for global in globals.contents().clone_list() {
        if global.interface == "wl_output" {
            let output: WlOutput =
                globals
                    .registry()
                    .bind(global.name, 4.min(global.version), &qh, ());
            state.outputs.push((output, OutputInfo::default()));
        }
    }

    // one roundtrip fills names and modes ("done" events)
    queue.roundtrip(&mut state)?;

    if list {
        for (_, info) in &state.outputs {
            if let (Some(name), Some((w, h))) = (&info.name, info.current_mode) {
                println!("{name} {w} {h}");
            }
        }
        return Ok(0);
    }

    let output_name = output_name.ok_or("missing --output NAME (or --list)")?;
    if pattern.is_some() == png_path.is_some() {
        return Err("choose --pattern NAME or --png FILE".into());
    }

    let (output, native) = state
        .outputs
        .iter()
        .find(|(_, info)| info.name.as_deref() == Some(output_name.as_str()))
        .and_then(|(o, info)| {
            info.current_mode
                .map(|(w, h)| (o.clone(), if info.rotated { (h, w) } else { (w, h) }))
        })
        .ok_or_else(|| format!("output not found: {output_name}"))?;

    let mut settings = settings.unwrap_or_else(|| crate::Settings {
        pattern: pattern.unwrap_or(crate::Pattern::Contrast),
        ..Default::default()
    });
    // The PNG call is the C# frontend's: a fullscreen window, as it always was, which
    // only needs xdg-shell. Patterns are OSDs, and need layer-shell.
    let window = png_path.is_some();
    let (width, height, pixels) = if pattern.is_some() {
        let mut pixels =
            crate::render_settings(&settings, native.0 as usize, native.1 as usize, false)?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        (native.0, native.1, pixels)
    } else {
        load_png_xrgb(&png_path.unwrap())?
    };

    let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
    let shm: WlShm = globals.bind(&qh, 1..=1, ())?;

    let viewporter: Option<WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
    let seat: Result<WlSeat, _> = globals.bind(&qh, 5..=9, ());

    if pattern.is_some() && viewporter.is_none() {
        return Err("Native-resolution patterns require the Wayland viewporter protocol".into());
    }
    if viewporter.is_none() {
        eprintln!("lbm-pattern: wp_viewporter not available, output will be rescaled");
    }
    // seat is optional: without it the viewer just relies on SIGTERM
    drop(seat);

    let buffer = create_buffer(&shm, &qh, width, height, &pixels)?;
    if settings.alternates() && pattern.is_some() {
        state.alternate = Some(pattern_buffer(&shm, &qh, native, &settings, true)?);
        state.phase = blink_phase();
    }
    let surface = compositor.create_surface(&qh, ());
    let viewport = viewporter.map(|v| v.get_viewport(&surface, &qh, ()));

    state.surface = Some(surface.clone());
    state.buffer = Some(buffer);
    state.viewport = viewport;
    if window {
        let wm_base: XdgWmBase = globals.bind(&qh, 1..=6, ())?;
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        toplevel.set_app_id("io.mgth.littlebigmouse.pattern".into());
        toplevel.set_title("LittleBigMouse test pattern".into());
        toplevel.set_fullscreen(Some(&output));
        state.xdg = Some((xdg_surface, toplevel));
        surface.commit();
        while !state.exit {
            queue.blocking_dispatch(&mut state)?;
        }
        return Ok(0);
    }
    state.logical = Some(native);
    // KWin recognizes this namespace as an OSD and skips window animations.
    // Both presentations use it: only stacking and keyboard focus differ.
    let shell: ZwlrLayerShellV1 = globals.bind(&qh, 1..=4, ()).map_err(|_| {
        "Test patterns need a compositor with the wlr layer-shell protocol (KDE Plasma, Sway, Hyprland…)"
    })?;
    let layer = shell.get_layer_surface(
        &surface,
        Some(&output),
        if behind {
            Layer::Bottom
        } else if with_editor {
            Layer::Top
        } else {
            Layer::Overlay
        },
        "on-screen-display".into(),
        &qh,
        (),
    );
    layer.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
    layer.set_size(0, 0);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(if behind || with_editor {
        KeyboardInteractivity::None
    } else {
        KeyboardInteractivity::Exclusive
    });
    state.layer = Some(layer);
    surface.commit();

    while !state.exit {
        queue.dispatch_pending(&mut state)?;
        // Keep only the newest slider position if rendering took longer than an edit.
        if let Some(update) = updates.try_iter().last() {
            let buffer = pattern_buffer(&shm, &qh, native, &update.settings, false)?;
            let alternate = if update.settings.alternates() {
                Some(pattern_buffer(&shm, &qh, native, &update.settings, true)?)
            } else {
                None
            };
            if let Some(old) = state.buffer.replace(buffer) {
                old.destroy();
            }
            if let Some(old) = state.alternate.take() {
                old.destroy();
            }
            state.alternate = alternate;
            settings = update.settings;
            state.phase = settings.alternates() && blink_phase();
            present(&state);
            if let Some(revision) = state.acknowledge(update.revision) {
                println!("READY {revision}");
                let _ = std::io::stdout().flush();
            }
        }
        let phase = settings.alternates() && blink_phase();
        if state.drawn && state.phase != phase {
            state.phase = phase;
            present(&state);
        }
        conn.flush()?;
        if state.exit {
            break;
        }
        if let Some(guard) = queue.prepare_read() {
            let mut fd = libc::pollfd {
                fd: conn.as_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // The short timeout also wakes for edits from the owner pipe. Alternation
            // follows wall-clock phase, so all matching screens use the same cadence.
            let result = unsafe { libc::poll(&mut fd, 1, 40) };
            if result > 0 {
                guard.read()?;
            } else if result < 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
    }

    Ok(0)
}

fn blink_phase() -> bool {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 700
        % 2
        == 1
}

fn present(state: &State) {
    if !state.drawn {
        return;
    }
    if let (Some(surface), Some(buffer)) = (&state.surface, &state.buffer) {
        surface.attach(
            Some(if state.phase {
                state.alternate.as_ref().unwrap_or(buffer)
            } else {
                buffer
            }),
            0,
            0,
        );
        surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        surface.commit();
    }
}

fn pattern_buffer(
    shm: &WlShm,
    qh: &QueueHandle<State>,
    native: (i32, i32),
    settings: &crate::Settings,
    phase: bool,
) -> Result<WlBuffer, Box<dyn std::error::Error>> {
    let mut pixels = crate::render_settings(settings, native.0 as usize, native.1 as usize, phase)?;
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    create_buffer(shm, qh, native.0, native.1, &pixels)
}

fn create_buffer(
    shm: &WlShm,
    qh: &QueueHandle<State>,
    width: i32,
    height: i32,
    pixels: &[u8],
) -> Result<WlBuffer, Box<dyn std::error::Error>> {
    let fd = memfd(pixels.len())?;
    File::from(fd.try_clone()?).write_all(pixels)?;
    let pool: WlShmPool = shm.create_pool(fd.as_fd(), pixels.len() as i32, qh, ());
    let buffer = pool.create_buffer(0, width, height, width * 4, Format::Xrgb8888, qh, ());
    pool.destroy();
    Ok(buffer)
}

fn memfd(size: usize) -> std::io::Result<OwnedFd> {
    let fd = unsafe { libc::memfd_create(c"lbm-pattern".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let rc = unsafe { libc::ftruncate(fd.as_fd().as_raw_fd(), size as libc::off_t) };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}

use std::os::fd::AsRawFd;

/// Decode a PNG to XRGB8888 little-endian bytes (B,G,R,X).
fn load_png_xrgb(path: &str) -> Result<(i32, i32, Vec<u8>), Box<dyn std::error::Error>> {
    let mut decoder = png::Decoder::new(File::open(path)?);
    // expand palette/1-2-4-bit to 8-bit channels, strip 16-bit down to 8
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;

    let (w, h) = (info.width as usize, info.height as usize);
    let mut out = vec![0u8; w * h * 4];

    match info.color_type {
        png::ColorType::Rgba => {
            for (dst, src) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(buf[..w * h * 4].as_chunks::<4>().0)
            {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = 0xFF;
            }
        }
        png::ColorType::Rgb => {
            for (dst, src) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(buf[..w * h * 3].as_chunks::<3>().0)
            {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = 0xFF;
            }
        }
        png::ColorType::Grayscale => {
            for (dst, src) in out.as_chunks_mut::<4>().0.iter_mut().zip(&buf[..w * h]) {
                dst[0] = *src;
                dst[1] = *src;
                dst[2] = *src;
                dst[3] = 0xFF;
            }
        }
        other => return Err(format!("unsupported PNG color type: {other:?}").into()),
    }

    Ok((w as i32, h as i32, out))
}

// ---- event handling --------------------------------------------------

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_, info)) = state.outputs.iter_mut().find(|(o, _)| o == output) else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => info.name = Some(name),
            wl_output::Event::Geometry {
                transform: WEnum::Value(transform),
                ..
            } => {
                info.rotated = matches!(
                    transform,
                    wl_output::Transform::_90
                        | wl_output::Transform::_270
                        | wl_output::Transform::Flipped90
                        | wl_output::Transform::Flipped270
                );
            }
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                ..
            } => {
                if flags.contains(wl_output::Mode::Current) {
                    info.current_mode = Some((width, height));
                }
            }
            wl_output::Event::Done => state.outputs_done = true,
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
            if caps.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
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
        match event {
            wl_pointer::Event::Button {
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                state.exit = true;
            }
            wl_pointer::Event::Enter { serial, .. } => {
                // hide the cursor over the pattern
                pointer.set_cursor(serial, None, 0, 0);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key {
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
            ..
        } = event
        {
            state.exit = true;
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WpViewporter);
delegate_noop!(State: ignore WpViewport);

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                if width > 0 && height > 0 {
                    state.logical = Some((width as i32, height as i32));
                }
                state.commit_content();
            }
            zwlr_layer_surface_v1::Event::Closed => state.exit = true,
            _ => {}
        }
    }
}
delegate_noop!(State: ignore ZwlrLayerShellV1);

impl Dispatch<XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, ()> for State {
    fn event(
        state: &mut Self,
        xdg_surface: &XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.commit_content();
        }
    }
}

impl Dispatch<XdgToplevel, ()> for State {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0 && height > 0 {
                    state.logical = Some((width, height));
                }
            }
            xdg_toplevel::Event::Close => state.exit = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_revision_received_before_the_first_frame_is_acknowledged_by_that_frame() {
        let mut state = State::new();
        assert_eq!(state.acknowledge(1), None, "nothing is on screen yet");
        assert_eq!(state.first_revision(), 1);
        state.drawn = true;
        assert_eq!(state.acknowledge(2), Some(2));
        assert_eq!(State::new().first_revision(), 0);
    }
}
