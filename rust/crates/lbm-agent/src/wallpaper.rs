//! The span wallpaper: one image cut across the desktop, a slice per screen.
//!
//! Port of C#'s `SpanRenderer`. The geometry is already `lbm_layout::wallpaper`, which says
//! which part of the image belongs to which screen and at what size; what is left is the
//! cutting, and where the pieces go.
//!
//! Filenames are content-addressed — source, its mtime, the screen, the crop, the output
//! size — for a reason that is about the desktop rather than about caching: an unchanged
//! configuration maps to the same paths, so the desktop is asked for what it is already
//! showing and does nothing (no flicker); any change maps to new paths, so `org.kde.image`,
//! which caches by path, is made to read the file again. Slices nobody points at any more
//! are swept afterwards.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use lbm_layout::geo::Size;
use lbm_layout::model::Layout;
use lbm_layout::wallpaper::{self, Slice};
use lbm_store::wallpaper_settings::{
    LayoutWallpaperSettings, ScreenWallpaperKind, WallpaperMode, WallpaperStyle,
};

use crate::desktop::ScreenWallpaper;

/// Where the slices live (C#'s `SpanRenderer.OutputDir`).
pub fn output_dir() -> PathBuf {
    lbm_store::lbm_paths::data_dir().join("wallpapers")
}

/// Cuts `source` into `dir`, one file per slice, and answers screen id → file.
///
/// A slice that cannot be made is left out rather than failing the others: the desktop is
/// then asked for what could be cut, which is better than an unchanged wallpaper and much
/// better than no wallpaper. Everything skipped is said out loud.
pub fn render(dir: &Path, source: &Path, slices: &[Slice]) -> BTreeMap<String, PathBuf> {
    let mut made = BTreeMap::new();
    if slices.is_empty() {
        return made;
    }
    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!(
            "[lbm-agent] wallpaper: {} is not writable: {error}",
            dir.display()
        );
        return made;
    }
    let stamp = stamp(source);

    // Read once, and only if something actually has to be cut: the whole point of the
    // naming is that an apply which changes nothing opens no file at all.
    let mut image = None;
    for slice in slices {
        let crop = crop_of(slice);
        let (width, height) = output_of(slice);
        let file = dir.join(format!(
            "span_{:016x}.png",
            fingerprint(&format!(
                "{}|{stamp}|{}|{},{},{},{}|{width}x{height}",
                source.display(),
                slice.id,
                crop.0,
                crop.1,
                crop.2,
                crop.3
            ))
        ));

        if !file.exists() {
            let source_image = match image.get_or_insert_with(|| image::open(source)) {
                Ok(loaded) => &*loaded,
                Err(error) => {
                    eprintln!(
                        "[lbm-agent] wallpaper: {} cannot be read: {error}",
                        source.display()
                    );
                    return made;
                }
            };
            // Rounding can push the crop a pixel past the edge, and a screen can sit
            // outside an image smaller than the desktop: clamp, and skip what is left of
            // nothing.
            let Some((x, y, w, h)) = clamp(crop, source_image.width(), source_image.height())
            else {
                continue;
            };
            let cut = source_image.crop_imm(x, y, w, h).resize_exact(
                width,
                height,
                // Bicubic, as ImageSharp's Resize is by default.
                FilterType::CatmullRom,
            );
            // Written beside and moved into place: an apply must never find a half-written
            // file, and the desktop may be reading the one being replaced. The format is
            // said rather than guessed — the name being written ends in `.tmp`.
            let temporary = file.with_extension("png.tmp");
            if let Err(error) = cut
                .save_with_format(&temporary, image::ImageFormat::Png)
                .and_then(|()| {
                    std::fs::rename(&temporary, &file).map_err(image::ImageError::IoError)
                })
            {
                eprintln!(
                    "[lbm-agent] wallpaper: {} not written: {error}",
                    file.display()
                );
                let _ = std::fs::remove_file(&temporary);
                continue;
            }
        }

        made.insert(slice.id.clone(), file);
    }

    sweep(dir, &made);
    made
}

