//! What the desktop shows now, read without changing it, and drawn small.
//!
//! Two readers of the same thing: the window draws this machine's wallpaper behind its
//! screens on the map, and a remote's agent draws its own for its master's map
//! (docs/multi-pc.md, "Wallpapers"). Both ask the desktop the same question —
//! [`query`]: Plasma's live containments, or `IDesktopWallpaper` — and render an entry
//! the same way ([`render`]), so a remote screen on the master's map looks like a local
//! one on its own.
//!
//! [`query`] is the only part that reaches the session: read-only, but a test still has no
//! business asking the developer's desktop, so the agent puts it behind its `Desktop`
//! trait and the window behind its watcher.
use image::{imageops::FilterType, Rgba, RgbaImage};
use lbm_layout::geo::Rect;

use crate::{Size, Step, Style};

#[cfg(target_os = "linux")]
mod plasma;
#[cfg(windows)]
mod windows;

/// Which axes a tiled picture repeats on. On a single axis, Plasma stretches it to the
/// output along the other one. Windows only ever tiles on both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Both,
    Vertically,
    Horizontally,
}

/// One output of the desktop and what it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Where the output is, in the desktop's own coordinates (logical pixels on Plasma,
    /// physical ones for a per-monitor-aware process on Windows).
    pub bounds: Rect,
    /// The picture; `None` is a solid colour.
    pub path: Option<std::path::PathBuf>,
    pub style: Style,
    pub repeat: Repeat,
    /// The colour around a picture that does not cover the output, or the whole output.
    pub color: [u8; 4],
}

/// A screen to draw: where it is in the desktop's coordinates, and how many device pixels
/// one of those is.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub id: String,
    pub bounds: Rect,
    pub scale: f64,
}

/// What this session's desktop shows on each of its outputs.
pub fn query() -> Result<Vec<Entry>, String> {
    #[cfg(target_os = "linux")]
    return plasma::query();
    #[cfg(windows)]
    return windows::query();
    #[cfg(not(any(target_os = "linux", windows)))]
    Err("Desktop wallpaper reading is unavailable on this platform.".into())
}

/// Whether two rectangles are the same output: the desktop and the display detection may
/// round a scaled geometry differently.
pub fn same_output(a: Rect, b: Rect) -> bool {
    [
        (a.x(), b.x()),
        (a.y(), b.y()),
        (a.width(), b.width()),
        (a.height(), b.height()),
    ]
    .into_iter()
    .all(|(a, b)| (a - b).abs() < 2.)
}

/// The entry that shows on `target`, if the desktop said anything about it.
pub fn entry_for<'a>(target: &Target, entries: &'a [Entry]) -> Option<&'a Entry> {
    entries
        .iter()
        .find(|e| same_output(e.bounds, target.bounds))
}

