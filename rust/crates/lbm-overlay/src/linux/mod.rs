//! Linux surfaces live in an owned subprocess: a blocked compositor cannot block egui.
mod process;
mod wayland;
mod x11;
use crate::{
    scene::{Band, Content, Pixels, Side},
    Backend,
};
pub use process::{Native, Waker};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Read},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_MESSAGE: usize = 262_144;
const LEASE: Duration = Duration::from_secs(2);
#[derive(Debug, Serialize, Deserialize)]
enum Command {
    Scene(Vec<Band>),
    Pulse,
}
#[derive(Debug, Serialize, Deserialize)]
enum Reply {
    Ready,
    Input(crate::input::Input),
    Error(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Server {
    Wayland,
    X11,
}
pub fn server(session: Option<&str>, wayland: bool) -> Server {
    if session == Some("x11") {
        Server::X11
    } else if session == Some("wayland") || wayland {
        Server::Wayland
    } else {
        Server::X11
    }
}
fn selected_server() -> Server {
    server(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").is_some(),
    )
}

pub fn validate(bands: &[Band]) -> Result<(), String> {
    if bands.is_empty() || bands.len() > 80 {
        return Err("Invalid number of overlay bands.".into());
    }
    for b in bands {
        let s = b.screen;
        let r = b.rect;
        for p in [s, r] {
            Pixels::checked(lbm_layout::geo::Rect::new(
                p.x as f64,
                p.y as f64,
                p.width as f64,
                p.height as f64,
            ))?;
        }
        if r.x < s.x
            || r.y < s.y
            || r.x + r.width > s.x + s.width
            || r.y + r.height > s.y + s.height
            || !b.scale.is_finite()
            || !(0.5..=8.).contains(&b.scale)
            || !b.axis_mm.is_finite()
            || !(1.0..=10000.).contains(&b.axis_mm)
            || !b.cross_pixels_per_mm.is_finite()
            || b.cross_pixels_per_mm <= 0.
            || b.connector.is_empty()
            || b.connector.len() > 1024
            || b.monitor.len() > 1024
        {
            return Err("Invalid overlay geometry or output identity.".into());
        }
        match &b.content {
            Content::Guide { at, .. }
                if !at.is_finite()
                    || !(0.0..=1.0).contains(at)
                    || !matches!(b.side, Side::Top | Side::Left) =>
            {
                return Err("Invalid reference line geometry.".into())
            }
            Content::Ruler {
                zero_mm,
                measured_mm,
            } if !zero_mm.is_finite()
                || zero_mm.abs() > 2_000_000.
                || !measured_mm.is_finite()
                || !(1.0..=10000.).contains(measured_mm) =>
            {
                return Err("Invalid ruler dimensions.".into())
            }
            Content::Resistance(s)
                if s.len() > 16
                    || s.iter().any(|s| {
                        !s.from().is_finite() || !s.to().is_finite() || s.to() < s.from()
                    }) =>
            {
                return Err("Invalid resistance intervals.".into())
            }
            _ => {}
        }
    }
    Ok(())
}

/// Surface-local geometry is logical; the rendered buffer uses the preferred scale / 120.
/// The origin stays relative to the explicitly selected output, never to XWayland's root.
pub fn wayland_geometry(band: &Band, preferred: u32) -> Result<(Band, Band), String> {
    validate(std::slice::from_ref(band))?;
    let factor = preferred as f64 / 120.;
    if !(0.5..=8.).contains(&factor) || (factor - band.scale).abs() > 1. / 120. {
        return Err(
            "The output scale changed; reload the displays before showing overlays.".into(),
        );
    }
    let mut logical = band.clone();
    let screen = band.screen;
    let depth = if matches!(band.content, Content::Ruler { .. }) {
        100
    } else {
        40
    };
    let depth = depth.min(screen.width / 3).min(screen.height / 3).max(1);
    logical.rect = if let Content::Guide { at, .. } = band.content {
        crate::scene::guide_rect(screen, band.side, 2, at)
    } else {
        match band.side {
            Side::Top => Pixels {
                height: depth,
                ..screen
            },
            Side::Bottom => Pixels {
                y: screen.y + screen.height - depth,
                height: depth,
                ..screen
            },
            Side::Left => Pixels {
                width: depth,
                ..screen
            },
            Side::Right => Pixels {
                x: screen.x + screen.width - depth,
                width: depth,
                ..screen
            },
        }
    };
    let mut buffer = logical.clone();
    buffer.rect = Pixels {
        x: 0,
        y: 0,
        width: (logical.rect.width as f64 * factor).round() as i32,
        height: (logical.rect.height as f64 * factor).round() as i32,
    };
    buffer.cross_pixels_per_mm *= factor;
    buffer.scale = factor;
    Ok((logical, buffer))
}

#[derive(Default)]
struct Mail {
    latest: Option<Vec<Band>>,
    lease: Option<Instant>,
    progress: Option<Instant>,
}
fn report(reply: Reply) {
    println!("{}", serde_json::to_string(&reply).unwrap());
}
fn fatal(error: String) -> ! {
    report(Reply::Error(error));
    std::process::exit(1)
}

/// Internal lbm-app entry point. EOF and the lease watchdog close the display connection
/// even if a native roundtrip is wedged; process exit releases every server-side surface.
pub fn run_host() -> i32 {
    let mail = Arc::new(Mutex::new(Mail {
        lease: Some(Instant::now()),
        progress: Some(Instant::now()),
        ..Default::default()
    }));
    let read = mail.clone();
    std::thread::spawn(move || {
        let mut input = std::io::BufReader::new(std::io::stdin());
        loop {
            let mut bytes = Vec::new();
            let n = (&mut input)
                .take((MAX_MESSAGE + 1) as u64)
                .read_until(b'\n', &mut bytes)
                .unwrap_or(0);
            if n == 0 {
                std::process::exit(0);
            }
            if n > MAX_MESSAGE || !bytes.ends_with(b"\n") {
                fatal("Overlay message exceeds its size limit.".into());
            }
            let command: Command = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| fatal(format!("Invalid overlay message: {e}")));
            let mut m = read.lock().unwrap();
            m.lease = Some(Instant::now());
            if let Command::Scene(bands) = command {
                validate(&bands).unwrap_or_else(|e| fatal(e));
                m.latest = Some(bands);
            }
        }
    });
    let watch = mail.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(50));
        let m = watch.lock().unwrap();
        if m.lease.unwrap().elapsed() > LEASE {
            fatal("The editor stopped refreshing its overlays.".into());
        }
        if m.progress.unwrap().elapsed() > LEASE {
            fatal("The display server stopped answering overlay requests.".into());
        }
    });
    let result = (|| -> Result<(), String> {
        let mut backend: Box<dyn Backend> = match selected_server() {
            Server::Wayland => Box::new(wayland::Host::connect()?),
            Server::X11 => Box::new(x11::Host::connect()?),
        };
        loop {
            let latest = mail.lock().unwrap().latest.take();
            backend.poll()?;
            if let Some(bands) = latest {
                backend.present(&bands)?;
                report(Reply::Ready);
            }
            for input in backend.take_input() {
                report(Reply::Input(input));
            }
            mail.lock().unwrap().progress = Some(Instant::now());
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    if let Err(error) = result {
        report(Reply::Error(error));
    }
    1
}
