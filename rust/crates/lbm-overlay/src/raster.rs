//! Small, deterministic CPU renderer. BGRA pixels are premultiplied for layered windows.
use crate::scene::{ticks, Band, Content, Side};
use ab_glyph::{point, Font, FontRef, OutlinedGlyph, ScaleFont};
use std::sync::OnceLock;

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub bgra: Vec<u8>,
}
impl Image {
    fn fill(&mut self, x: i32, y: i32, width: i32, height: i32, rgba: [u8; 4]) {
        let [r, g, b, a] = rgba;
        let pixel = [
            ((b as u16 * a as u16) / 255) as u8,
            ((g as u16 * a as u16) / 255) as u8,
            ((r as u16 * a as u16) / 255) as u8,
            a,
        ];
        for y in y.max(0)..y.saturating_add(height).min(self.height as i32) {
            for x in x.max(0)..x.saturating_add(width).min(self.width as i32) {
                let offset = (y as usize * self.width + x as usize) * 4;
                self.bgra[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
    }
    fn strip(&mut self, side: Side, start: i32, end: i32, depth: i32, color: [u8; 4]) {
        match side {
            Side::Top => self.fill(start, 0, end - start, depth, color),
            Side::Bottom => self.fill(start, self.height as i32 - depth, end - start, depth, color),
            Side::Left => self.fill(0, start, depth, end - start, color),
            Side::Right => self.fill(self.width as i32 - depth, start, depth, end - start, color),
        }
    }
    fn text(&mut self, label: &Label, x: i32, y: i32, color: [u8; 4]) {
        for glyph in &label.glyphs {
            let bounds = glyph.px_bounds();
            glyph.draw(|gx, gy, coverage| {
                let x = x + bounds.min.x as i32 - label.left + gx as i32;
                let y = y + bounds.min.y as i32 - label.top + gy as i32;
                if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
                    return;
                }
                // Source-over in premultiplied BGRA: partial glyph coverage must
                // blend with the translucent ruler, not punch holes through it.
                let alpha = (coverage * f32::from(color[3])).round() as u16;
                let offset = (y as usize * self.width + x as usize) * 4;
                let pixel = &mut self.bgra[offset..offset + 4];
                for (dst, src) in pixel.iter_mut().zip([color[2], color[1], color[0], 255]) {
                    *dst = ((u16::from(src) * alpha + u16::from(*dst) * (255 - alpha) + 127) / 255)
                        as u8;
                }
            });
        }
    }
}

/// Use the same bundled proportional face as egui, rasterized at native pixel
/// resolution (including fractional display scales), with no bitmap enlargement.
struct Label {
    glyphs: Vec<OutlinedGlyph>,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}
impl Label {
    fn new(text: &str, ink_height: f32) -> Self {
        static FONT: OnceLock<FontRef<'static>> = OnceLock::new();
        let font = FONT.get_or_init(|| {
            FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT)
                .expect("bundled Ubuntu font is valid")
        });
        let zero = font
            .outline(font.glyph_id('0'))
            .expect("font contains digits");
        let scale = ink_height * font.height_unscaled() / zero.bounds.height().abs();
        let scaled = font.as_scaled(scale);
        let mut glyphs = Vec::new();
        let mut cursor = 0.;
        let mut previous = None;
        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(previous) = previous {
                cursor += scaled.kern(previous, id);
            }
            if let Some(glyph) =
                font.outline_glyph(id.with_scale_and_position(scale, point(cursor, 0.)))
            {
                glyphs.push(glyph);
            }
            cursor += scaled.h_advance(id);
            previous = Some(id);
        }
        let mut left = 0;
        let mut top = 0;
        let mut right = 0;
        let mut bottom = 0;
        for glyph in &glyphs {
            let bounds = glyph.px_bounds();
            left = left.min(bounds.min.x as i32);
            top = top.min(bounds.min.y as i32);
            right = right.max(bounds.max.x as i32);
            bottom = bottom.max(bounds.max.y as i32);
        }
        Self {
            glyphs,
            left,
            top,
            width: right - left,
            height: bottom - top,
        }
    }
}

