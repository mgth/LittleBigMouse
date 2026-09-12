//! The X11 back.
//!
//! X11 asks nothing and allows everything a ruler needs, so the work is not in getting
//! permission but in getting three things right at once:
//!
//! * a **32-bit visual** with its own colormap, or there is no per-pixel alpha and the
//!   band is a grey box over the screen rather than something you can see through;
//! * **override-redirect**, which takes the window out of the window manager's hands
//!   entirely: nobody moves it, nobody resizes it, no taskbar entry, and it is placed
//!   exactly where it is put;
//! * an empty **input shape** (the Shape extension), which is how a window says the
//!   pointer should go past it to whatever is underneath.
//!
//! And then, unlike Wayland, the server can be asked back. `GetGeometry`,
//! `GetWindowAttributes` and `ShapeGetRectangles` report what the server actually holds,
//! not what we hoped it would — so this spike checks its own claims instead of making
//! them.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{self, ConnectionExt as _, SK};
use x11rb::protocol::xproto::{
    self, ColormapAlloc, ConnectionExt as _, CreateWindowAux, EventMask, ImageFormat, PropMode,
    Rectangle, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

use crate::{AT, BAND};

pub fn run(seconds: u64, wanted_output: Option<String>) -> i32 {
    let (connection, screen_number) = match x11rb::connect(None) {
        Ok(it) => it,
        Err(error) => {
            eprintln!("no X display ({error}): this back is about X11");
            return 2;
        }
    };
    let screen = &connection.setup().roots[screen_number];
    let root = screen.root;

    // Where the chosen output sits in the one coordinate space X11 has. Under X11 a
    // "screen" is a region of the root window, so choosing one is choosing an offset —
    // there is nothing to attach the window to, unlike a Wayland output.
    let (origin, on) = match output_origin(&connection, root, wanted_output.as_deref()) {
        Ok(it) => it,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    let at = (origin.0 + AT.0, origin.1 + AT.1);

    // A visual with an alpha channel, or the transparency is a lie.
    let Some((depth, visual)) = argb_visual(screen) else {
        eprintln!("no 32-bit visual: this server cannot show a see-through ruler");
        return 1;
    };

    let colormap = match connection.generate_id() {
        Ok(id) => id,
        Err(error) => {
            eprintln!("out of X ids: {error}");
            return 2;
        }
    };
    if connection
        .create_colormap(ColormapAlloc::NONE, colormap, root, visual)
        .is_err()
    {
        eprintln!("the server refused a colormap for the 32-bit visual");
        return 1;
    }

    let window = match connection.generate_id() {
        Ok(id) => id,
        Err(error) => {
            eprintln!("out of X ids: {error}");
            return 2;
        }
    };
    let attributes = CreateWindowAux::new()
        // Out of the window manager's hands: placed exactly, never reparented, never in
        // a taskbar. For a ruler that is not a workaround, it is the right relationship.
        .override_redirect(1)
        .background_pixel(0)
        .border_pixel(0)
        .colormap(colormap)
        .event_mask(EventMask::EXPOSURE);
    if connection
        .create_window(
            depth,
            window,
            root,
            at.0 as i16,
            at.1 as i16,
            BAND.0 as u16,
            BAND.1 as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &attributes,
        )
        .is_err()
    {
        eprintln!("the server refused the window");
        return 1;
    }

    // The pointer goes through. An empty input shape is the whole of it.
    if connection
        .shape_rectangles(
            shape::SO::SET,
            SK::INPUT,
            xproto::ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[] as &[Rectangle],
        )
        .is_err()
    {
        eprintln!("the server refused the empty input shape: no click-through here");
        return 1;
    }

    let _ = name_it(&connection, window);
    if connection.map_window(window).is_err() || connection.flush().is_err() {
        eprintln!("the window would not map");
        return 1;
    }
    let _ = draw_band(&connection, window, depth);
    let _ = connection.flush();

    // What the server actually holds, rather than what was asked for. This is the part
    // Wayland cannot do: there, a surface's state is write-only.
    let geometry = connection
        .get_geometry(window)
        .ok()
        .and_then(|c| c.reply().ok());
    let attributes = connection
        .get_window_attributes(window)
        .ok()
        .and_then(|c| c.reply().ok());
    let input_shape = connection
        .shape_get_rectangles(window, SK::INPUT)
        .ok()
        .and_then(|c| c.reply().ok());

    println!(
        "asked for {}x{} at ({},{}) of {on} — which is ({},{}) of the root",
        BAND.0, BAND.1, AT.0, AT.1, at.0, at.1
    );
    let mut wrong = false;
    match geometry {
        Some(g) => {
            println!(
                "server holds {}x{} at ({},{}), depth {}",
                g.width, g.height, g.x, g.y, g.depth
            );
            if (g.x as i32, g.y as i32) != at || (g.width as i32, g.height as i32) != BAND {
                println!("MISMATCH: the server placed it somewhere else");
                wrong = true;
            }
            if g.depth != 32 {
                println!("MISMATCH: depth {} has no alpha channel", g.depth);
                wrong = true;
            }
        }
        None => {
            println!("the server would not say where the window is");
            wrong = true;
        }
    }
    match attributes {
        Some(a) if a.override_redirect => println!("override-redirect: granted"),
        Some(_) => {
            println!("MISMATCH: not override-redirect — a window manager may move it");
            wrong = true;
        }
        None => {
            println!("the server would not say whether it is override-redirect");
            wrong = true;
        }
    }
    match input_shape {
        Some(reply) if reply.rectangles.is_empty() => {
            println!("input shape: empty — the pointer goes through, and the server says so")
        }
        Some(reply) => {
            println!(
                "MISMATCH: the input shape has {} rectangle(s); clicks would be caught",
                reply.rectangles.len()
            );
            wrong = true;
        }
        None => {
            println!("the server would not report the input shape");
            wrong = true;
        }
    }

    println!("on screen for {seconds}s");
    let until = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < until {
        // Polled rather than blocked on, for the reason the Wayland back learned:
        // nothing happens to a quiet overlay, so waiting for an event is waiting
        // forever, and a band stuck on someone's screen is not a spike.
        while let Ok(Some(_)) = connection.poll_for_event() {}
        std::thread::sleep(Duration::from_millis(30));
    }

    let _ = connection.destroy_window(window);
    let _ = connection.flush();
    i32::from(wrong)
}

/// Where a named output starts, in root coordinates. Without a name, the primary — or
/// failing that the root's own corner.
fn output_origin(
    connection: &impl Connection,
    root: xproto::Window,
    wanted: Option<&str>,
) -> Result<((i32, i32), String), String> {
    let resources = connection
        .randr_get_screen_resources_current(root)
        .map_err(|e| format!("no RandR: {e}"))?
        .reply()
        .map_err(|e| format!("no RandR: {e}"))?;

    let mut names = Vec::new();
    for output in resources.outputs {
        let Ok(Ok(info)) = connection
            .randr_get_output_info(output, resources.config_timestamp)
            .map(|c| c.reply())
        else {
            continue;
        };
        if info.crtc == 0 {
            continue;
        }
        let name = String::from_utf8_lossy(&info.name).into_owned();
        let Ok(Ok(crtc)) = connection
            .randr_get_crtc_info(info.crtc, resources.config_timestamp)
            .map(|c| c.reply())
        else {
            continue;
        };
        names.push((name, (crtc.x as i32, crtc.y as i32)));
    }
    if names.is_empty() {
        return Err("RandR reports no connected output".to_owned());
    }
    println!(
        "outputs: {}",
        names
            .iter()
            .map(|(n, (x, y))| format!("{n} at ({x},{y})"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    match wanted {
        Some(wanted) => names
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(name, origin)| (*origin, name.clone()))
            .ok_or_else(|| format!("no such output: {wanted}")),
        None => {
            let (name, origin) = &names[0];
            Ok((*origin, name.clone()))
        }
    }
}

/// A depth-32 visual of the screen, if it has one.
fn argb_visual(screen: &xproto::Screen) -> Option<(u8, xproto::Visualid)> {
    screen
        .allowed_depths
        .iter()
        .find(|d| d.depth == 32)
        .and_then(|depth| {
            depth
                .visuals
                .first()
                .map(|visual| (depth.depth, visual.visual_id))
        })
}

/// Says what it is, for anyone looking at the window list. Cosmetic, and ignored for an
/// override-redirect window by most window managers — which is the point of it here.
fn name_it(
    connection: &impl Connection,
    window: xproto::Window,
) -> Result<(), Box<dyn std::error::Error>> {
    connection.change_property8(
        PropMode::REPLACE,
        window,
        xproto::AtomEnum::WM_NAME,
        xproto::AtomEnum::STRING,
        b"lbm-ruler-spike",
    )?;
    Ok(())
}

/// The same band the Wayland back draws: half transparent, with a tick every 20 px, so
/// a screenshot shows both the see-through and the pixel grid.
fn draw_band(
    connection: &impl Connection,
    window: xproto::Window,
    depth: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (BAND.0, BAND.1);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let at = ((y * width + x) * 4) as usize;
            let tick = x % 20 == 0;
            let edge = y == 0 || y == height - 1 || x == 0 || x == width - 1;
            // Premultiplied BGRA, as an ARGB visual expects it.
            let (a, v) = if edge || (tick && y < height / 3) {
                (0xFFu8, 0xFFu8)
            } else {
                (0x80, 0x80)
            };
            pixels[at] = v;
            pixels[at + 1] = v;
            pixels[at + 2] = v;
            pixels[at + 3] = a;
        }
    }
    let gc = connection.generate_id()?;
    connection.create_gc(gc, window, &xproto::CreateGCAux::new())?;
    connection.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        gc,
        width as u16,
        height as u16,
        0,
        0,
        0,
        depth,
        &pixels,
    )?;
    connection.free_gc(gc)?;
    Ok(())
}
