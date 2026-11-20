//! Native SDR patterns for manual display adjustment. Samples are raw 8-bit codes.
use ab_glyph::{point, Font, FontRef, ScaleFont};

mod manual;
#[cfg(target_os = "linux")]
pub mod viewer;
pub use manual::{render_settings, Settings, Update};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Pattern {
    Contrast,
    Whites,
    Blacks,
    Orange,
    White,
    Gradient,
    Gamma,
    GraySteps,
    GrayGradient,
    Solid,
    Sharpness,
    Geometry,
    RgbClip,
}

impl Pattern {
    pub const ALL: [Self; 13] = [
        Self::Contrast,
        Self::Whites,
        Self::Blacks,
        Self::Orange,
        Self::White,
        Self::Gradient,
        Self::Gamma,
        Self::GraySteps,
        Self::GrayGradient,
        Self::Solid,
        Self::Sharpness,
        Self::Geometry,
        Self::RgbClip,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Contrast => "contrast",
            Self::Whites => "whites",
            Self::Blacks => "blacks",
            Self::Orange => "orange",
            Self::White => "white",
            Self::Gradient => "gradient",
            Self::Gamma => "gamma",
            Self::GraySteps => "gray-steps",
            Self::GrayGradient => "gray-gradient",
            Self::Solid => "solid",
            Self::Sharpness => "sharpness",
            Self::Geometry => "geometry",
            Self::RgbClip => "rgb-clip",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Contrast => "Contrast",
            Self::Whites => "White levels",
            Self::Blacks => "Black levels",
            Self::Orange => "Orange circle",
            Self::White => "White circle",
            Self::Gradient => "RGB gradients",
            Self::Gamma => "Gamma",
            Self::GraySteps => "Gray steps",
            Self::GrayGradient => "Gray ramp",
            Self::Solid => "Solid field",
            Self::Sharpness => "Fine detail",
            Self::Geometry => "Grid and framing",
            Self::RgbClip => "RGB clipping",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Contrast | Self::Whites | Self::Blacks => "Numbered near-black and near-white levels. Compare visible steps without forcing the faintest ones to stand out.",
            Self::Orange => "Orange circle on black, as in the original test patterns.",
            Self::White => "White circle on black: geometry and visible halos.",
            Self::Gradient => "Linear code ramps: white, red, green, blue and inverse grey.",
            Self::Gamma => "Gamma 1.2–3.0: compare the smooth ramp to the native one-pixel checkerboard.",
            Self::GraySteps => "Neutral steps from black to white. Look for colour casts in shadows, midtones and highlights.",
            Self::GrayGradient => "A neutral ramp from code 0 to 255. Look for tint changes and banding.",
            Self::Solid => "Uniform field. Examine tint, panel uniformity and defective pixels, or compare screens.",
            Self::Sharpness => "Native 1, 2 or 3 pixel stripes and checkerboard. Preserve detail without added halos; compare the text below.",
            Self::Geometry => "One-pixel outer frame, square grid and centred circle. Check cropping, overscan and stretching.",
            Self::RgbClip => "Numbered near-white levels in each RGB channel. Look for highlights that merge as contrast or gains increase.",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}

/// Opaque RGBA, at the panel's native resolution. No colour-space conversion.
pub fn render(pattern: Pattern, width: usize, height: usize) -> Result<Vec<u8>, String> {
    render_settings(
        &Settings {
            pattern,
            ..Default::default()
        },
        width,
        height,
        false,
    )
}

fn render_original(pattern: Pattern, width: usize, height: usize) -> Result<Vec<u8>, String> {
    let len = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .filter(|n| *n > 0 && *n <= 256 * 1024 * 1024)
        .ok_or("Invalid pattern dimensions")?;
    let mut pixels = vec![0; len];
    // The gamma curve depends on the row only; keep powers out of the pixel loop.
    let gamma: Vec<u8> = (0..height)
        .map(|y| (255. * 0.5_f64.powf(1. / (1.15 + y as f64 / height as f64 * 1.9))).round() as u8)
        .collect();
    for (y, &gamma_code) in gamma.iter().enumerate() {
        for x in 0..width {
            let rgb = match pattern {
                Pattern::GraySteps
                | Pattern::GrayGradient
                | Pattern::Solid
                | Pattern::Sharpness
                | Pattern::Geometry
                | Pattern::RgbClip => unreachable!("manual renderer"),
                Pattern::Contrast | Pattern::Whites | Pattern::Blacks => {
                    let split = pattern == Pattern::Contrast;
                    let black = pattern == Pattern::Blacks || (split && y < height / 2);
                    let row_start = if split && y >= height / 2 {
                        height / 2
                    } else {
                        0
                    };
                    let row_height = if split {
                        if row_start == 0 {
                            height / 2
                        } else {
                            height - row_start
                        }
                    } else {
                        height
                    };
                    let cell = x * 5 / width;
                    let cw = width as f64 / 5.;
                    let dx = x as f64 + 0.5 - (cell as f64 + 0.5) * cw;
                    let dy = (y - row_start) as f64 + 0.5 - row_height as f64 * 0.5;
                    let radius = cw.min(row_height as f64) * 0.45;
                    let inside = dx * dx + dy * dy <= radius * radius;
                    let step = if inside { cell as u8 + 1 } else { 0 };
                    [if black { step } else { 255 - step }; 3]
                }
                Pattern::Orange | Pattern::White => {
                    let dx = x as f64 + 0.5 - width as f64 * 0.5;
                    let dy = y as f64 + 0.5 - height as f64 * 0.5;
                    let r = width.min(height) as f64 * 0.45;
                    let coverage = (r + 0.5 - dx.hypot(dy)).clamp(0., 1.);
                    let colour = if pattern == Pattern::Orange {
                        [255., 128., 0.]
                    } else {
                        [255.; 3]
                    };
                    colour.map(|c| (c * coverage).round() as u8)
                }
                Pattern::Gradient => {
                    let value = (255. * (1. - x as f64 / (width - 1).max(1) as f64)).round() as u8;
                    match y * 5 / height {
                        0 => [value; 3],
                        1 => [value, 0, 0],
                        2 => [0, value, 0],
                        3 => [0, 0, value],
                        _ => [255 - value; 3],
                    }
                }
                Pattern::Gamma => {
                    let column = x * 5 / width;
                    let local = (x as f64 + 0.5) * 5. / width as f64 - column as f64;
                    let a = match column {
                        0 => [255; 3],
                        1 => [255, 0, 0],
                        2 => [0, 255, 0],
                        3 => [0, 0, 255],
                        _ => [0; 3],
                    };
                    let b = if column == 4 { [255; 3] } else { [0; 3] };
                    if (4. / 9. ..5. / 9.).contains(&local) {
                        b
                    } else if (1. / 3. ..2. / 3.).contains(&local) {
                        let tick = y as f64 / height as f64 * 19.;
                        if (tick - tick.round()).abs() < 19. / height as f64 * 0.6 {
                            a.map(|c| if c == 0 { 0 } else { gamma_code })
                        } else if (x ^ y) & 1 == 0 {
                            a
                        } else {
                            b
                        }
                    } else {
                        a.map(|c| if c == 0 { 0 } else { gamma_code })
                    }
                }
            };
            let i = (y * width + x) * 4;
            pixels[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    if pattern == Pattern::Gamma && width >= 500 && height >= 300 {
        for column in 0..5 {
            let colour = match column {
                0 => [255; 3],
                1 => [255, 0, 0],
                2 => [0, 255, 0],
                3 => [0, 0, 255],
                _ => [0; 3],
            };
            for tick in 0..19 {
                let value = 1.2 + tick as f32 * 0.1;
                let y = ((value - 1.15) / 1.9 * height as f32) as i32;
                text(
                    &mut pixels,
                    [width, height],
                    &format!("{value:.1}"),
                    (height as f32 / 38.).min(width as f32 / 55.),
                    [(column as f32 + 0.5) * width as f32 / 5., y as f32],
                    colour,
                );
            }
        }
    }
    Ok(pixels)
}

fn text(
    pixels: &mut [u8],
    size: [usize; 2],
    label: &str,
    scale: f32,
    centre: [f32; 2],
    rgb: [u8; 3],
) {
    let font = FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("bundled font");
    let scaled = font.as_scaled(scale);
    let width: f32 = label
        .chars()
        .map(|c| scaled.h_advance(font.glyph_id(c)))
        .sum();
    let mut x = centre[0] - width / 2.;
    for c in label.chars() {
        let id = font.glyph_id(c);
        let glyph = id.with_scale_and_position(
            scale,
            point(x, centre[1] + (scaled.ascent() + scaled.descent()) / 2.),
        );
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, alpha| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px >= 0 && py >= 0 && px < size[0] as i32 && py < size[1] as i32 {
                    let offset = (py as usize * size[0] + px as usize) * 4;
                    for channel in 0..3 {
                        pixels[offset + channel] = (rgb[channel] as f32 * alpha
                            + pixels[offset + channel] as f32 * (1. - alpha))
                            .round() as u8;
                    }
                }
            });
        }
        x += scaled.h_advance(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contrast_keeps_exact_codes_and_gradients_reach_endpoints() {
        let pixel = |p, x: usize, y: usize| {
            let image = render(p, 100, 100).unwrap();
            image[(y * 100 + x) * 4]
        };
        assert_eq!(pixel(Pattern::Gradient, 0, 5), 255);
        assert_eq!(pixel(Pattern::Gradient, 99, 5), 0);
        assert!(render(Pattern::Gamma, usize::MAX, 5).is_err());
    }
    #[test]
    fn gamma_has_unfiltered_one_pixel_cells_and_correct_code_at_22() {
        let image = render(Pattern::Gamma, 500, 380).unwrap();
        let pixel = |x: usize, y: usize| image[(y * 500 + x) * 4];
        assert_eq!(pixel(35, 209) + pixel(36, 209), 255);
        assert_ne!(pixel(35, 209), pixel(36, 209));
        assert_eq!(pixel(2, 210), 186);
    }
}