/// Everything named like a slice that no screen points at any more. A file the desktop is
/// still showing may refuse to go on some platforms; the next apply tries again.
fn sweep(dir: &Path, keep: &BTreeMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_slice = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("span_") && name.ends_with(".png"));
        if is_slice && !keep.values().any(|kept| *kept == path) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The crop in whole pixels, as C# rounds it.
fn crop_of(slice: &Slice) -> (i64, i64, i64, i64) {
    (
        slice.source_px.x().round() as i64,
        slice.source_px.y().round() as i64,
        (slice.source_px.width().round() as i64).max(1),
        (slice.source_px.height().round() as i64).max(1),
    )
}

fn output_of(slice: &Slice) -> (u32, u32) {
    (
        (slice.output_px.width().round() as i64).clamp(1, u32::MAX as i64) as u32,
        (slice.output_px.height().round() as i64).clamp(1, u32::MAX as i64) as u32,
    )
}

/// The crop, cut down to what the image actually has. `None` when nothing is left.
fn clamp(crop: (i64, i64, i64, i64), width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let (x, y, w, h) = crop;
    let left = x.max(0);
    let top = y.max(0);
    let right = (x + w).min(i64::from(width));
    let bottom = (y + h).min(i64::from(height));
    if right <= left || bottom <= top {
        return None;
    }
    Some((
        left as u32,
        top as u32,
        (right - left) as u32,
        (bottom - top) as u32,
    ))
}

/// What makes the source a different source: when it was last written. Unreadable means
/// "assume it changed", which costs a re-render and never shows a stale slice.
fn stamp(source: &Path) -> u128 {
    std::fs::metadata(source)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or_else(
            || {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |since| since.as_nanos())
            },
            |since| since.as_nanos(),
        )
}

