//! A remote's wallpapers for its master's map (docs/multi-pc.md, "Wallpapers"): what each
//! of its screens shows, drawn as the desktop draws it and made small enough to cross the
//! link beside the input stream's control messages.
//!
//! Pure: the desktop is read elsewhere (the agent's `Desktop`), and this only decides what
//! to send for what was read.

use base64::Engine;
use image::{imageops::FilterType, RgbImage, RgbaImage};
use lbm_ipc::peer::{PeerDisplay, PeerWallpaper};
use lbm_layout::geo::Rect;
use lbm_wallpaper::desktop::{entry_for, render, Entry, Target};

/// The widest picture sent, in pixels. A screen on the master's map is rarely drawn
/// wider than this, and the height follows the screen's shape.
pub const MAX_WIDTH: u32 = 384;
/// The largest picture sent for one display, encoded. Past it the picture is made
/// smaller, then given up for its average colour.
pub const MAX_IMAGE_BYTES: usize = 96 * 1024;
/// The largest set of pictures in one message. Past it the largest pictures are replaced
/// by their average colour until the rest fits: a message is never refused for its size.
pub const MAX_MESSAGE_BYTES: usize = 512 * 1024;

/// The JPEG qualities tried, best first, at each size.
const QUALITIES: [u8; 3] = [80, 65, 50];
/// The sizes tried, as a fraction of the thumbnail's.
const SHRINKS: [u32; 3] = [1, 2, 4];

/// One display's thumbnail before it is encoded for the wire.
#[derive(Clone, Debug, PartialEq)]
struct Thumbnail {
    display: String,
    /// The encoded picture, when there is one to send.
    image: Option<Vec<u8>>,
    /// The average colour of the picture, or the solid colour shown.
    color: [u8; 3],
}

/// What each of `displays` shows, by the desktop's `entries`. A display the desktop said
/// nothing about (detached, or a geometry the desktop does not report) is left out, and
/// so is one whose picture cannot be read: the master then draws it as it would without.
pub fn thumbnails(displays: &[PeerDisplay], entries: &[Entry]) -> Vec<PeerWallpaper> {
    let thumbnails = displays
        .iter()
        .filter(|d| d.attached && d.width > 0 && d.height > 0)
        .filter_map(|display| thumbnail(display, entries))
        .collect();
    within_message(thumbnails, MAX_MESSAGE_BYTES)
        .into_iter()
        .map(|t| PeerWallpaper {
            display: t.display,
            image: t
                .image
                .as_ref()
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes)),
            color: t.image.is_none().then(|| hex(t.color)),
        })
        .collect()
}

fn thumbnail(display: &PeerDisplay, entries: &[Entry]) -> Option<Thumbnail> {
    let target = Target {
        id: display.id.clone(),
        bounds: Rect::new(
            f64::from(display.x),
            f64::from(display.y),
            f64::from(display.width),
            f64::from(display.height),
        ),
        scale: 1.,
    };
    let entry = entry_for(&target, entries)?;
    if entry.path.is_none() {
        // A solid background is a colour, not a picture of one.
        let [r, g, b, _] = entry.color;
        return Some(Thumbnail {
            display: display.id.clone(),
            image: None,
            color: [r, g, b],
        });
    }
    // The limit is on the width: a portrait screen keeps its shape at that width, which
    // makes it taller than wide but still small (`render` bounds both sides alike).
    let limit = f64::from(MAX_WIDTH) * (target.bounds.height() / target.bounds.width()).max(1.);
    let pixels = match render(entry, &target, entries, limit) {
        Ok(pixels) => pixels,
        Err(error) => {
            eprintln!(
                "[lbm-agent] multi-PC: the wallpaper of {} cannot be drawn: {error}",
                display.id
            );
            return None;
        }
    };
    let pixels = within_width(pixels, MAX_WIDTH);
    Some(Thumbnail {
        display: display.id.clone(),
        color: average(&pixels),
        image: encode(&pixels, MAX_IMAGE_BYTES),
    })
}

/// `pixels` no wider than `width`, its shape kept.
fn within_width(pixels: RgbaImage, width: u32) -> RgbaImage {
    if pixels.width() <= width {
        return pixels;
    }
    let height =
        (u64::from(pixels.height()) * u64::from(width) / u64::from(pixels.width())).max(1) as u32;
    image::imageops::resize(&pixels, width, height, FilterType::Triangle)
}

