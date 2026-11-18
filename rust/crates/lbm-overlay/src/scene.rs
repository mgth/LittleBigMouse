use lbm_layout::{
    geo::Rect,
    model::{BorderSection, Layout},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Rulers,
    Resistance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Ruler {
        zero_mm: f64,
        measured_mm: f64,
    },
    Resistance(#[serde(with = "sections_wire")] Vec<BorderSection>),
    /// Ratio across the destination panel, independent of its DPI and pixel origin.
    Guide {
        at: f64,
        kind: GuideKind,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuideKind {
    Middle,
    ScreenEdge,
    Section,
}
impl GuideKind {
    pub fn rgba(self) -> [u8; 4] {
        match self {
            Self::Middle => [255, 255, 255, 217],
            Self::ScreenEdge => [0x33, 0xc4, 0xd6, 217],
            Self::Section => [0xe8, 0xc3, 0x3a, 217],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub horizontal: bool,
    pub layout_mm: f64,
    pub kind: GuideKind,
}

/// Thin transparent windows remain inside the output, including a guide on its far edge.
pub(crate) fn guide_rect(screen: Pixels, side: Side, depth: i32, at: f64) -> Pixels {
    let length = if side.vertical() {
        screen.width
    } else {
        screen.height
    };
    let depth = depth.clamp(1, length);
    let start = ((at * f64::from(length)).round() as i32 - depth / 2).clamp(0, length - depth);
    if side.vertical() {
        Pixels {
            x: screen.x + start,
            width: depth,
            ..screen
        }
    } else {
        Pixels {
            y: screen.y + start,
            height: depth,
            ..screen
        }
    }
}

/// Add the active snap reference only when resistance overlays were explicitly enabled.
pub fn with_reference(
    layout: &Layout,
    mode: Mode,
    selected: Option<&str>,
    reference: Option<Reference>,
) -> Result<Vec<Band>, String> {
    let mut bands = from_layout(layout, mode, selected)?;
    let Some(reference) = reference.filter(|_| mode == Mode::Resistance) else {
        return Ok(bands);
    };
    if !reference.layout_mm.is_finite() || reference.layout_mm.abs() > 2_000_000. {
        return Err("Invalid physical reference line.".into());
    }
    let side = if reference.horizontal {
        Side::Top
    } else {
        Side::Left
    };
    let mut guides = Vec::new();
    for band in bands.iter().filter(|b| b.side == side) {
        let monitor = layout
            .monitor(&band.monitor)
            .ok_or("Reference monitor disappeared.")?;
        let bounds = layout
            .depth_projection(monitor)
            .ok_or("No physical reference bounds.")?
            .bounds();
        let (origin, length) = if reference.horizontal {
            (bounds.top(), bounds.height())
        } else {
            (bounds.left(), bounds.width())
        };
        let local = reference.layout_mm - origin;
        if !(0.0..=length).contains(&local) {
            continue;
        }
        let at = local / length;
        let mut guide = band.clone();
        guide.content = Content::Guide {
            at,
            kind: reference.kind,
        };
        guide.rect = guide_rect(band.screen, side, (2. * band.scale).round() as i32, at);
        guides.push(guide);
    }
    bands.extend(guides);
    Ok(bands)
}
/// What a native surface is bound to: one output and one role on it. A surface is only
/// reused for the same binding, so it never moves to another screen, where Windows would
/// give it that screen's DPI and report a display change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    connector: String,
    screen: Pixels,
    side: Side,
    guide: bool,
}
/// For each band, the previous surface with the same binding, each one used at most once.
/// A band without one needs a new surface; previous surfaces left out are released.
pub fn reuse(previous: &[Binding], bands: &[Band]) -> Vec<Option<usize>> {
    let mut taken = vec![false; previous.len()];
    bands
        .iter()
        .map(|band| {
            let binding = band.binding();
            let index = (0..previous.len()).find(|&i| !taken[i] && previous[i] == binding)?;
            taken[index] = true;
            Some(index)
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Band {
    pub monitor: String,
    /// The enumerated output name/interface path, never a model or monitor label.
    pub connector: String,
    pub screen: Pixels,
    pub rect: Pixels,
    pub side: Side,
    pub scale: f64,
    pub axis_mm: f64,
    pub cross_pixels_per_mm: f64,
    pub content: Content,
}
impl Band {
    pub fn binding(&self) -> Binding {
        Binding {
            connector: self.connector.clone(),
            screen: self.screen,
            side: self.side,
            guide: matches!(self.content, Content::Guide { .. }),
        }
    }
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
                Content::Resistance(
                    sections
                        .iter()
                        .map(|s| s.resolved(layout.options.resistance_defaults))
                        .collect(),
                )
            };
            let cross_pixels_per_mm = if side.vertical() {
                screen.width as f64 / bounds.width()
            } else {
                screen.height as f64 / bounds.height()
            };
            bands.push(Band {
                monitor: monitor.id.clone(),
                connector: source
                    .interface_path
                    .clone()
                    .filter(|s| !s.is_empty())
                    .ok_or("An attached screen has no output identity.")?,
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

mod sections_wire {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        sections: &[BorderSection],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        sections
            .iter()
            .map(|v| {
                (
                    v.from(),
                    v.to(),
                    v.move_resistance(),
                    v.move_block(),
                    v.drag(),
                    v.drag_block(),
                )
            })
            .collect::<Vec<_>>()
            .serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<BorderSection>, D::Error> {
        let values = Vec::<(f64, f64, f64, bool, f64, bool)>::deserialize(d)?;
        Ok(values
            .into_iter()
            .map(|(a, b, c, d, e, f)| BorderSection::new(a, b, c, d, e, f))
            .collect())
    }
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