pub fn render(band: &Band) -> Result<Image, String> {
    let (width, height) = (band.rect.width, band.rect.height);
    if width <= 0
        || height <= 0
        || i64::from(width) * i64::from(height) > 16_777_216
        || !band.axis_mm.is_finite()
        || band.axis_mm <= 0.
        || !band.cross_pixels_per_mm.is_finite()
        || band.cross_pixels_per_mm <= 0.
        || !band.scale.is_finite()
        || !(0.5..=8.).contains(&band.scale)
    {
        return Err("Overlay geometry exceeds the supported image bounds.".into());
    }
    let mut image = Image {
        width: width as usize,
        height: height as usize,
        bgra: vec![0; width as usize * height as usize * 4],
    };
    let axis = band.axis_pixels();
    let depth = band.thickness();
    if let Content::Guide { kind, .. } = band.content {
        let dash = (4. * band.scale).round().max(1.) as i32;
        for start in (0..axis).step_by((2 * dash) as usize) {
            image.strip(
                band.side,
                start,
                (start + dash).min(axis),
                depth,
                kind.rgba(),
            );
        }
        return Ok(image);
    }
    image.strip(band.side, 0, axis, depth, [20, 25, 32, 160]);
    match &band.content {
        Content::Ruler {
            zero_mm,
            measured_mm,
        } => {
            let start = band.pixel_at(*zero_mm).round().clamp(0., axis as f64) as i32;
            let end = band
                .pixel_at(zero_mm + measured_mm)
                .round()
                .clamp(0., axis as f64) as i32;
            image.strip(band.side, start, end, depth, [20, 75, 95, 200]);
            for tick in ticks(band) {
                let position = tick.pixel.round() as i32;
                if position < 0 || position >= axis {
                    continue;
                }
                let color = if tick.inside {
                    [240, 245, 248, 255]
                } else {
                    [175, 183, 190, 200]
                };
                let length = (tick.length_mm * band.cross_pixels_per_mm)
                    .round()
                    .min((depth - 1) as f64) as i32;
                let stroke = band.scale.round().max(1.) as i32;
                image.strip(band.side, position, position + stroke, length, color);
                if let Some(label) = tick.label {
                    let label = Label::new(
                        &label,
                        (tick.label_mm * band.cross_pixels_per_mm).clamp(7., 56.) as f32,
                    );
                    let (tw, th) = (label.width, label.height);
                    let (x, y) = match band.side {
                        Side::Top => (position + stroke + 1, (length - th).max(1)),
                        Side::Bottom => (position + stroke + 1, (depth - length).max(1)),
                        Side::Left => ((length - tw).max(1), position + stroke + 1),
                        Side::Right => ((depth - length).max(1), position + stroke + 1),
                    };
                    image.text(&label, x, y, color);
                }
            }
        }
        Content::Resistance(sections) => {
            for (index, s) in sections.iter().enumerate() {
                let start = band.pixel_at(s.from()).round().clamp(0., axis as f64) as i32;
                let end = band.pixel_at(s.to()).round().clamp(0., axis as f64) as i32;
                let color = match (s.move_block(), s.drag_block()) {
                    (true, true) => [192, 57, 43, 235],
                    (true, false) => [224, 142, 43, 235],
                    (false, true) => [155, 89, 182, 235],
                    _ if s.move_resistance() > 0. || s.drag() > 0. => [43, 132, 192, 235],
                    _ => [112, 112, 112, 210],
                };
                image.strip(band.side, start, end, depth, color);
                let stroke = (2. * band.scale).round().max(1.) as i32;
                if band.selected == Some(index) {
                    if band.side.vertical() {
                        image.fill(0, start, stroke, end - start, [255, 255, 255, 255]);
                        image.fill(
                            depth - stroke,
                            start,
                            stroke,
                            end - start,
                            [255, 255, 255, 255],
                        );
                    } else {
                        image.fill(start, 0, end - start, stroke, [255, 255, 255, 255]);
                        image.fill(
                            start,
                            depth - stroke,
                            end - start,
                            stroke,
                            [255, 255, 255, 255],
                        );
                    }
                }
                let grip = |boundary: i32| {
                    (6. * band.scale)
                        .max(f64::from(depth) / 2.)
                        .max(f64::from(depth - boundary.min(axis - boundary)))
                        .min(f64::from(end - start) / 3.)
                };
                for (a, b) in [
                    (start, start + grip(start).round() as i32),
                    (end - grip(end).round() as i32, end),
                ] {
                    let (x, y, w, h) = if band.side.vertical() {
                        (0, a, depth, b - a)
                    } else {
                        (a, 0, b - a, depth)
                    };
                    let radius = (2. * band.scale)
                        .min(f64::from(w) / 2.)
                        .min(f64::from(h) / 2.);
                    for row in 0..h {
                        let d =
                            (radius - (f64::from(row) + 0.5).min(f64::from(h - row) - 0.5)).max(0.);
                        let inset =
                            (radius - (radius * radius - d * d).max(0.).sqrt()).round() as i32;
                        image.fill(x + inset, y + row, w - 2 * inset, 1, [255, 255, 255, 208]);
                    }
                }
            }
        }
        Content::Guide { .. } => unreachable!("guides return before band rendering"),
    }
    // Miter the four corners so one edge never paints over its neighbour's marks.
    for y in 0..height {
        for x in 0..width {
            let (along, inward) = match band.side {
                Side::Top => (x, y),
                Side::Bottom => (x, height - 1 - y),
                Side::Left => (y, x),
                Side::Right => (y, width - 1 - x),
            };
            let visible = if band.side.vertical() {
                along > inward && along < axis - 1 - inward
            } else {
                along >= inward && along <= axis - 1 - inward
            };
            if !visible {
                let offset = (y as usize * image.width + x as usize) * 4;
                image.bgra[offset..offset + 4].fill(0);
            }
        }
    }
    Ok(image)
}
