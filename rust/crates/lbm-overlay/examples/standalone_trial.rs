//! Manual trial of `Standalone` (not committed): shows passive rulers on one output.
//! `standalone_trial CONNECTOR X Y W H SCALE SECONDS [SECONDS_UPDATE]`
use lbm_overlay::scene::{Band, Content, Pixels, Side};
fn main() {
    #[cfg(target_os = "linux")]
    if std::env::args().nth(1).as_deref() == Some("--screen-overlay-host") {
        std::process::exit(lbm_overlay::linux::run_host());
    }
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| a[i].parse::<f64>().unwrap();
    let screen = Pixels {
        x: n(1) as i32,
        y: n(2) as i32,
        width: n(3) as i32,
        height: n(4) as i32,
    };
    let scale = n(5);
    let bands = |zero: f64| -> Vec<Band> {
        Side::ALL
            .iter()
            .map(|&side| {
                let t = (100. * scale).round() as i32;
                let rect = match side {
                    Side::Top => Pixels {
                        height: t,
                        ..screen
                    },
                    Side::Bottom => Pixels {
                        y: screen.y + screen.height - t,
                        height: t,
                        ..screen
                    },
                    Side::Left => Pixels { width: t, ..screen },
                    Side::Right => Pixels {
                        x: screen.x + screen.width - t,
                        width: t,
                        ..screen
                    },
                };
                let (axis, cross) = if side.vertical() {
                    (300., screen.width as f64 / 520.)
                } else {
                    (520., screen.height as f64 / 300.)
                };
                Band {
                    monitor: "m".into(),
                    connector: a[0].clone(),
                    screen,
                    rect,
                    side,
                    scale,
                    axis_mm: axis,
                    cross_pixels_per_mm: cross,
                    content: Content::Ruler {
                        zero_mm: zero,
                        measured_mm: 400.,
                    },
                    selected: None,
                    passive: false,
                }
            })
            .collect()
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let shown = lbm_overlay::Standalone::show(
        bands(0.),
        Box::new(move |e| {
            let _ = tx.send(e);
        }),
    )
    .unwrap();
    eprintln!("shown");
    if let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs_f64(n(6))) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    shown.update(bands(50.));
    eprintln!("updated");
    if let Ok(e) = rx.recv_timeout(std::time::Duration::from_secs_f64(
        a.get(7).map(|s| s.parse().unwrap()).unwrap_or(1.),
    )) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    drop(shown);
    std::thread::sleep(std::time::Duration::from_millis(300));
    eprintln!("dropped");
}