/// `entry` drawn as the desktop draws it on `target`, no larger than `limit` pixels on
/// either side. `entries` is the whole desktop: a span is cut out of all of it.
pub fn render(
    entry: &Entry,
    target: &Target,
    entries: &[Entry],
    limit: f64,
) -> Result<RgbaImage, String> {
    let w = target.bounds.width() * target.scale;
    let h = target.bounds.height() * target.scale;
    if !w.is_finite() || !h.is_finite() || w < 1. || h < 1. || limit.is_nan() || limit < 1. {
        return Err("Invalid output size".into());
    }
    let ratio = (limit / w).min(limit / h).min(1.);
    let target_size = Size {
        w: (w * ratio).round().max(1.) as i32,
        h: (h * ratio).round().max(1.) as i32,
    };
    let Some(path) = &entry.path else {
        return Ok(RgbaImage::from_pixel(
            target_size.w as u32,
            target_size.h as u32,
            Rgba(entry.color),
        ));
    };
    let source = crate::render::decode(path)?;
    // Apply the same reduction to the source and target. Center and Tile keep
    // their desktop pixel scale instead of treating a thumbnail as a new image.
    let source = image::imageops::resize(
        &source,
        (f64::from(source.width()) * ratio).round().max(1.) as u32,
        (f64::from(source.height()) * ratio).round().max(1.) as u32,
        FilterType::CatmullRom,
    );
    let stretched = match entry.repeat {
        Repeat::Both => None,
        Repeat::Vertically => Some((target_size.w as u32, source.height())),
        Repeat::Horizontally => Some((source.width(), target_size.h as u32)),
    };
    let source = match stretched {
        Some((w, h)) => image::imageops::resize(&source, w, h, FilterType::CatmullRom),
        None => source,
    };
    let source_size = Size {
        w: source.width() as i32,
        h: source.height() as i32,
    };
    let steps = if entry.style == Style::Span || (entry.style == Style::Tile && cfg!(windows)) {
        let left = entries
            .iter()
            .map(|e| e.bounds.x())
            .fold(f64::INFINITY, f64::min);
        let top = entries
            .iter()
            .map(|e| e.bounds.y())
            .fold(f64::INFINITY, f64::min);
        let right = entries
            .iter()
            .map(|e| e.bounds.x() + e.bounds.width())
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = entries
            .iter()
            .map(|e| e.bounds.y() + e.bounds.height())
            .fold(f64::NEG_INFINITY, f64::max);
        let rect = |x: f64, y: f64, w: f64, h: f64| crate::Rect {
            x: (x * ratio).round() as i32,
            y: (y * ratio).round() as i32,
            w: (w * ratio).round().max(1.) as i32,
            h: (h * ratio).round().max(1.) as i32,
        };
        crate::across_the_desktop(
            entry.style,
            source_size,
            rect(target.bounds.x(), target.bounds.y(), w, h),
            rect(left, top, right - left, bottom - top),
        )
    } else if entry.style == Style::Tile {
        vec![
            Step::Tile { to: target_size },
            Step::Crop(crate::Rect {
                x: 0,
                y: 0,
                w: target_size.w,
                h: target_size.h,
            }),
        ]
    } else {
        crate::on_one_screen(entry.style, source_size, target_size)
    };
    for step in &steps {
        let size = match step {
            Step::Resize(s) | Step::Pad { to: s, .. } | Step::Tile { to: s } => Some(s),
            _ => None,
        };
        if size
            .is_some_and(|s| s.w <= 0 || s.h <= 0 || i64::from(s.w) * i64::from(s.h) > 16_777_216)
        {
            return Err("Oversized wallpaper preview".into());
        }
    }
    Ok(crate::apply(source, &steps, Rgba(entry.color)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_wallpaper_keeps_its_native_scale_when_reduced_to_a_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.png");
        RgbaImage::from_pixel(256, 256, Rgba([255, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let target = Target {
            id: "screen".into(),
            bounds: Rect::new(0., 0., 2048., 1024.),
            scale: 1.,
        };
        let entry = Entry {
            bounds: target.bounds,
            path: Some(path),
            style: Style::Center,
            repeat: Repeat::Both,
            color: [0, 0, 100, 255],
        };
        let out = render(&entry, &target, std::slice::from_ref(&entry), 512.).unwrap();
        assert_eq!(out.dimensions(), (512, 256));
        assert_eq!(out.get_pixel(200, 128).0, [0, 0, 100, 255]);
        assert_eq!(out.get_pixel(256, 128).0, [255, 0, 0, 255]);
    }

    #[test]
    fn a_picture_tiled_on_one_axis_is_stretched_along_the_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tile.png");
        let (red, blue) = (Rgba([255, 0, 0, 255]), Rgba([0, 0, 255, 255]));
        RgbaImage::from_fn(20, 10, |x, _| if x < 10 { red } else { blue })
            .save(&path)
            .unwrap();
        let target = Target {
            id: "screen".into(),
            bounds: Rect::new(0., 0., 100., 40.),
            scale: 1.,
        };
        let pixel = |repeat, x, y| {
            let entry = Entry {
                bounds: target.bounds,
                path: Some(path.clone()),
                style: Style::Tile,
                repeat,
                color: [0, 0, 0, 255],
            };
            let out = render(&entry, &target, std::slice::from_ref(&entry), 512.).unwrap();
            assert_eq!(out.dimensions(), (100, 40));
            *out.get_pixel(x, y)
        };
        // One copy across the width, so the left half stays red all the way down.
        assert_eq!(pixel(Repeat::Vertically, 35, 5), red);
        assert_eq!(pixel(Repeat::Vertically, 35, 35), red);
        assert_eq!(pixel(Repeat::Vertically, 65, 35), blue);
        // Repeated every 20 pixels across, each copy as tall as the screen.
        assert_eq!(pixel(Repeat::Horizontally, 25, 35), red);
        assert_eq!(pixel(Repeat::Horizontally, 35, 35), blue);
        assert_eq!(pixel(Repeat::Both, 35, 5), blue);
    }

    #[test]
    fn spanning_uses_desktop_coordinates_including_negative_origins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("panorama.png");
        RgbaImage::from_fn(400, 100, |x, _| {
            if x < 200 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 255, 0, 255])
            }
        })
        .save(&path)
        .unwrap();
        let targets = [
            Target {
                id: "left".into(),
                bounds: Rect::new(-200., 0., 200., 100.),
                scale: 1.,
            },
            Target {
                id: "right".into(),
                bounds: Rect::new(0., 0., 200., 100.),
                scale: 1.,
            },
        ];
        let entries: Vec<_> = targets
            .iter()
            .map(|t| Entry {
                bounds: t.bounds,
                path: Some(path.clone()),
                style: Style::Span,
                repeat: Repeat::Both,
                color: [0, 0, 0, 255],
            })
            .collect();
        let left = render(&entries[0], &targets[0], &entries, 512.).unwrap();
        let right = render(&entries[1], &targets[1], &entries, 512.).unwrap();
        assert_eq!(left.get_pixel(100, 50).0, [255, 0, 0, 255]);
        assert_eq!(right.get_pixel(100, 50).0, [0, 255, 0, 255]);
    }

    #[test]
    fn a_thumbnail_stays_within_its_limit_and_keeps_the_screens_shape() {
        let target = Target {
            id: "screen".into(),
            bounds: Rect::new(0., 0., 2560., 1440.),
            scale: 1.,
        };
        let entry = Entry {
            bounds: target.bounds,
            path: None,
            style: Style::Fill,
            repeat: Repeat::Both,
            color: [1, 2, 3, 255],
        };
        let out = render(&entry, &target, std::slice::from_ref(&entry), 384.).unwrap();
        assert_eq!(out.dimensions(), (384, 216));
        // The output geometry is matched with some slack, as detections round apart.
        assert!(entry_for(
            &Target {
                bounds: Rect::new(1., 0., 2559., 1441.),
                ..target.clone()
            },
            std::slice::from_ref(&entry)
        )
        .is_some());
        assert!(entry_for(
            &Target {
                bounds: Rect::new(1920., 0., 2560., 1440.),
                ..target
            },
            &[entry]
        )
        .is_none());
    }
}
