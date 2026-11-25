//! Rulers on a remote's screens (docs/multi-pc.md, "Rulers on a remote's screens").
//!
//! The master's editor computes, from its layout in millimetres, the bands each screen of
//! a remote gets — where the ruler's zero falls, how long the measured panel is — and in
//! the remote's own desktop pixels, which it knows from the screens the remote reported.
//! The remote checks every band against its screens as they are now: a master working
//! from an older report must not draw a ruler across the wrong place. Only then are they
//! drawn by [`lbm_overlay::Standalone`], on surfaces that exist only while they are
//! shown: passive, or — when the master asks — taking the pointer, what is done on them
//! going back to the master, whose editor acts on it. The remote itself edits nothing.

use std::sync::Arc;

use lbm_ipc::peer::PeerDisplay;
use lbm_overlay::scene::{Band, Content, Pixels};
use lbm_overlay::standalone::{OnError, OnInput};
use serde_json::Value;

/// The most bands a master may ask for: four sides of sixteen screens.
pub const MAX_BANDS: usize = 64;

/// Overlays being shown. Dropping it takes them down.
pub trait Shown: Send {
    /// Shows `bands` instead.
    fn update(&self, bands: Vec<Band>);
}

impl Shown for lbm_overlay::Standalone {
    fn update(&self, bands: Vec<Band>) {
        lbm_overlay::Standalone::update(self, bands)
    }
}

type Paint =
    dyn Fn(Vec<Band>, OnError, Option<OnInput>) -> Result<Box<dyn Shown>, String> + Send + Sync;

/// What draws the overlays a master asks for: the screens themselves, or — in tests — a
/// record of what would have been drawn.
#[derive(Clone)]
pub struct Painter(Arc<Paint>);

impl Painter {
    /// `paint` shows the bands; with an `OnInput`, they take the pointer and every
    /// gesture on them goes to it.
    pub fn new(
        paint: impl Fn(Vec<Band>, OnError, Option<OnInput>) -> Result<Box<dyn Shown>, String>
            + Send
            + Sync
            + 'static,
    ) -> Painter {
        Painter(Arc::new(paint))
    }

    /// This machine's screens.
    pub fn screens() -> Painter {
        Painter::new(|bands, on_error, on_input| {
            match on_input {
                Some(on_input) => lbm_overlay::Standalone::interactive(bands, on_error, on_input),
                None => lbm_overlay::Standalone::show(bands, on_error),
            }
            .map(|shown| Box::new(shown) as Box<dyn Shown>)
        })
    }

    pub(super) fn paint(
        &self,
        bands: Vec<Band>,
        on_error: OnError,
        on_input: Option<OnInput>,
    ) -> Result<Box<dyn Shown>, String> {
        (self.0)(bands, on_error, on_input)
    }
}

impl Default for Painter {
    fn default() -> Painter {
        Painter::screens()
    }
}

impl std::fmt::Debug for Painter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Painter")
    }
}

/// The bands a master sent, if every one of them is a ruler on one of `displays` as this
/// machine has them now — the same output, at the same pixels, at the same scale.
pub fn check(bands: &[Value], displays: &[PeerDisplay]) -> Result<Vec<Band>, String> {
    if bands.len() > MAX_BANDS {
        return Err(format!(
            "{} rulers asked for, at most {MAX_BANDS}",
            bands.len()
        ));
    }
    bands
        .iter()
        .map(|value| {
            let band: Band = serde_json::from_value(value.clone())
                .map_err(|error| format!("a ruler this machine cannot read: {error}"))?;
            check_band(&band, displays)?;
            Ok(band)
        })
        .collect()
}

