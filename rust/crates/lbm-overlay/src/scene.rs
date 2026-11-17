use lbm_layout::{
    geo::Rect,
    model::{BorderSection, Layout},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Rulers,
    Resistance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}
impl Side {
    pub const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];
    pub fn vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pixels {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Pixels {
    pub fn checked(r: Rect) -> Result<Self, String> {
        if [
            r.left(),
            r.top(),
            r.width(),
            r.height(),
            r.right(),
            r.bottom(),
        ]
        .iter()
        .any(|v| !v.is_finite() || v.fract() != 0. || *v < i32::MIN as f64 || *v > i32::MAX as f64)
            || r.width() <= 0.
            || r.height() <= 0.
            || r.width() > 32768.
            || r.height() > 32768.
        {
            return Err("No usable physical pixel bounds for this screen.".into());
        }
        Ok(Self {
            x: r.left() as i32,
            y: r.top() as i32,
            width: r.width() as i32,
            height: r.height() as i32,
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Ruler { zero_mm: f64, measured_mm: f64 },
    Resistance(Vec<BorderSection>),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub monitor: String,
    pub screen: Pixels,
    pub rect: Pixels,
    pub side: Side,
    pub scale: f64,
    pub axis_mm: f64,
    pub cross_pixels_per_mm: f64,
    pub content: Content,
}
impl Band {
    pub fn axis_pixels(&self) -> i32 {
        if self.side.vertical() {
            self.rect.height
        } else {
            self.rect.width
        }
    }
    pub fn thickness(&self) -> i32 {
        if self.side.vertical() {
            self.rect.width
        } else {
            self.rect.height
        }
    }
    pub fn pixel_at(&self, mm: f64) -> f64 {
        mm / self.axis_mm * self.axis_pixels() as f64
    }
}
fn physical(r: Rect) -> bool {
    [
        r.left(),
        r.top(),
        r.width(),
        r.height(),
        r.right(),
        r.bottom(),
    ]
    .iter()
    .all(|v| v.is_finite())
        && r.left().abs() <= 1_000_000.
        && r.top().abs() <= 1_000_000.
        && r.width() >= 1.
        && r.height() >= 1.
        && r.width() <= 10000.
        && r.height() <= 10000.
}

/// Rulers project the selected panel's origin/length onto each destination panel.
/// DPI changes band thickness; only the destination's pixel/mm ratio changes tick spacing.
pub fn from_layout(
    layout: &Layout,
    mode: Mode,
    selected: Option<&str>,
) -> Result<Vec<Band>, String> {
    if mode == Mode::Off {
        return Ok(Vec::new());
    }
    if layout.is_virtual() {
        return Err("A foreign layout cannot show physical-screen overlays.".into());
    }
    let measured = if mode == Mode::Rulers {
        let monitor = selected
            .and_then(|id| layout.monitor(id))
            .ok_or("Select a monitor to show its rulers.")?;
        Some(
            layout
                .depth_projection(monitor)
                .ok_or("No measured panel for the selected monitor.")?
                .bounds(),
        )
    } else {
        None
    };
    if measured.is_some_and(|r| !physical(r)) {
        return Err("The selected panel has invalid physical dimensions.".into());
    }
    let mut bands = Vec::new();
    for monitor in layout.monitors() {
        let Some(source) = monitor
            .active_source
            .as_deref()
            .and_then(|id| layout.source(id))
        else {
            continue;
        };
        let source = &source.source;
        if !source.attached_to_desktop {
            continue;
        }
        let screen = Pixels::checked(source.in_pixel.bounds())?;
        // Cloned destinations would stack conflicting drawings at identical pixels.
        if bands.iter().any(|b: &Band| b.screen == screen) {
            return Err(
                "Screen overlays require an extended desktop; cloned screens overlap.".into(),
            );
        }
        let bounds = layout
            .depth_projection(monitor)
            .ok_or("An attached screen has no measured panel.")?
            .bounds();
        if !physical(bounds) {
            return Err("An attached screen has invalid physical dimensions.".into());
        }
        let scale = source.effective_dpi.y / 96.;
        if !scale.is_finite() || !(0.5..=8.).contains(&scale) {
            return Err("An attached screen has invalid DPI information.".into());
        }
        let thickness = ((if mode == Mode::Rulers { 100. } else { 40. }) * scale).round() as i32;
        let thickness = thickness
            .min(screen.width / 3)
            .min(screen.height / 3)
            .max(1);
        for side in Side::ALL {
            let rect = match side {
                Side::Top => Pixels {
                    height: thickness,
                    ..screen
                },
                Side::Bottom => Pixels {
                    y: screen.y + screen.height - thickness,
                    height: thickness,
                    ..screen
                },
                Side::Left => Pixels {
                    width: thickness,
                    ..screen
                },
                Side::Right => Pixels {
                    x: screen.x + screen.width - thickness,
                    width: thickness,
                    ..screen
                },
            };
            let axis_mm = if side.vertical() {
                bounds.height()
            } else {
                bounds.width()
            };
            let content = if let Some(measured) = measured {
                Content::Ruler {
                    zero_mm: if side.vertical() {
                        measured.top() - bounds.top()
                    } else {
                        measured.left() - bounds.left()
                    },
                    measured_mm: if side.vertical() {
                        measured.height()
                    } else {
                        measured.width()
                    },
                }
            } else {
                let r = &monitor.border_resistance;
                let sections = &match side {
                    Side::Top => &r.top,
                    Side::Bottom => &r.bottom,
                    Side::Left => &r.left,
                    Side::Right => &r.right,
                }
                .sections;
                if sections.len() > 16
                    || sections
                        .iter()
                        .any(|s| !s.from().is_finite() || !s.to().is_finite() || s.to() < s.from())
                {
                    return Err("An attached screen contains invalid resistance sections.".into());
                }
                Content::Resistance(sections.clone())
            };
            let cross_pixels_per_mm = if side.vertical() {
                screen.width as f64 / bounds.width()
            } else {
                screen.height as f64 / bounds.height()
            };
            bands.push(Band {
                monitor: monitor.id.clone(),
                screen,
                rect,
                side,
                scale,
                axis_mm,
                cross_pixels_per_mm,
                content,
            });
        }
    }
    if bands.is_empty() {
        return Err("No attached screen to show overlays on.".into());
    }
    if bands.len() > 64 {
        return Err("Too many attached screens for overlays.".into());
    }
    Ok(bands)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub mm: i32,
    pub pixel: f64,
    pub length_mm: f64,
    pub label: Option<String>,
    pub label_mm: f64,
    pub inside: bool,
}
pub fn ticks(band: &Band) -> Vec<Tick> {
    let Content::Ruler {
        zero_mm,
        measured_mm,
    } = band.content
    else {
        return vec![];
    };
    if !zero_mm.is_finite()
        || !band.axis_mm.is_finite()
        || band.axis_mm <= 0.
        || band.axis_mm > 10000.
    {
        return vec![];
    }
    let start = (-zero_mm).floor();
    // Avoid float-to-int saturation or overflow for profiles with unreasonable offsets.
    if start < (i32::MIN + 10001) as f64 || start > (i32::MAX - 10001) as f64 {
        return vec![];
    }
    (start as i32..=start as i32 + band.axis_mm.ceil() as i32 + 1)
        .map(|mm| {
            let (length_mm, label_mm, label) = if mm % 100 == 0 {
                (20., 5., Some((mm / 100).to_string()))
            } else if mm % 50 == 0 {
                (15., 4., Some((mm % 100 / 10).to_string()))
            } else if mm % 10 == 0 {
                (10., 3., Some((mm % 100 / 10).to_string()))
            } else if mm % 5 == 0 {
                (5., 0., None)
            } else {
                (2.5, 0., None)
            };
            Tick {
                mm,
                pixel: band.pixel_at(zero_mm + mm as f64),
                length_mm,
                label_mm,
                label,
                inside: mm >= 0 && (mm as f64) <= measured_mm,
            }
        })
        .collect()
}