/// A JPEG of `pixels` no larger than `bound`: the best quality that fits, at the largest
/// size that fits. `None` when even the smallest does not — the caller sends the colour.
pub(crate) fn encode(pixels: &RgbaImage, bound: usize) -> Option<Vec<u8>> {
    // A wallpaper is opaque, and JPEG has no alpha: what a transparent pixel would let
    // through is the desktop's colour, which the renderer has already painted.
    let rgb: RgbImage = image::DynamicImage::ImageRgba8(pixels.clone()).to_rgb8();
    for shrink in SHRINKS {
        let (w, h) = (rgb.width() / shrink, rgb.height() / shrink);
        if w == 0 || h == 0 {
            break;
        }
        let sized = if shrink == 1 {
            rgb.clone()
        } else {
            image::imageops::resize(&rgb, w, h, FilterType::Triangle)
        };
        for quality in QUALITIES {
            let mut bytes = Vec::new();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
            if sized.write_with_encoder(encoder).is_err() {
                return None;
            }
            if bytes.len() <= bound {
                return Some(bytes);
            }
        }
    }
    None
}

/// The mean of every pixel: what a picture that cannot be sent is drawn as.
fn average(pixels: &RgbaImage) -> [u8; 3] {
    let count = u64::from(pixels.width()) * u64::from(pixels.height());
    if count == 0 {
        return [0, 0, 0];
    }
    let mut sum = [0u64; 3];
    for pixel in pixels.pixels() {
        for (total, channel) in sum.iter_mut().zip(pixel.0) {
            *total += u64::from(channel);
        }
    }
    sum.map(|total| (total / count) as u8)
}

