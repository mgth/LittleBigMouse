//! Read the desktop's actual wallpaper without changing it or the editable layout. The
//! reading and the drawing are `lbm_wallpaper::desktop`'s, shared with the agent, which
//! draws a remote's screens for its master's map the same way.
use image::RgbaImage;
use lbm_layout::model::Layout;
use lbm_wallpaper::desktop::{entry_for, render, Entry, Target};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

/// The longest side of a picture behind a screen on this machine's map.
const LIMIT: f64 = 512.;

type Pictures = Vec<(String, RgbaImage)>;

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
            let result = lbm_wallpaper::desktop::query()
                .map(|entries| read(targets, entries, previous.as_deref()));
            let _ = send.send(result);
        });
        None
    }
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
        if let Some(entry) = entry_for(target, &entries) {
            match render(entry, target, &entries, LIMIT) {
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
#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use lbm_layout::geo::Rect;
    use lbm_wallpaper::desktop::Repeat;
    use lbm_wallpaper::Style;
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
                repeat: Repeat::Both,
                color: [32, 64, 96, 255],
            },
            Entry {
                bounds: targets[0].bounds,
                path: Some(path.clone()),
                style: Style::Fit,
                repeat: Repeat::Both,
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
}
