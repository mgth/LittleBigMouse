use crate::{text, Pattern};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub pattern: Pattern,
    pub steps: u8,
    pub level: u8,
    pub colour: [u8; 3],
    pub line_width: u8,
    pub invert: bool,
    pub blink: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            pattern: Pattern::Contrast,
            steps: 11,
            level: 128,
            colour: [255; 3],
            line_width: 1,
            invert: false,
            blink: false,
        }
    }
}
impl Settings {
    pub fn alternates(&self) -> bool {
        self.blink
            && matches!(
                self.pattern,
                Pattern::Contrast | Pattern::Whites | Pattern::Blacks | Pattern::RgbClip
            )
    }
    pub fn solid_rgb(&self) -> [u8; 3] {
        self.colour
            .map(|c| ((u16::from(c) * u16::from(self.level) + 127) / 255) as u8)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Update {
    pub revision: u64,
    pub settings: Settings,
}

/// Phase only replaces the near-black/white patches with their background.
/// Labels stay visible, so alternation never changes the interpretation of a code.
pub fn render_settings(
    settings: &Settings,
    width: usize,
    height: usize,
    phase: bool,
) -> Result<Vec<u8>, String> {
    let size = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .filter(|n| *n > 0 && *n <= 256 * 1024 * 1024)
        .ok_or("Invalid pattern dimensions")?;
    let pattern = settings.pattern;
    if matches!(
        pattern,
        Pattern::Orange | Pattern::White | Pattern::Gradient | Pattern::Gamma
    ) {
        return crate::render_original(pattern, width, height);
    }
    let mut image = vec![0; size];
    let steps = usize::from(settings.steps.clamp(2, 32));
    let line = usize::from(settings.line_width.clamp(1, 3));
    let grid = (width.min(height) / 10).max(1);
    let solid = settings.solid_rgb();
    for y in 0..height {
        for x in 0..width {
            let rgb = match pattern {
                Pattern::GraySteps => {
                    let cell = x * steps / width;
                    [(cell * 255 / (steps - 1)) as u8; 3]
                }
                Pattern::GrayGradient => [(x * 255 / (width - 1).max(1)) as u8; 3],
                Pattern::Solid => solid,
                Pattern::Contrast | Pattern::Whites | Pattern::Blacks | Pattern::RgbClip => {
                    let rows = match pattern {
                        Pattern::Contrast => 2,
                        Pattern::RgbClip => 3,
                        _ => 1,
                    };
                    let row = (y * rows / height).min(rows - 1);
                    let cell = x * 16 / width;
                    let black =
                        pattern == Pattern::Blacks || (pattern == Pattern::Contrast && row == 0);
                    let start_y = row * height / rows;
                    let rh = ((row + 1) * height / rows - start_y).max(1);
                    let local_x = (x as f64 + 0.5) * 16. / width as f64 - cell as f64;
                    let local_y = (y - start_y) as f64 / rh as f64;
                    let patch = (0.13..0.87).contains(&local_x)
                        && (0.15..0.7).contains(&local_y)
                        && !(phase && settings.alternates());
                    let delta = if patch { cell as u8 } else { 0 };
                    let value = if black { delta } else { 255 - delta };
                    if pattern == Pattern::RgbClip {
                        std::array::from_fn(|channel| if channel == row { value } else { 0 })
                    } else {
                        [value; 3]
                    }
                }
                Pattern::Sharpness => {
                    if y < height * 2 / 3 {
                        let high = match x * 3 / width {
                            0 => (x / line) & 1 == 0,
                            1 => (y / line) & 1 == 0,
                            _ => ((x / line) ^ (y / line)) & 1 == 0,
                        };
                        [if high ^ settings.invert { 255 } else { 0 }; 3]
                    } else {
                        [if settings.invert { 255 } else { 0 }; 3]
                    }
                }
                Pattern::Geometry => {
                    let edge = x == 0 || y == 0 || x == width - 1 || y == height - 1;
                    let inset = [1, 5].into_iter().any(|percent| {
                        let mx = (width * percent / 100).max(1);
                        let my = (height * percent / 100).max(1);
                        ((x == mx || x == width.saturating_sub(mx + 1))
                            && y >= my
                            && y < height.saturating_sub(my))
                            || ((y == my || y == height.saturating_sub(my + 1))
                                && x >= mx
                                && x < width.saturating_sub(mx))
                    });
                    let dx = x as f64 + 0.5 - width as f64 / 2.;
                    let dy = y as f64 + 0.5 - height as f64 / 2.;
                    let circle = (dx.hypot(dy) - width.min(height) as f64 * 0.4).abs() <= 0.6;
                    if edge {
                        [255; 3]
                    } else if inset {
                        [255, 140, 0]
                    } else if circle || x == width / 2 || y == height / 2 {
                        [200; 3]
                    } else if x % grid == 0 || y % grid == 0 {
                        [60; 3]
                    } else {
                        [0; 3]
                    }
                }
                _ => unreachable!(),
            };
            let offset = (y * width + x) * 4;
            image[offset..offset + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    if width >= 480 && height >= 240 {
        let dimensions = [width, height];
        match pattern {
            Pattern::GraySteps => {
                for cell in 0..steps {
                    let code = (cell * 255 / (steps - 1)) as u8;
                    text(
                        &mut image,
                        dimensions,
                        &code.to_string(),
                        (width as f32 / steps as f32 * 0.28).clamp(10., 34.),
                        [
                            (cell as f32 + 0.5) * width as f32 / steps as f32,
                            height as f32 * 0.87,
                        ],
                        [if code > 127 { 0 } else { 255 }; 3],
                    );
                }
            }
            Pattern::Contrast | Pattern::Whites | Pattern::Blacks | Pattern::RgbClip => {
                let rows = match pattern {
                    Pattern::Contrast => 2,
                    Pattern::RgbClip => 3,
                    _ => 1,
                };
                for row in 0..rows {
                    let black =
                        pattern == Pattern::Blacks || (pattern == Pattern::Contrast && row == 0);
                    for cell in 0..16 {
                        let code = if black { cell } else { 255 - cell };
                        let rgb = if black {
                            [160; 3]
                        } else if pattern == Pattern::RgbClip && row == 2 {
                            [255; 3]
                        } else {
                            [0; 3]
                        };
                        text(
                            &mut image,
                            dimensions,
                            &code.to_string(),
                            (width as f32 / 70.).clamp(10., 34.),
                            [
                                (cell as f32 + 0.5) * width as f32 / 16.,
                                (row as f32 + 0.85) * height as f32 / rows as f32,
                            ],
                            rgb,
                        );
                    }
                }
            }
            Pattern::Sharpness => {
                for (index, scale) in [14., 22., 34.].into_iter().enumerate() {
                    text(
                        &mut image,
                        dimensions,
                        "Aa Bb 0123456789 · Fine detail / Nettete",
                        scale,
                        [
                            width as f32 / 2.,
                            height as f32 * (0.73 + index as f32 * 0.1),
                        ],
                        [if settings.invert { 0 } else { 255 }; 3],
                    );
                }
            }
            Pattern::Geometry => text(
                &mut image,
                dimensions,
                "Outer frame: 1 px · Insets: 1% / 5%",
                (height as f32 / 45.).clamp(14., 32.),
                [width as f32 / 2., height as f32 * 0.94],
                [255; 3],
            ),
            _ => {}
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pixel(settings: &Settings, x: usize, y: usize, phase: bool) -> [u8; 4] {
        render_settings(settings, 320, 180, phase).unwrap()[(y * 320 + x) * 4..][..4]
            .try_into()
            .unwrap()
    }
    #[test]
    fn numbered_levels_and_blink_preserve_background_and_channel_codes() {
        for (pattern, base) in [(Pattern::Blacks, 0), (Pattern::Whites, 255)] {
            let settings = Settings {
                pattern,
                blink: true,
                ..Default::default()
            };
            for cell in 0..16 {
                let value = if base == 0 {
                    cell as u8
                } else {
                    255 - cell as u8
                };
                assert_eq!(
                    pixel(&settings, cell * 20 + 10, 60, false),
                    [value, value, value, 255]
                );
                assert_eq!(
                    pixel(&settings, cell * 20 + 10, 60, true),
                    [base, base, base, 255]
                );
            }
        }
        let settings = Settings {
            pattern: Pattern::RgbClip,
            ..Default::default()
        };
        assert_eq!(pixel(&settings, 110, 20, false), [250, 0, 0, 255]);
        assert_eq!(pixel(&settings, 110, 80, false), [0, 250, 0, 255]);
        assert_eq!(pixel(&settings, 110, 140, false), [0, 0, 250, 255]);
    }
    #[test]
    fn neutral_steps_uniform_fields_and_native_geometry() {
        let settings = Settings {
            pattern: Pattern::GraySteps,
            steps: 3,
            ..Default::default()
        };
        assert_eq!(pixel(&settings, 10, 20, false), [0, 0, 0, 255]);
        assert_eq!(pixel(&settings, 160, 20, false), [127, 127, 127, 255]);
        assert_eq!(pixel(&settings, 319, 20, false), [255; 4]);
        let settings = Settings {
            pattern: Pattern::Solid,
            level: 64,
            colour: [255, 0, 0],
            ..Default::default()
        };
        assert_eq!(pixel(&settings, 17, 80, false), [64, 0, 0, 255]);
        let settings = Settings {
            pattern: Pattern::Geometry,
            ..Default::default()
        };
        assert_eq!(pixel(&settings, 319, 179, false), [255; 4]);
        for line_width in 1..=3 {
            let settings = Settings {
                pattern: Pattern::Sharpness,
                line_width,
                ..Default::default()
            };
            assert_eq!(pixel(&settings, 0, 2, false), [255; 4]);
            assert_eq!(
                pixel(&settings, line_width as usize, 2, false),
                [0, 0, 0, 255]
            );
        }
    }
}