/// The pictures of one message no larger than `bound` in all: the largest given up for
/// their colour first, since one large picture is worth less than several small ones.
fn within_message(mut thumbnails: Vec<Thumbnail>, bound: usize) -> Vec<Thumbnail> {
    let size = |t: &[Thumbnail]| -> usize {
        t.iter()
            .filter_map(|t| t.image.as_ref())
            .map(Vec::len)
            .sum()
    };
    while size(&thumbnails) > bound {
        let Some(largest) = thumbnails
            .iter_mut()
            .filter(|t| t.image.is_some())
            .max_by_key(|t| t.image.as_ref().map_or(0, Vec::len))
        else {
            break;
        };
        largest.image = None;
    }
    thumbnails
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use lbm_wallpaper::desktop::Repeat;
    use lbm_wallpaper::Style;

    fn display(id: &str, x: i32, width: i32, height: i32) -> PeerDisplay {
        PeerDisplay {
            id: id.into(),
            connector: id.into(),
            name: id.into(),
            edid: None,
            x,
            y: 0,
            width,
            height,
            scale: 1.0,
            effective_dpi: 96.0,
            orientation: 0,
            frequency: 60.0,
            primary: x == 0,
            attached: true,
            width_mm: 0.0,
            height_mm: 0.0,
            input_source: None,
        }
    }

    fn entry(display: &PeerDisplay, path: Option<std::path::PathBuf>) -> Entry {
        Entry {
            bounds: Rect::new(
                f64::from(display.x),
                0.,
                f64::from(display.width),
                f64::from(display.height),
            ),
            path,
            style: Style::Fill,
            repeat: Repeat::Both,
            color: [0x20, 0x40, 0x60, 255],
        }
    }

    /// Noise: the worst case for a JPEG, as a photograph full of detail is.
    fn noise(width: u32, height: u32) -> RgbaImage {
        let mut state = 0x2545_f491_u32;
        RgbaImage::from_fn(width, height, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let [r, g, b, _] = state.to_le_bytes();
            Rgba([r, g, b, 255])
        })
    }

    fn decoded(wallpaper: &PeerWallpaper) -> RgbaImage {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(wallpaper.image.as_ref().unwrap())
            .unwrap();
        image::load_from_memory(&bytes).unwrap().to_rgba8()
    }

    #[test]
    fn a_picture_is_sent_no_wider_than_the_limit_in_the_screens_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wall.png");
        RgbaImage::from_pixel(3840, 2160, Rgba([200, 100, 50, 255]))
            .save(&path)
            .unwrap();
        let screens = [
            display("wide", 0, 2560, 1440),
            display("tall", 2560, 1080, 1920),
        ];
        let entries: Vec<_> = screens
            .iter()
            .map(|d| entry(d, Some(path.clone())))
            .collect();

        let sent = thumbnails(&screens, &entries);
        assert_eq!(sent.len(), 2);
        let wide = decoded(&sent[0]);
        assert_eq!(wide.dimensions(), (384, 216));
        let tall = decoded(&sent[1]);
        assert_eq!(
            tall.width(),
            384,
            "the width is the limit, whatever the shape"
        );
        assert_eq!(tall.height(), 683);
        let pixel = wide.get_pixel(100, 100).0;
        assert!(
            pixel[0].abs_diff(200) < 8 && pixel[1].abs_diff(100) < 8,
            "{pixel:?}"
        );
        assert!(sent.iter().all(|w| w.color.is_none()));
    }

    #[test]
    fn a_solid_background_is_sent_as_its_colour() {
        let screen = display("eDP-1", 0, 1920, 1080);
        let sent = thumbnails(std::slice::from_ref(&screen), &[entry(&screen, None)]);
        assert_eq!(
            sent,
            [PeerWallpaper {
                display: "eDP-1".into(),
                image: None,
                color: Some("#204060".into()),
            }]
        );
    }

    #[test]
    fn what_the_desktop_said_nothing_about_is_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let screen = display("eDP-1", 0, 1920, 1080);
        let mut detached = display("HDMI-1", 1920, 1920, 1080);
        detached.attached = false;
        let elsewhere = display("DP-1", 5000, 1920, 1080);
        let unreadable = entry(&screen, Some(dir.path().join("gone.png")));
        let sent = thumbnails(
            &[screen.clone(), detached.clone(), elsewhere],
            &[unreadable, entry(&detached, None)],
        );
        assert!(sent.is_empty(), "{sent:?}");
    }

    #[test]
    fn a_picture_too_large_is_made_smaller_then_given_up_for_its_colour() {
        let pixels = noise(384, 216);
        let encoded = encode(&pixels, MAX_IMAGE_BYTES).expect("noise fits once smaller");
        assert!(encoded.len() <= MAX_IMAGE_BYTES);
        // Nothing fits a bound that small: the caller is told so, and sends the colour.
        assert_eq!(encode(&pixels, 64), None);
    }

    #[test]
    fn a_message_never_exceeds_its_bound_and_keeps_the_smaller_pictures() {
        let picture = |display: &str, size: usize| Thumbnail {
            display: display.into(),
            image: Some(vec![0; size]),
            color: [1, 2, 3],
        };
        let kept = within_message(
            vec![
                picture("a", 90 * 1024),
                picture("b", 300 * 1024),
                picture("c", 200 * 1024),
            ],
            MAX_MESSAGE_BYTES,
        );
        assert!(kept[0].image.is_some());
        assert!(kept[1].image.is_none(), "the largest goes first");
        assert!(kept[2].image.is_some());
        let total: usize = kept
            .iter()
            .filter_map(|t| t.image.as_ref())
            .map(Vec::len)
            .sum();
        assert!(total <= MAX_MESSAGE_BYTES);
    }

    /// The worst case of one message: eight screens of fine detail, which JPEG does
    /// worst with. Every picture keeps within its bound and the message within its own
    /// and the control frame's. (KDE's own wallpapers come out at 8–21 KiB on a 16:9
    /// screen and 22–47 KiB on a portrait one: docs/multi-pc.md.)
    #[test]
    fn eight_screens_of_fine_detail_fit_one_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grain.png");
        noise(768, 432).save(&path).unwrap();
        let screens: Vec<_> = (0..8)
            .map(|i| display(&format!("D{i}"), i * 768, 768, 432))
            .collect();
        let entries: Vec<_> = screens
            .iter()
            .map(|d| entry(d, Some(path.clone())))
            .collect();
        let sent = thumbnails(&screens, &entries);
        assert_eq!(sent.len(), 8);
        let sizes: Vec<usize> = sent
            .iter()
            .filter_map(|w| w.image.as_ref())
            .map(|image| {
                base64::engine::general_purpose::STANDARD
                    .decode(image)
                    .unwrap()
                    .len()
            })
            .collect();
        assert!(sizes.iter().all(|&s| s <= MAX_IMAGE_BYTES), "{sizes:?}");
        assert!(
            sizes.iter().sum::<usize>() <= MAX_MESSAGE_BYTES,
            "{sizes:?}"
        );
        let message = serde_json::to_vec(&lbm_net::wire::Control::Wallpapers {
            revision: 1,
            wallpapers: sent,
        })
        .unwrap();
        assert!(message.len() < lbm_net::wire::MAX_CONTROL_SIZE);
    }
}
