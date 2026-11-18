//! Native input uses surface-local UI points, independent of fractional buffer scale.
use crate::scene::{Band, Content, Pixels};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Press,
    Move,
    Release,
    Wheel(f64),
    Escape,
    Delete,
    Cancel,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub band: Band,
    pub kind: Kind,
    pub at: f64,
    pub length: f64,
    pub thickness: f64,
    pub ctrl: bool,
    pub shift: bool,
    pub time: u32,
}
impl Input {
    pub fn new(
        band: &Band,
        kind: Kind,
        point: (f64, f64),
        units: f64,
        time: u32,
        ctrl: bool,
        shift: bool,
    ) -> Self {
        Self {
            band: band.clone(),
            kind,
            at: if band.side.vertical() {
                point.1
            } else {
                point.0
            } / units,
            length: f64::from(band.axis_pixels()) / units,
            thickness: f64::from(band.thickness()) / units,
            ctrl,
            shift,
            time,
        }
    }
    pub fn valid(&self) -> bool {
        [self.at, self.length, self.thickness]
            .iter()
            .all(|n| n.is_finite() && n.abs() <= 1_000_000.)
            && self.length > 0.
            && self.thickness > 0.
            && !matches!(self.kind, Kind::Wheel(d) if !d.is_finite() || d.abs() > 100.)
            && !matches!(self.band.content, Content::Guide { .. })
    }
}
/// Tells a gesture cut short from the capture ending because the button came up.
///
/// Windows reports both as `WM_CAPTURECHANGED`, even when the window released the capture
/// itself. Cancelling on the normal one would forget every click, and so every double click.
#[derive(Clone, Copy, Debug, Default)]
pub struct Capture {
    held: bool,
}
impl Capture {
    pub fn press(&mut self) {
        self.held = true;
    }
    pub fn release(&mut self) {
        self.held = false;
    }
    /// The capture is gone: a cancellation only if the button was still down.
    pub fn lost(&mut self) -> Option<Kind> {
        std::mem::take(&mut self.held).then_some(Kind::Cancel)
    }
}

/// The same half-open diagonals as the pixels: neighbouring edge windows never
/// consume each other's corner. Reference lines have no input region at all.
pub fn regions(band: &Band) -> Vec<Pixels> {
    if matches!(band.content, Content::Guide { .. }) {
        return vec![];
    }
    let mut regions = Vec::new();
    let axis = band.axis_pixels();
    let depth = band.thickness();
    for inward in 0..depth {
        let start = inward + i32::from(band.side.vertical());
        let length = axis - 2 * start;
        if length <= 0 {
            continue;
        }
        use crate::scene::Side;
        regions.push(match band.side {
            Side::Top => Pixels {
                x: start,
                y: inward,
                width: length,
                height: 1,
            },
            Side::Bottom => Pixels {
                x: start,
                y: depth - 1 - inward,
                width: length,
                height: 1,
            },
            Side::Left => Pixels {
                x: inward,
                y: start,
                width: 1,
                height: length,
            },
            Side::Right => Pixels {
                x: depth - 1 - inward,
                y: start,
                width: 1,
                height: length,
            },
        });
    }
    regions
}