/// FNV-1a, 64 bits. A file name, not a signature: C# hashes with SHA-256 and then keeps
/// 64 bits of it, so nothing here is weaker than what it replaces — and a handful of
/// slices in one directory is a long way from a collision either way.
fn fingerprint(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    text.bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

//==================//
// What to show     //
//==================//

/// What each screen of `layout` should show, with any slices rendered into `dir`
/// (C#'s `WallpaperManager.BuildScreens`).
///
/// Only screens the desktop actually has: a monitor with no attached source is not
/// somewhere a wallpaper can go.
pub fn screens(
    layout: &Layout,
    settings: &LayoutWallpaperSettings,
    dir: &Path,
) -> Vec<ScreenWallpaper> {
    let attached: Vec<&lbm_layout::model::Monitor> = layout
        .monitors()
        .iter()
        .filter(|monitor| on_the_desktop(layout, monitor))
        .collect();

    match settings.mode {
        WallpaperMode::Span => span(layout, &attached, settings, dir),
        WallpaperMode::PerScreen => per_screen(layout, &attached, settings),
    }
}

/// One image or colour per screen, each shown the way that screen asks.
fn per_screen(
    layout: &Layout,
    monitors: &[&lbm_layout::model::Monitor],
    settings: &LayoutWallpaperSettings,
) -> Vec<ScreenWallpaper> {
    let mut screens = Vec::new();
    for monitor in monitors {
        let Some(wanted) = settings.per_screen.get(&monitor.id) else {
            continue;
        };
        let Some(bounds) = pixel_bounds(layout, monitor) else {
            continue;
        };
        match wanted.kind {
            ScreenWallpaperKind::Image => {
                // A picture that has been moved or deleted is not shown as a black
                // screen: the desktop keeps what it had.
                let Some(path) = wanted
                    .image_path
                    .as_ref()
                    .filter(|p| Path::new(p).is_file())
                else {
                    continue;
                };
                screens.push(ScreenWallpaper {
                    x: bounds.0,
                    y: bounds.1,
                    image: Some(PathBuf::from(path)),
                    style: wanted.style,
                    color: wanted.color.clone(),
                });
            }
            ScreenWallpaperKind::Color => screens.push(ScreenWallpaper {
                x: bounds.0,
                y: bounds.1,
                image: None,
                style: wanted.style,
                color: wanted.color.clone(),
            }),
        }
    }
    screens
}

/// One image across the whole desktop, cut per screen.
fn span(
    layout: &Layout,
    monitors: &[&lbm_layout::model::Monitor],
    settings: &LayoutWallpaperSettings,
    dir: &Path,
) -> Vec<ScreenWallpaper> {
    let Some(source) = settings
        .span_image_path
        .as_ref()
        .filter(|path| Path::new(path).is_file())
    else {
        return Vec::new();
    };
    let source = Path::new(source);

    let bounds_mm = wallpaper::compute_bounds_mm(
        monitors
            .iter()
            .filter_map(|monitor| layout.depth_projection(monitor))
            .map(|projection| projection.outside_bounds()),
    );

    let inputs: Vec<wallpaper::ScreenInput> = monitors
        .iter()
        .filter_map(|monitor| {
            let projection = layout.depth_projection(monitor)?;
            Some(wallpaper::ScreenInput::new(
                monitor.id.clone(),
                projection.bounds(),
                panel_pixels(layout, monitor)?,
            ))
        })
        .collect();

    // The image's own size decides the cut, so it is read before anything is written.
    let Some(image_px) = measure(source) else {
        return Vec::new();
    };
    let cut = render(
        dir,
        source,
        &wallpaper::compute_slices(image_px, bounds_mm, &inputs),
    );

    monitors
        .iter()
        .filter_map(|monitor| {
            let file = cut.get(&monitor.id)?;
            let (x, y) = pixel_bounds(layout, monitor)?;
            Some(ScreenWallpaper {
                x,
                y,
                image: Some(file.clone()),
                // Each slice already has its screen's exact aspect: stretching maps it 1:1.
                style: WallpaperStyle::Stretch,
                color: String::new(),
            })
        })
        .collect()
}

/// How big the image is, without decoding it.
fn measure(source: &Path) -> Option<Size> {
    match image::image_dimensions(source) {
        Ok((width, height)) => Some(Size::new(f64::from(width), f64::from(height))),
        Err(error) => {
            eprintln!(
                "[lbm-agent] wallpaper: {} cannot be read: {error}",
                source.display()
            );
            None
        }
    }
}

fn on_the_desktop(layout: &Layout, monitor: &lbm_layout::model::Monitor) -> bool {
    active_source(layout, monitor).is_some_and(|source| source.attached_to_desktop)
}

fn active_source<'a>(
    layout: &'a Layout,
    monitor: &lbm_layout::model::Monitor,
) -> Option<&'a lbm_layout::model::DisplaySource> {
    Some(&layout.source(monitor.active_source.as_deref()?)?.source)
}

/// Where the desktop believes the screen is: its top-left in the cursor's own pixels,
/// which is what the desktop matches its screens by.
fn pixel_bounds(layout: &Layout, monitor: &lbm_layout::model::Monitor) -> Option<(f64, f64)> {
    let bounds = active_source(layout, monitor)?.in_pixel.bounds();
    Some((bounds.x(), bounds.y()))
}

/// The panel's own pixels, which is what a slice has to be rendered at.
///
/// `in_pixel` is the cursor space: the panel's pixels on Windows, the compositor's
/// logical size on Linux — where `effective_dpi = 96 × scale` recovers the panel.
fn panel_pixels(layout: &Layout, monitor: &lbm_layout::model::Monitor) -> Option<Size> {
    let source = active_source(layout, monitor)?;
    let (kx, ky) = if cfg!(windows) {
        (1.0, 1.0)
    } else {
        (source.effective_dpi.x / 96.0, source.effective_dpi.y / 96.0)
    };
    Some(Size::new(
        (source.in_pixel.width * kx).round().max(1.0),
        (source.in_pixel.height * ky).round().max(1.0),
    ))
}

