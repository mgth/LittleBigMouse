//! The Wayland back: `zwlr_layer_shell_v1`, the one protocol that lets a client say
//! where it wants to be.
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_output::{self, WlOutput},
        wl_region::WlRegion,
        wl_registry::WlRegistry,
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use crate::{AT, BAND};

#[derive(Default)]
struct OutputInfo {
    name: Option<String>,
}

struct State {
    outputs: Vec<(WlOutput, OutputInfo)>,
    configured: Option<(u32, u32)>,
    closed: bool,
}

pub fn run(seconds: u64, wanted_output: Option<String>) -> i32 {
    let Ok(connection) = Connection::connect_to_env() else {
        eprintln!("no Wayland display: this spike is about Wayland");
        return 2;
    };
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&connection) else {
        eprintln!("the compositor did not answer the registry");
        return 2;
    };
    let handle = queue.handle();

    let compositor: WlCompositor = match globals.bind(&handle, 1..=6, ()) {
        Ok(it) => it,
        Err(error) => {
            eprintln!("no wl_compositor: {error}");
            return 2;
        }
    };
    let shm: WlShm = match globals.bind(&handle, 1..=2, ()) {
        Ok(it) => it,
        Err(error) => {
            eprintln!("no wl_shm: {error}");
            return 2;
        }
    };
    // The whole question of this spike. A compositor without it cannot be asked for
    // a placed surface at all, and the frontend would have to fall back.
    let layer_shell: ZwlrLayerShellV1 = match globals.bind(&handle, 1..=5, ()) {
        Ok(it) => it,
        Err(error) => {
            eprintln!("no zwlr_layer_shell_v1 ({error}): this compositor cannot place a ruler");
            return 1;
        }
    };

    let mut state = State {
        outputs: Vec::new(),
        configured: None,
        closed: false,
    };
    // Outputs first: the band belongs to one screen, and which one is the point.
    for global in globals.contents().clone_list() {
        if global.interface == "wl_output" {
            let output: WlOutput =
                globals
                    .registry()
                    .bind(global.name, global.version.min(4), &handle, ());
            state.outputs.push((output, OutputInfo::default()));
        }
    }
    if queue.roundtrip(&mut state).is_err() {
        eprintln!("the compositor stopped answering while listing outputs");
        return 2;
    }

    let names: Vec<String> = state
        .outputs
        .iter()
        .map(|(_, info)| info.name.clone().unwrap_or_else(|| "?".to_owned()))
        .collect();
    println!("outputs: {}", names.join(", "));

    let chosen = match &wanted_output {
        Some(wanted) => state
            .outputs
            .iter()
            .find(|(_, info)| info.name.as_deref() == Some(wanted.as_str())),
        None => state.outputs.first(),
    };
    let Some((output, info)) = chosen else {
        eprintln!("no such output: {wanted_output:?}");
        return 2;
    };
    let on = info.name.clone().unwrap_or_else(|| "?".to_owned());

    let surface = compositor.create_surface(&handle, ());
    let layer = layer_shell.get_layer_surface(
        &surface,
        Some(output),
        // Above everything, including full-screen windows: a ruler that a maximised
        // window can hide is a ruler that lies about where the edge is.
        Layer::Overlay,
        "lbm-ruler-spike".to_owned(),
        &handle,
        (),
    );
    // Anchoring to two adjacent edges turns the margins into a position. This is
    // the whole trick: Wayland has no "move my window here", but it has "hold me
    // this far from that corner".
    layer.set_anchor(Anchor::Top | Anchor::Left);
    layer.set_margin(AT.1, 0, 0, AT.0);
    layer.set_size(BAND.0 as u32, BAND.1 as u32);
    // Neither the keyboard nor the pointer: the ruler is something you look at
    // while you drag a screen underneath it.
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    let empty: WlRegion = compositor.create_region(&handle, ());
    surface.set_input_region(Some(&empty));
    surface.commit();

    if queue.roundtrip(&mut state).is_err() || state.closed {
        eprintln!("the compositor refused the layer surface");
        return 1;
    }
    let Some((width, height)) = state.configured else {
        eprintln!("the compositor never configured the layer surface");
        return 1;
    };
    println!(
        "asked for {}x{} at ({},{}) of {on}; configured {width}x{height}",
        BAND.0, BAND.1, AT.0, AT.1
    );
    if (width, height) != (BAND.0 as u32, BAND.1 as u32) {
        println!("NOTE: the compositor chose a different size");
    }

    let Some(buffer) = band_buffer(&shm, &handle, width as i32, height as i32) else {
        eprintln!("could not make a shared-memory buffer");
        return 2;
    };
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
    surface.commit();
    let _ = queue.roundtrip(&mut state);

    println!("on screen for {seconds}s");
    let until = Instant::now() + Duration::from_secs(seconds);
    // Polled, not blocked on: `blocking_dispatch` waits for an event, and a surface
    // nothing is happening to gets none — the deadline would only be noticed the
    // next time the compositor felt like saying something, which on a quiet desktop
    // is never. A spike that can leave a band stuck on someone's screen is not a
    // tool, it is a liability.
    while Instant::now() < until && !state.closed {
        if queue.dispatch_pending(&mut state).is_err() || connection.flush().is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    0
}

