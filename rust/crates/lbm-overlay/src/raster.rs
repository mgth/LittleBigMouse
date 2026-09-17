//! Small, deterministic CPU renderer. BGRA pixels are premultiplied for layered windows.
use crate::scene::{ticks, Band, Content, Side};

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
    fn digits(&mut self, text: &str, x: i32, y: i32, unit: i32, color: [u8; 4]) {
        // A compact measuring-instrument font, identical on every platform and DPI.
        const GLYPHS: [[u8; 7]; 11] = [
            [14, 17, 19, 21, 25, 17, 14],
            [4, 12, 4, 4, 4, 4, 14],
            [14, 17, 1, 2, 4, 8, 31],
            [30, 1, 1, 14, 1, 1, 30],
            [2, 6, 10, 18, 31, 2, 2],
            [31, 16, 16, 30, 1, 1, 30],
            [14, 16, 16, 30, 17, 17, 14],
            [31, 1, 2, 4, 8, 8, 8],
            [14, 17, 17, 14, 17, 17, 14],
            [14, 17, 17, 15, 1, 1, 14],
            [0, 0, 0, 31, 0, 0, 0],
        ];
        for (i, c) in text.bytes().enumerate() {
            let index = if c == b'-' { 10 } else { (c - b'0') as usize };
            for (row, bits) in GLYPHS[index].iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.fill(
                            x + (i as i32 * 6 + col) * unit,
                            y + row as i32 * unit,
                            unit,
                            unit,
                            color,
                        );
                    }
                }
            }
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
                    let unit = ((tick.label_mm * band.cross_pixels_per_mm / 7.).round() as i32)
                        .clamp(1, 8);
                    let tw = label.len() as i32 * 6 * unit;
                    let th = 7 * unit;
                    let (x, y) = match band.side {
                        Side::Top => (position + stroke + 1, (length - th).max(1)),
                        Side::Bottom => (position + stroke + 1, (depth - length).max(1)),
                        Side::Left => ((length - tw).max(1), position + stroke + 1),
                        Side::Right => ((depth - length).max(1), position + stroke + 1),
                    };
                    image.digits(&label, x, y, unit, color);
                }
            }
        }
        Content::Resistance(sections) => {
            for s in sections {
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
                image.strip(band.side, start, start + 1, depth, [240, 245, 248, 255]);
                image.strip(band.side, end - 1, end, depth, [240, 245, 248, 255]);
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
