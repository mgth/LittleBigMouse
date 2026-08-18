//! lbm-pattern — native-Wayland fullscreen bitmap viewer.
//!
//! The VCP test patterns need pixel-perfect output (a 1-px checkerboard IS the
//! measurement). The Avalonia UI runs on XWayland, and KWin maps the whole X11
//! space at a single global factor: on any output whose scale differs, the
//! window buffer is rescaled and fine patterns are destroyed. This helper is a
//! bare Wayland client instead: it fullscreens an xdg_toplevel on the requested
//! output and attaches a wl_shm buffer at the output's native resolution, with
//! a wp_viewport mapping it onto the logical size — when the buffer matches the
//! panel, the compositor samples it 1:1.
//!
//! Usage:
//!   lbm-pattern --list                     outputs: "NAME WIDTH HEIGHT" per line
//!   lbm-pattern --output NAME --png FILE   display FILE fullscreen on NAME
//!
//! The viewer exits on pointer click, key press, toplevel close, or SIGTERM.

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("lbm-pattern is Linux/Wayland only");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    std::process::exit(linux::run());
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fs::File;
    use std::io::Write;
    use std::os::fd::{AsFd, FromRawFd, OwnedFd};

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
    }

    struct State {
        outputs: Vec<(WlOutput, OutputInfo)>,
        outputs_done: bool,

        // display mode
        xdg_surface: Option<XdgSurface>,
        viewport: Option<WpViewport>,
        surface: Option<WlSurface>,
        buffer: Option<WlBuffer>,
        logical: Option<(i32, i32)>,
        drawn: bool,
        exit: bool,
    }

    impl State {
        fn new() -> Self {
            State {
                outputs: Vec::new(),
                outputs_done: false,
                xdg_surface: None,
                viewport: None,
                surface: None,
                buffer: None,
                logical: None,
                drawn: false,
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
            surface.attach(Some(buffer), 0, 0);
            surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            surface.commit();
            self.drawn = true;
        }
    }

    pub fn run() -> i32 {
        match main_inner() {
            Ok(code) => code,
            Err(e) => {
                eprintln!("lbm-pattern: {e}");
                1
            }
        }
    }

    fn main_inner() -> Result<i32, Box<dyn std::error::Error>> {
        let mut list = false;
        let mut output_name = None;
        let mut png_path = None;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--list" => list = true,
                "--output" => output_name = args.next(),
                "--png" => png_path = args.next(),
                other => return Err(format!("unknown argument: {other}").into()),
            }
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
        let png_path = png_path.ok_or("missing --png FILE")?;

        let output = state
            .outputs
            .iter()
            .find(|(_, info)| info.name.as_deref() == Some(output_name.as_str()))
            .map(|(o, _)| o.clone())
            .ok_or_else(|| format!("output not found: {output_name}"))?;

        let (width, height, pixels) = load_png_xrgb(&png_path)?;

        let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let wm_base: XdgWmBase = globals.bind(&qh, 1..=6, ())?;
        let viewporter: Option<WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
        let seat: Result<WlSeat, _> = globals.bind(&qh, 5..=9, ());

        if viewporter.is_none() {
            eprintln!("lbm-pattern: wp_viewporter not available, output will be rescaled");
        }
        // seat is optional: without it the viewer just relies on SIGTERM
        drop(seat);

        // shm buffer at the bitmap's native size
        let stride = width * 4;
        let size = (stride * height) as usize;
        let fd = memfd(size)?;
        {
            let mut file = File::from(fd.try_clone()?);
            file.write_all(&pixels)?;
        }
        let pool: WlShmPool = shm.create_pool(fd.as_fd(), size as i32, &qh, ());
        let buffer: WlBuffer =
            pool.create_buffer(0, width, height, stride, Format::Xrgb8888, &qh, ());

        let surface = compositor.create_surface(&qh, ());
        let viewport = viewporter.map(|v| v.get_viewport(&surface, &qh, ()));

        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        toplevel.set_app_id("io.mgth.littlebigmouse.pattern".into());
        toplevel.set_title("LittleBigMouse test pattern".into());
        toplevel.set_fullscreen(Some(&output));
        surface.commit();

        state.surface = Some(surface);
        state.buffer = Some(buffer);
        state.viewport = viewport;
        state.xdg_surface = Some(xdg_surface);

        while !state.exit {
            queue.blocking_dispatch(&mut state)?;
        }

        Ok(0)
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
                    .chunks_exact_mut(4)
                    .zip(buf[..w * h * 4].chunks_exact(4))
                {
                    dst[0] = src[2];
                    dst[1] = src[1];
                    dst[2] = src[0];
                    dst[3] = 0xFF;
                }
            }
            png::ColorType::Rgb => {
                for (dst, src) in out
                    .chunks_exact_mut(4)
                    .zip(buf[..w * h * 3].chunks_exact(3))
                {
                    dst[0] = src[2];
                    dst[1] = src[1];
                    dst[2] = src[0];
                    dst[3] = 0xFF;
                }
            }
            png::ColorType::Grayscale => {
                for (dst, src) in out.chunks_exact_mut(4).zip(buf[..w * h].iter()) {
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
                wl_output::Event::Mode {
                    flags,
                    width,
                    height,
                    ..
                } => {
                    if let WEnum::Value(flags) = flags {
                        if flags.contains(wl_output::Mode::Current) {
                            info.current_mode = Some((width, height));
                        }
                    }
                }
                wl_output::Event::Done => state.outputs_done = true,
                _ => {}
            }
        }
    }

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
}
