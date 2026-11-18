//! Read the desktop's actual wallpaper without changing it or the editable layout.
use image::{imageops::FilterType, Rgba, RgbaImage};
use lbm_layout::{geo::Rect, model::Layout};
use lbm_wallpaper::{Size, Step, Style};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
mod plasma;
#[cfg(windows)]
mod windows;

type Pictures = Vec<(String, RgbaImage)>;

#[derive(Clone, Debug, PartialEq)]
struct Target {
    id: String,
    bounds: Rect,
    scale: f64,
}
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    bounds: Rect,
    path: Option<PathBuf>,
    style: Style,
    color: [u8; 4],
}
struct Read {
    targets: Vec<Target>,
    signature: Option<String>,
    pictures: Option<Pictures>,
}

/// Only one background read at a time. Changes to the physical layout do not
/// invalidate the desktop image; changes to the OS output geometry do.
pub struct Watcher {
    next: Instant,
    pending: Option<mpsc::Receiver<Result<Read, String>>>,
    signature: Option<String>,
}
impl Default for Watcher {
    fn default() -> Self {
        Self {
            next: Instant::now(),
            pending: None,
            signature: None,
        }
    }
}
impl Watcher {
    pub fn poll(&mut self, layout: &Layout) -> Option<Pictures> {
        let targets: Vec<_> = layout
            .monitors()
            .iter()
            .filter_map(|m| {
                let s = &layout.source(m.active_source.as_deref()?)?.source;
                s.attached_to_desktop.then(|| Target {
                    id: m.id.clone(),
                    bounds: s.in_pixel.bounds(),
                    scale: if cfg!(windows) {
                        1.
                    } else {
                        s.effective_dpi.y / 96.
                    },
                })
            })
            .collect();
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    if let Ok(read) = result {
                        if read.targets == targets {
                            self.signature = read.signature;
                            return read.pictures;
                        }
                        self.signature = None;
                        self.next = Instant::now();
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
                Err(mpsc::TryRecvError::Empty) => return None,
            }
        }
        if targets.is_empty() || Instant::now() < self.next {
            return None;
        }
        self.next = Instant::now() + Duration::from_secs(3);
        let previous = self.signature.clone();
        let (send, receive) = mpsc::channel();
        self.pending = Some(receive);
        std::thread::spawn(move || {
            let result = query().map(|entries| read(targets, entries, previous.as_deref()));
            let _ = send.send(result);
        });
        None
    }
}
fn query() -> Result<Vec<Entry>, String> {
    #[cfg(target_os = "linux")]
    return plasma::query();
    #[cfg(windows)]
    return windows::query();
    #[cfg(not(any(target_os = "linux", windows)))]
    Err("Desktop wallpaper reading is unavailable on this platform.".into())
}
fn read(targets: Vec<Target>, entries: Vec<Entry>, previous: Option<&str>) -> Read {
    let files: Vec<_> = entries
        .iter()
        .map(|e| {
            e.path.as_ref().and_then(|p| {
                std::fs::metadata(p)
                    .ok()
                    .map(|m| (m.len(), m.modified().ok()))
            })
        })
        .collect();
    let signature = format!("{targets:?}{entries:?}{files:?}");
    if previous == Some(signature.as_str()) {
        return Read {
            targets,
            signature: Some(signature),
            pictures: None,
        };
    }
    let mut pictures = Vec::new();
    let mut complete = true;
    for target in &targets {
        // Both Plasma and the display detector use logical desktop coordinates.
        let entry = entries
            .iter()
            .find(|e| same_output(e.bounds, target.bounds));
        if let Some(entry) = entry {
            match render(entry, target, &entries) {
                Ok(pixels) => pictures.push((target.id.clone(), pixels)),
                Err(_) => complete = false, // Retry a temporarily missing/replaced file.
            }
        }
    }
    Read {
        targets,
        signature: complete.then_some(signature),
        pictures: Some(pictures),
    }
}
fn same_output(a: Rect, b: Rect) -> bool {
    [
        (a.x(), b.x()),
        (a.y(), b.y()),
        (a.width(), b.width()),
        (a.height(), b.height()),
    ]
    .into_iter()
    .all(|(a, b)| (a - b).abs() < 2.)
}
fn render(entry: &Entry, target: &Target, entries: &[Entry]) -> Result<RgbaImage, String> {
    let w = target.bounds.width() * target.scale;
    let h = target.bounds.height() * target.scale;
    if !w.is_finite() || !h.is_finite() || w < 1. || h < 1. {
        return Err("Invalid output size".into());
    }
    let ratio = (512. / w).min(512. / h).min(1.);
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
    let source = lbm_wallpaper::render::decode(path)?;
    // Apply the same reduction to the source and target. Center and Tile keep
    // their desktop pixel scale instead of treating a thumbnail as a new image.
    let source = image::imageops::resize(
        &source,
        (f64::from(source.width()) * ratio).round().max(1.) as u32,
        (f64::from(source.height()) * ratio).round().max(1.) as u32,
        FilterType::CatmullRom,
    );
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
        let rect = |x: f64, y: f64, w: f64, h: f64| lbm_wallpaper::Rect {
            x: (x * ratio).round() as i32,
            y: (y * ratio).round() as i32,
            w: (w * ratio).round().max(1.) as i32,
            h: (h * ratio).round().max(1.) as i32,
        };
        lbm_wallpaper::across_the_desktop(
            entry.style,
            source_size,
            rect(target.bounds.x(), target.bounds.y(), w, h),
            rect(left, top, right - left, bottom - top),
        )
    } else if entry.style == Style::Tile {
        vec![
            Step::Tile { to: target_size },
            Step::Crop(lbm_wallpaper::Rect {
                x: 0,
                y: 0,
                w: target_size.w,
                h: target_size.h,
            }),
        ]
    } else {
        lbm_wallpaper::on_one_screen(entry.style, source_size, target_size)
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
    Ok(lbm_wallpaper::apply(source, &steps, Rgba(entry.color)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(id: &str, x: f64) -> Target {
        Target {
            id: id.into(),
            bounds: Rect::new(x, 0., 200., 100.),
            scale: 1.25,
        }
    }
    #[test]
    fn desktop_geometry_matches_each_image_and_color_even_when_entries_are_reordered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("image.png");
        RgbaImage::from_pixel(100, 100, Rgba([240, 80, 20, 255]))
            .save(&path)
            .unwrap();
        let targets = vec![target("left", -200.), target("right", 0.)];
        let entries = vec![
            Entry {
                bounds: targets[1].bounds,
                path: None,
                style: Style::Fill,
                color: [32, 64, 96, 255],
            },
            Entry {
                bounds: targets[0].bounds,
                path: Some(path.clone()),
                style: Style::Fit,
                color: [2, 4, 6, 255],
            },
        ];
        let read = read(targets.clone(), entries.clone(), None);
        let pictures = read.pictures.unwrap();
        assert_eq!(pictures[0].0, "left");
        assert_eq!(pictures[0].1.dimensions(), (250, 125));
        assert_eq!(pictures[0].1.get_pixel(0, 0).0, [2, 4, 6, 255]);
        assert_eq!(pictures[0].1.get_pixel(125, 60).0, [240, 80, 20, 255]);
        assert!(pictures[1].1.pixels().all(|p| p.0 == [32, 64, 96, 255]));
        let cached = super::read(targets.clone(), entries.clone(), read.signature.as_deref());
        assert!(cached.pictures.is_none());
        // Same path, different file contents: the thumbnail must be decoded again.
        RgbaImage::from_pixel(20, 40, Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        let changed = super::read(targets.clone(), entries.clone(), read.signature.as_deref());
        assert!(changed.pictures.is_some());
        std::fs::remove_file(path).unwrap();
        let missing = super::read(targets, entries, changed.signature.as_deref());
        assert!(missing.signature.is_none());
        assert_eq!(missing.pictures.unwrap()[0].0, "right");
    }
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
            color: [0, 0, 100, 255],
        };
        let out = render(&entry, &target, std::slice::from_ref(&entry)).unwrap();
        assert_eq!(out.dimensions(), (512, 256));
        assert_eq!(out.get_pixel(200, 128).0, [0, 0, 100, 255]);
        assert_eq!(out.get_pixel(256, 128).0, [255, 0, 0, 255]);
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
        let mut targets = vec![target("left", -200.), target("right", 0.)];
        for target in &mut targets {
            target.scale = 1.;
        }
        let entries: Vec<_> = targets
            .iter()
            .map(|t| Entry {
                bounds: t.bounds,
                path: Some(path.clone()),
                style: Style::Span,
                color: [0, 0, 0, 255],
            })
            .collect();
        let out = read(targets, entries, None).pictures.unwrap();
        assert_eq!(out[0].1.get_pixel(100, 50).0, [255, 0, 0, 255]);
        assert_eq!(out[1].1.get_pixel(100, 50).0, [0, 255, 0, 255]);
    }
}