/// A half-transparent band with a tick every 20 px, so both the transparency and the
/// pixel grid are visible in a screenshot.
fn band_buffer(
    shm: &WlShm,
    handle: &QueueHandle<State>,
    width: i32,
    height: i32,
) -> Option<WlBuffer> {
    let stride = width * 4;
    let size = (stride * height) as usize;
    let fd = shared_memory(size)?;
    let mut pixels = vec![0u8; size];
    for y in 0..height {
        for x in 0..width {
            let at = ((y * stride) + x * 4) as usize;
            let tick = x % 20 == 0;
            let edge = y == 0 || y == height - 1 || x == 0 || x == width - 1;
            // Argb8888 is premultiplied: a half-transparent white is 0x80 everywhere.
            let (a, v) = if edge || (tick && y < height / 3) {
                (0xFF, 0xFF)
            } else {
                (0x80, 0x80)
            };
            pixels[at] = v;
            pixels[at + 1] = v;
            pixels[at + 2] = v;
            pixels[at + 3] = a;
        }
    }
    // SAFETY: the fd is ours, open for writing, and sized above.
    unsafe {
        let mapped = libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_fd().as_raw_fd(),
            0,
        );
        if mapped == libc::MAP_FAILED {
            return None;
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), mapped as *mut u8, size);
        libc::munmap(mapped, size);
    }
    let pool: WlShmPool = shm.create_pool(fd.as_fd(), size as i32, handle, ());
    let buffer = pool.create_buffer(0, width, height, stride, Format::Argb8888, handle, ());
    pool.destroy();
    Some(buffer)
}

fn shared_memory(size: usize) -> Option<OwnedFd> {
    let name = std::ffi::CString::new(format!("/lbm-overlay-{}", std::process::id())).ok()?;
    // SAFETY: a fresh name, unlinked immediately so nothing else can open it.
    unsafe {
        let fd = libc::shm_open(
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
            0o600,
        );
        if fd < 0 {
            return None;
        }
        libc::shm_unlink(name.as_ptr());
        if libc::ftruncate(fd, size as libc::off_t) < 0 {
            libc::close(fd);
            return None;
        }
        Some(OwnedFd::from_raw_fd(fd))
    }
}

use std::os::fd::AsRawFd;

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
        if let wl_output::Event::Name { name } = event {
            if let Some((_, info)) = state.outputs.iter_mut().find(|(o, _)| o == output) {
                info.name = Some(name);
            }
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
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                state.configured = Some((width, height));
            }
            zwlr_layer_surface_v1::Event::Closed => state.closed = true,
            _ => {}
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore ZwlrLayerShellV1);