fn check_band(band: &Band, displays: &[PeerDisplay]) -> Result<(), String> {
    let Content::Ruler {
        zero_mm,
        measured_mm,
    } = band.content
    else {
        return Err("only rulers are drawn for another machine".into());
    };
    let finite = |v: f64, range: std::ops::RangeInclusive<f64>| v.is_finite() && range.contains(&v);
    if !finite(zero_mm, -2_000_000.0..=2_000_000.0)
        || !finite(measured_mm, 1.0..=10_000.0)
        || !finite(band.axis_mm, 1.0..=10_000.0)
        || !finite(band.cross_pixels_per_mm, f64::MIN_POSITIVE..=10_000.0)
        || !finite(band.scale, 0.5..=8.0)
    {
        return Err(format!(
            "the ruler for {} has invalid dimensions",
            band.connector
        ));
    }
    let display = displays
        .iter()
        .find(|d| d.attached && !d.connector.is_empty() && d.connector == band.connector)
        .ok_or_else(|| format!("{} is not a screen of this desktop now", band.connector))?;
    let screen = Pixels {
        x: display.x,
        y: display.y,
        width: display.width,
        height: display.height,
    };
    if band.screen != screen {
        return Err(format!(
            "{} is not where the master thinks it is: the screens changed",
            band.connector
        ));
    }
    let scale = if display.effective_dpi > 0.0 {
        display.effective_dpi / 96.0
    } else {
        display.scale
    };
    if (scale - band.scale).abs() > 1.0 / 120.0 {
        return Err(format!(
            "{} changed scale: the screens changed",
            band.connector
        ));
    }
    let r = band.rect;
    if r.width <= 0
        || r.height <= 0
        || r.x < screen.x
        || r.y < screen.y
        || i64::from(r.x) + i64::from(r.width) > i64::from(screen.x) + i64::from(screen.width)
        || i64::from(r.y) + i64::from(r.height) > i64::from(screen.y) + i64::from(screen.height)
    {
        return Err(format!(
            "the ruler for {} leaves its screen",
            band.connector
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_overlay::scene::Side;

    fn display(connector: &str, x: i32) -> PeerDisplay {
        PeerDisplay {
            id: connector.into(),
            connector: connector.into(),
            name: connector.into(),
            edid: None,
            x,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
            effective_dpi: 96.0,
            orientation: 0,
            frequency: 60.0,
            primary: x == 0,
            attached: true,
            width_mm: 527.0,
            height_mm: 296.0,
            input_source: None,
        }
    }

    fn ruler(connector: &str, x: i32) -> Band {
        let screen = Pixels {
            x,
            y: 0,
            width: 1920,
            height: 1080,
        };
        Band {
            monitor: format!("{connector}@master"),
            connector: connector.into(),
            screen,
            rect: Pixels {
                height: 100,
                ..screen
            },
            side: Side::Top,
            scale: 1.0,
            axis_mm: 527.0,
            cross_pixels_per_mm: 3.6,
            content: Content::Ruler {
                zero_mm: -12.5,
                measured_mm: 600.0,
            },
            selected: None,
            passive: false,
        }
    }

    fn values(bands: &[Band]) -> Vec<Value> {
        bands
            .iter()
            .map(|b| serde_json::to_value(b).unwrap())
            .collect()
    }

    #[test]
    fn rulers_on_this_machines_screens_as_they_are_are_drawn() {
        let displays = [display("DP-1", 0), display("HDMI-1", 1920)];
        let bands = [ruler("DP-1", 0), ruler("HDMI-1", 1920)];
        assert_eq!(check(&values(&bands), &displays).unwrap(), bands);
        assert_eq!(check(&[], &displays).unwrap(), []);
    }

    #[test]
    fn a_ruler_from_an_older_report_of_the_screens_is_refused() {
        let displays = [display("DP-1", 0), display("HDMI-1", 1920)];
        // Moved since the master read it.
        let moved = check(&values(&[ruler("HDMI-1", 2560)]), &displays).unwrap_err();
        assert!(moved.contains("screens changed"), "{moved}");
        // Unplugged, or taken off the desktop.
        assert!(check(&values(&[ruler("DP-2", 0)]), &displays).is_err());
        let mut off = displays.clone();
        off[1].attached = false;
        assert!(check(&values(&[ruler("HDMI-1", 1920)]), &off).is_err());
        // Another scale.
        let mut scaled = displays.clone();
        scaled[0].effective_dpi = 120.0;
        assert!(check(&values(&[ruler("DP-1", 0)]), &scaled).is_err());
    }

    #[test]
    fn only_rulers_inside_their_screen_are_drawn() {
        let displays = [display("DP-1", 0)];
        let mut band = ruler("DP-1", 0);
        band.content = Content::Guide {
            at: 0.5,
            kind: lbm_overlay::scene::GuideKind::Middle,
        };
        assert!(check(&values(&[band]), &displays).is_err());
        let mut band = ruler("DP-1", 0);
        band.rect.y = 1000;
        assert!(check(&values(&[band]), &displays).is_err());
        let mut band = ruler("DP-1", 0);
        band.axis_mm = f64::NAN;
        // NaN is not JSON: what reaches the remote is `null`, which does not read.
        assert!(check(&[serde_json::to_value(&band).unwrap()], &displays).is_err());
        assert!(check(&[serde_json::json!({"Nonsense": 1})], &displays).is_err());
        let many: Vec<Band> = (0..=MAX_BANDS).map(|_| ruler("DP-1", 0)).collect();
        assert!(check(&values(&many), &displays).is_err());
    }
}