/// What the desktop was last told, as one line. Comparing it with the last one is what
/// keeps an apply that changes nothing from reaching the desktop at all — and the slice
/// paths being content-addressed is what makes the comparison mean something.
pub fn signature(screens: &[ScreenWallpaper]) -> String {
    screens
        .iter()
        .map(|screen| {
            format!(
                "{},{}|{}|{:?}|{}",
                screen.x,
                screen.y,
                screen
                    .image
                    .as_ref()
                    .map_or(String::new(), |p| p.to_string_lossy().into_owned()),
                screen.style,
                screen.color
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use lbm_layout::geo::{Rect, Size};
    use lbm_layout::linux::{add_monitor, LinuxEdid, LinuxMonitor};
    use lbm_layout::model::LayoutOptions;
    use lbm_store::wallpaper_settings::ScreenWallpaperSettings;

    use super::*;

    /// Two screens side by side, as the Linux enumeration hands them over: the path the
    /// agent actually builds its layout through.
    fn desktop() -> Layout {
        let mut layout = Layout::new(LayoutOptions::default());
        for (index, connector) in ["DP-1", "DP-2"].iter().enumerate() {
            let at = 1920.0 * index as f64;
            add_monitor(
                &mut layout,
                &LinuxMonitor {
                    connector_name: (*connector).to_owned(),
                    logical_x: at,
                    logical_y: 0.0,
                    logical_width: 1920.0,
                    logical_height: 1080.0,
                    pixel_width: 1920,
                    pixel_height: 1080,
                    scale: 1.0,
                    width_mm: 600.0,
                    height_mm: 340.0,
                    primary: index == 0,
                    enabled: true,
                    orientation: 0,
                    frequency: 60,
                    edid: Some(LinuxEdid {
                        manufacturer_code: Some("TST".to_owned()),
                        product_code: Some(format!("000{index}")),
                        serial: None,
                        serial_number: Some(format!("SN{index}")),
                        model: Some("Test".to_owned()),
                        physical_width: 600.0,
                        physical_height: 340.0,
                        video_interface: None,
                    }),
                },
            );
        }
        layout
    }

    fn ids(layout: &Layout) -> Vec<String> {
        layout.monitors().iter().map(|m| m.id.clone()).collect()
    }

    /// A source whose every pixel says where it is: a crop can be checked by reading one.
    fn source(dir: &Path, width: u32, height: u32) -> PathBuf {
        let path = dir.join("desktop.png");
        image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 0])
        })
        .save(&path)
        .expect("a written source");
        path
    }

    fn slice(id: &str, source_px: Rect, output_px: Size) -> Slice {
        Slice {
            id: id.to_owned(),
            source_px,
            output_px,
        }
    }

    #[test]
    fn each_screen_shows_what_it_was_given() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let picture = source(dir.path(), 100, 100);
        let layout = desktop();
        let [left, right] = <[String; 2]>::try_from(ids(&layout)).expect("two screens");

        let mut settings = LayoutWallpaperSettings::default();
        settings.per_screen.insert(
            left.clone(),
            ScreenWallpaperSettings {
                kind: ScreenWallpaperKind::Image,
                image_path: Some(picture.to_string_lossy().into_owned()),
                style: WallpaperStyle::Fit,
                color: "#204060".to_owned(),
            },
        );
        settings.per_screen.insert(
            right.clone(),
            ScreenWallpaperSettings {
                kind: ScreenWallpaperKind::Color,
                image_path: None,
                style: WallpaperStyle::Fill,
                color: "#102030".to_owned(),
            },
        );

        let shown = screens(&layout, &settings, &dir.path().join("wallpapers"));

        assert_eq!(shown.len(), 2);
        assert_eq!(shown[0].image.as_deref(), Some(picture.as_path()));
        assert_eq!(shown[0].style, WallpaperStyle::Fit);
        assert_eq!((shown[0].x, shown[0].y), (0.0, 0.0));
        assert_eq!(shown[1].image, None, "a colour screen shows no file");
        assert_eq!(shown[1].color, "#102030");
        assert_eq!(
            (shown[1].x, shown[1].y),
            (1920.0, 0.0),
            "the second screen's own position, which is how the desktop finds it"
        );
    }

    #[test]
    fn a_picture_that_is_no_longer_there_leaves_the_screen_alone() {
        // Wallpapers outlive the files they point at. Sending nothing for that screen
        // keeps what the desktop is showing; sending a missing path would blank it.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let layout = desktop();
        let mut settings = LayoutWallpaperSettings::default();
        settings.per_screen.insert(
            ids(&layout)[0].clone(),
            ScreenWallpaperSettings {
                kind: ScreenWallpaperKind::Image,
                image_path: Some("/where/it/used/to/be.png".to_owned()),
                style: WallpaperStyle::Fill,
                color: "#204060".to_owned(),
            },
        );

        assert!(screens(&layout, &settings, dir.path()).is_empty());
    }

    #[test]
    fn a_screen_nobody_configured_is_not_touched() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let layout = desktop();

        let shown = screens(&layout, &LayoutWallpaperSettings::default(), dir.path());

        assert!(shown.is_empty());
    }

    #[test]
    fn a_span_reaches_every_screen_with_its_own_slice() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let picture = source(dir.path(), 3840, 1080);
        let layout = desktop();
        let settings = LayoutWallpaperSettings {
            mode: WallpaperMode::Span,
            span_image_path: Some(picture.to_string_lossy().into_owned()),
            per_screen: Default::default(),
        };

        let shown = screens(&layout, &settings, &dir.path().join("wallpapers"));

        assert_eq!(shown.len(), 2);
        for screen in &shown {
            let file = screen.image.as_ref().expect("a slice");
            assert!(file.exists(), "{} was written", file.display());
            // Each slice is already its screen's shape; stretching maps it 1:1.
            assert_eq!(screen.style, WallpaperStyle::Stretch);
        }
        assert_ne!(
            shown[0].image, shown[1].image,
            "a slice each, not one shared"
        );
        assert_eq!((shown[0].x, shown[1].x), (0.0, 1920.0));
    }

    #[test]
    fn a_span_with_no_image_shows_nothing_rather_than_black() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let layout = desktop();
        let settings = LayoutWallpaperSettings {
            mode: WallpaperMode::Span,
            span_image_path: Some("/gone.png".to_owned()),
            per_screen: Default::default(),
        };

        assert!(screens(&layout, &settings, dir.path()).is_empty());
    }

    #[test]
    fn the_signature_moves_only_when_the_desktop_would_see_it() {
        // What keeps a rebuild from rewriting an identical wallpaper: the slice paths are
        // content-addressed, so an unchanged desktop produces an unchanged line.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let picture = source(dir.path(), 3840, 1080);
        let out = dir.path().join("wallpapers");
        let layout = desktop();
        let settings = LayoutWallpaperSettings {
            mode: WallpaperMode::Span,
            span_image_path: Some(picture.to_string_lossy().into_owned()),
            per_screen: Default::default(),
        };

        let once = signature(&screens(&layout, &settings, &out));
        let twice = signature(&screens(&layout, &settings, &out));
        assert_eq!(once, twice);

        // Another picture is another desktop.
        let other = dir.path().join("other.png");
        std::fs::copy(&picture, &other).expect("a second picture");
        let moved = LayoutWallpaperSettings {
            span_image_path: Some(other.to_string_lossy().into_owned()),
            ..settings
        };
        assert_ne!(once, signature(&screens(&layout, &moved, &out)));
    }

    #[test]
    fn each_screen_gets_its_own_piece_at_its_own_size() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let image_path = source(dir.path(), 400, 100);
        let out = dir.path().join("wallpapers");

        let made = render(
            &out,
            &image_path,
            &[
                slice(
                    "LEFT",
                    Rect::new(0.0, 0.0, 200.0, 100.0),
                    Size::new(100.0, 50.0),
                ),
                slice(
                    "RIGHT",
                    Rect::new(200.0, 0.0, 200.0, 100.0),
                    Size::new(400.0, 200.0),
                ),
            ],
        );

        assert_eq!(made.len(), 2);
        let left = image::open(&made["LEFT"]).expect("a slice");
        let right = image::open(&made["RIGHT"]).expect("a slice");
        assert_eq!((left.width(), left.height()), (100, 50));
        assert_eq!((right.width(), right.height()), (400, 200));
        // Each screen shows its own half. The source paints its own x into the red
        // channel, so the first column of each slice says where it was cut from.
        let image::Rgb([from_left, _, _]) = left.to_rgb8()[(1, 1)];
        let image::Rgb([from_right, _, _]) = right.to_rgb8()[(1, 1)];
        assert!(
            from_left < 10,
            "the left slice starts at the image's edge: {from_left}"
        );
        assert!(
            (195..=205).contains(&from_right),
            "the right slice starts halfway across: {from_right}"
        );
    }

    #[test]
    fn the_same_configuration_writes_nothing_the_second_time() {
        // What keeps the desktop from flickering: the same paths come back, so the
        // desktop is asked for what it is already showing.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let image_path = source(dir.path(), 200, 100);
        let out = dir.path().join("wallpapers");
        let slices = [slice(
            "ONE",
            Rect::new(0.0, 0.0, 200.0, 100.0),
            Size::new(200.0, 100.0),
        )];

        let first = render(&out, &image_path, &slices);
        let written = std::fs::metadata(&first["ONE"])
            .and_then(|m| m.modified())
            .expect("a written slice");

        let again = render(&out, &image_path, &slices);

        assert_eq!(first, again, "the same configuration, the same files");
        assert_eq!(
            std::fs::metadata(&again["ONE"])
                .and_then(|m| m.modified())
                .expect("the slice"),
            written,
            "the file was not rewritten"
        );
    }

    #[test]
    fn a_changed_screen_gets_a_new_name_and_the_old_one_goes() {
        // The other half of the naming: org.kde.image caches by path, so a slice that
        // changed has to arrive under a name the desktop has never seen.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let image_path = source(dir.path(), 200, 100);
        let out = dir.path().join("wallpapers");

        let before = render(
            &out,
            &image_path,
            &[slice(
                "ONE",
                Rect::new(0.0, 0.0, 200.0, 100.0),
                Size::new(200.0, 100.0),
            )],
        );
        let after = render(
            &out,
            &image_path,
            &[slice(
                "ONE",
                Rect::new(0.0, 0.0, 100.0, 100.0),
                Size::new(200.0, 100.0),
            )],
        );

        assert_ne!(before["ONE"], after["ONE"]);
        assert!(
            !before["ONE"].exists(),
            "the slice nobody points at is swept"
        );
        assert!(after["ONE"].exists());
        assert_eq!(
            std::fs::read_dir(&out).expect("the directory").count(),
            1,
            "nothing else is left behind"
        );
    }

    #[test]
    fn a_screen_reaching_past_the_image_takes_what_there_is() {
        // A wallpaper smaller than the desktop: the crop is clamped rather than the
        // screen left with nothing. A screen entirely outside gets no slice at all.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let image_path = source(dir.path(), 100, 100);
        let out = dir.path().join("wallpapers");

        let made = render(
            &out,
            &image_path,
            &[
                slice(
                    "OVERLAPPING",
                    Rect::new(50.0, 0.0, 200.0, 100.0),
                    Size::new(80.0, 40.0),
                ),
                slice(
                    "OUTSIDE",
                    Rect::new(500.0, 0.0, 100.0, 100.0),
                    Size::new(80.0, 40.0),
                ),
            ],
        );

        assert_eq!(made.keys().collect::<Vec<_>>(), ["OVERLAPPING"]);
        let cut = image::open(&made["OVERLAPPING"]).expect("a slice");
        assert_eq!(
            (cut.width(), cut.height()),
            (80, 40),
            "asked for, not cropped to"
        );
    }

    #[test]
    fn an_image_that_cannot_be_read_costs_no_files() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let broken = dir.path().join("not-an-image.png");
        std::fs::write(&broken, b"this is not a PNG").expect("a written file");
        let out = dir.path().join("wallpapers");

        let made = render(
            &out,
            &broken,
            &[slice(
                "ONE",
                Rect::new(0.0, 0.0, 10.0, 10.0),
                Size::new(10.0, 10.0),
            )],
        );

        assert!(made.is_empty());
        // And nothing half-written was left where the desktop would find it.
        assert_eq!(
            std::fs::read_dir(&out).map(|entries| entries.count()).ok(),
            Some(0)
        );
    }

    #[test]
    fn nothing_to_cut_touches_nothing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let out = dir.path().join("wallpapers");
        assert!(render(&out, &dir.path().join("absent.png"), &[]).is_empty());
        assert!(!out.exists(), "no slices, no directory");
    }
}
