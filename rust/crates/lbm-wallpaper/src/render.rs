//! Bounded decoding and the same physical span for the local preview and desktop update.
use crate::{Rect, Size, Step, Style};
use image::{ImageReader, Rgba, RgbaImage};
use lbm_layout::{geo::Size as MmSize, model::Layout, wallpaper as span};
use lbm_store::wallpaper_settings::{
    LayoutWallpaperSettings, ScreenWallpaperKind, WallpaperMode, WallpaperStyle,
};
use std::path::Path;

pub struct Painted {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub pixels: RgbaImage,
}
pub fn color(value: &str) -> Result<[u8; 4], String> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("Use a color in #RRGGBB format.".into());
    }
    let channel = |at| u8::from_str_radix(&value[at..at + 2], 16).unwrap();
    Ok([channel(1), channel(3), channel(5), 255])
}
pub fn decode(path: &Path) -> Result<RgbaImage, String> {
    if !path.is_file() {
        return Err(format!("{} is not a regular image file.", path.display()));
    }
    let (w, h) = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())?;
    if u64::from(w) * u64::from(h) > 33_554_432 {
        return Err("The source image exceeds 32 megapixels.".into());
    }
    let mut reader = ImageReader::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("{}: {e}", path.display()))
}
pub fn one(
    image: RgbaImage,
    style: WallpaperStyle,
    width: u32,
    height: u32,
    fill: [u8; 4],
) -> Result<RgbaImage, String> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_777_216 {
        return Err("Wallpaper output exceeds the supported image size.".into());
    }
    let source = Size {
        w: image.width() as i32,
        h: image.height() as i32,
    };
    let target = Size {
        w: width as i32,
        h: height as i32,
    };
    if source.w == 0 || source.h == 0 {
        return Err("The wallpaper image is empty.".into());
    }
    let style = match style {
        WallpaperStyle::Fill => Style::Fill,
        WallpaperStyle::Fit => Style::Fit,
        WallpaperStyle::Stretch => Style::Stretch,
        WallpaperStyle::Center => Style::Center,
        WallpaperStyle::Tile => Style::Tile,
        WallpaperStyle::Span => {
            return Err("Use the shared Span mode for a panoramic image.".into())
        }
    };
    let mut steps = if style == Style::Tile {
        vec![Step::Tile { to: target }]
    } else {
        crate::on_one_screen(style, source, target)
    };
    // Integer recipe rounding can collapse an extreme aspect ratio to zero pixels.
    for step in &mut steps {
        match step {
            Step::Crop(r) => {
                r.w = r.w.max(1);
                r.h = r.h.max(1);
                *r = r.clamped(source);
            }
            Step::Resize(s) => {
                s.w = s.w.max(1);
                s.h = s.h.max(1);
            }
            _ => {}
        }
    }
    for step in &steps {
        let size = match step {
            Step::Resize(s) | Step::Pad { to: s, .. } | Step::Tile { to: s } => Some(s),
            _ => None,
        };
        if size.is_some_and(|s| i64::from(s.w) * i64::from(s.h) > 67_108_864) {
            return Err("This image aspect ratio requires an oversized intermediate image.".into());
        }
    }
    let mut out = crate::apply(image, &steps, Rgba(fill));
    opaque(&mut out, fill);
    Ok(out)
}
fn opaque(out: &mut RgbaImage, fill: [u8; 4]) {
    for p in out.pixels_mut() {
        if p[3] == 255 {
            continue;
        }
        let alpha = u32::from(p[3]);
        for c in 0..3 {
            p[c] = ((u32::from(p[c]) * alpha + u32::from(fill[c]) * (255 - alpha)) / 255) as u8;
        }
        p[3] = 255;
    }
}
/// `preview` bounds textures to 512 px per axis; full output uses the panel's native pixels.
pub fn paint(
    layout: &Layout,
    settings: &LayoutWallpaperSettings,
    preview: bool,
) -> Result<Vec<Painted>, String> {
    if layout.is_virtual() {
        return Err("A foreign layout cannot set this desktop's wallpaper.".into());
    }
    let attached: Vec<_> = layout
        .monitors()
        .iter()
        .filter_map(|m| {
            let s = &layout.source(m.active_source.as_deref()?)?.source;
            if !s.attached_to_desktop {
                return None;
            }
            Some((m, s, layout.depth_projection(m)?))
        })
        .collect();
    let bounds = span::compute_bounds_mm(attached.iter().map(|(_, _, p)| p.outside_bounds()));
    let source = if settings.mode == WallpaperMode::Span {
        Some(decode(Path::new(
            settings
                .span_image_path
                .as_deref()
                .filter(|p| !p.is_empty())
                .ok_or("Choose a panorama image.")?,
        ))?)
    } else {
        None
    };
    let mut result = Vec::new();
    let mut total = 0_u64;
    for (m, s, p) in attached {
        if settings.mode == WallpaperMode::PerScreen && !settings.per_screen.contains_key(&m.id) {
            continue;
        }
        let scale = if cfg!(windows) {
            1.
        } else {
            s.effective_dpi.y / 96.
        };
        let (w, h) = (s.in_pixel.width * scale, s.in_pixel.height * scale);
        if !w.is_finite() || !h.is_finite() || w < 1. || h < 1. || w * h > 16_777_216. {
            return Err("Invalid or oversized wallpaper panel dimensions.".into());
        }
        let ratio = if preview {
            (512. / w).min(512. / h).min(1.)
        } else {
            1.
        };
        let (width, height) = (
            (w * ratio).round().max(1.) as u32,
            (h * ratio).round().max(1.) as u32,
        );
        total += u64::from(width) * u64::from(height);
        if total > 67_108_864 {
            return Err("The combined wallpaper images exceed 256 MiB.".into());
        }
        let mut pixels = if let Some(source) = &source {
            let slices = span::compute_slices(
                MmSize::new(source.width().into(), source.height().into()),
                bounds,
                &[span::ScreenInput::new(
                    &m.id,
                    p.bounds(),
                    MmSize::new(width.into(), height.into()),
                )],
            );
            let slice = slices
                .first()
                .ok_or("The panorama does not intersect this screen.")?;
            let r = slice.source_px;
            if r.width() < 1. || r.height() < 1. {
                return Err("The panorama has an empty screen crop.".into());
            }
            crate::apply(
                source.clone(),
                &[
                    Step::Crop(Rect {
                        x: r.x().round() as i32,
                        y: r.y().round() as i32,
                        w: r.width().round().max(1.) as i32,
                        h: r.height().round().max(1.) as i32,
                    }),
                    Step::Resize(Size {
                        w: width as i32,
                        h: height as i32,
                    }),
                ],
                Rgba([0, 0, 0, 255]),
            )
        } else {
            let Some(wanted) = settings.per_screen.get(&m.id) else {
                continue;
            };
            let fill = color(&wanted.color)?;
            if wanted.kind == ScreenWallpaperKind::Color {
                RgbaImage::from_pixel(width, height, Rgba(fill))
            } else {
                let path = wanted
                    .image_path
                    .as_deref()
                    .filter(|p| !p.is_empty())
                    .ok_or("Choose an image for the configured screen, or choose Color.")?;
                let mut image = decode(Path::new(path))?;
                // Center and Tile measure original pixels; scale source and target together in previews.
                if preview
                    && matches!(wanted.style, WallpaperStyle::Center | WallpaperStyle::Tile)
                    && ratio < 1.
                {
                    image = image::imageops::resize(
                        &image,
                        (image.width() as f64 * ratio).round().max(1.) as u32,
                        (image.height() as f64 * ratio).round().max(1.) as u32,
                        image::imageops::FilterType::CatmullRom,
                    );
                }
                one(image, wanted.style, width, height, fill)?
            }
        };
        if settings.mode == WallpaperMode::Span {
            opaque(&mut pixels, [0, 0, 0, 255]);
        }
        result.push(Painted {
            id: m.id.clone(),
            x: s.in_pixel.x,
            y: s.in_pixel.y,
            pixels,
        });
    }
    if result.is_empty() {
        return Err("Configure at least one attached screen.".into());
    }
    Ok(result)
}
