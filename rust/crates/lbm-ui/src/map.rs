//! The map: every screen at one scale, placed inside the window.
//!
//! The map answers two questions and no others: **how big** is a millimetre here, and
//! **where** is the desktop's corner on screen. Everything a monitor draws follows from
//! those two through [`crate::frame`], which is why this module holds no drawing of its
//! own beyond placing frames.
//!
//! Ported from `MultiMonitorsLayoutPresenterView.axaml.cs:58-72` (`GetRatio`), the
//! `Grid Margin="30"` that wraps it, and `MonitorFrameViewModel.cs:73-95`
//! (`left = ratio * (x0 + x - leftBorder)`, which is the outside rect measured from the
//! extent's corner). Three things are deliberately not carried over, each marked below:
//! the empty-extent hole, the uncentred slack, and selection by exact pixel equality.
//!
//! **One ratio, not two.** Avalonia carries a `VisualRatio` with an `X` and a `Y`, and
//! writes the same number into both — `FrameMover.cs:167` says `var ratioY = ratioX;`
//! outright. A map that scaled the axes independently would show a 16:9 screen as 4:3,
//! so the pair is a shape the code never uses, and here the ratio is one number.

use crate::frame::{self, Drawn, Look, Ratio};
use lbm_layout::geo::Rect;

/// The map keeps this much clear of the window's edge, on every side, as the Avalonia
/// `Grid Margin="30"` around the presenter does.
pub const MARGIN: f32 = 30.0;

/// One monitor as the map needs it: what to draw, where, and what to call it.
///
/// Four things, which is the whole of the map's appetite — it does not need the model.
/// `mm_outside` is `DepthProjection.OutsideBounds` (the screen with its bezels) and
/// `mm_content` is `DepthProjection.Bounds` (the lit part).
#[derive(Clone, Copy)]
pub struct MapMonitor<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub mm_outside: Rect,
    pub mm_content: Rect,
    /// The manufacturer logo, resolved and uploaded by whoever has an egui context and
    /// the icon catalogue — the map itself loads nothing. `None` draws no logo.
    pub logo: Option<&'a egui::TextureHandle>,
}

/// How the map sits in the window: one scale, and one corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// Millimetres to points, the same on both axes.
    pub ratio: f64,
    /// The map's own corner in millimetres — the extent's top left, which is negative
    /// as soon as a screen sits left of or above the primary.
    pub origin: (f64, f64),
    /// Where that corner lands in the window.
    pub corner: egui::Pos2,
}

impl Fit {
    /// The per-axis pair [`crate::frame`] takes. Both components are [`Fit::ratio`].
    pub fn scale(&self) -> Ratio {
        Ratio {
            x: self.ratio,
            y: self.ratio,
        }
    }

    /// Where one monitor lands in the window, bezel, lit part and name size.
    pub fn place(&self, m: &MapMonitor) -> Drawn {
        frame::draw(m.mm_outside, m.mm_content, self.origin, self.scale())
            .translate(self.corner.to_vec2())
    }
}

/// The box the map is fitted into: the window less the margin, never inside out.
///
/// Shared with the list mode, which splits this same box in two: one rule for "the
/// window less the margin", in one place, so the two views cannot drift apart on it.
pub(crate) fn inner(available: egui::Rect) -> egui::Rect {
    let shrunk = available.shrink(MARGIN);
    // A window narrower than its own two margins would give a negative box, and a
    // negative box a negative ratio — a map drawn mirrored and off-screen. Avalonia
    // never produces one because a `Grid` clamps its own arrangement at zero; here it
    // has to be said.
    egui::Rect::from_min_size(
        shrunk.min,
        egui::vec2(shrunk.width().max(0.0), shrunk.height().max(0.0)),
    )
}

/// The scale and corner that fit `extent` into `available`.
///
/// `extent` is the layout's own `physical_bounds` for the map, and the selected
/// monitor's outside bounds for the list — the two modes differ by what they fit, not
/// by how, so `MonitorsListPresenterView.axaml.cs:61-75` is this same function with a
/// different argument. Its "nothing selected" case is `Rect::EMPTY`, which lands on the
/// fallback below.
///
/// **The empty-extent hole, not carried over.** The C# guards with
/// `all.Width * all.Height > 0.0`, which is true for `Rect.Empty`: its width and height
/// are both **negative infinity**, so the product is `+∞` and the guard waves it
/// through, giving `min(w / -∞, h / -∞)` — a ratio of minus zero, and a map that
/// vanishes rather than falling back. The rule the guard was reaching for is "an extent
/// I can divide by", and that is what is written here.
pub fn fit(extent: Rect, available: egui::Rect) -> Fit {
    fit_in(inner(available), extent)
}

/// The same, into a box that is already the size it should be.
///
/// `fit` takes a window and keeps the margin clear of its edge; this takes the box
/// itself. The list mode needs it: there the margin goes around the whole view and the
/// two columns divide what is left (`MonitorsListPresenterView.axaml:30`,
/// `Margin="30" ColumnDefinitions="*,2*"`), so the presenter's column has already had
/// its share of the margin and taking another one out would be taking it twice.
pub fn fit_in(inner: egui::Rect, extent: Rect) -> Fit {
    let usable = !extent.is_empty()
        && extent.width().is_finite()
        && extent.height().is_finite()
        && extent.width() > 0.0
        && extent.height() > 0.0;

    let ratio = if usable {
        f64::min(
            inner.width() as f64 / extent.width(),
            inner.height() as f64 / extent.height(),
        )
        .max(0.0)
    } else {
        // What the C# means to do when there is nothing to fit: leave the scale alone.
        1.0
    };

    let origin = if usable {
        (extent.left(), extent.top())
    } else {
        (0.0, 0.0)
    };

    // **The uncentred slack, not carried over.** `Math.Min` letterboxes, and the AXAML
    // asks for the result to be centred (`HorizontalAlignment="Center"`), but the
    // frames live in a `Canvas`, which arranges at `Canvas.Left/Top` and ignores
    // alignment outright — so in the shipped app every bit of slack falls to the right
    // and the bottom. Centring here is honouring the layout the view already declares,
    // not inventing one.
    //
    // The span has to come from the guarded branch too, and not from the extent
    // directly. An unusable extent is `-∞` wide, so `inner.width() - drawn.x` would be
    // `+∞` and the corner infinite — and this is not a case that cannot happen: it is
    // exactly the one above, monitors present and their union come out empty, where
    // every frame would then be placed at infinity instead of merely at the wrong
    // scale.
    let slack = if usable {
        let drawn = egui::vec2(
            (extent.width() * ratio) as f32,
            (extent.height() * ratio) as f32,
        );
        egui::vec2(
            (inner.width() - drawn.x).max(0.0) / 2.0,
            (inner.height() - drawn.y).max(0.0) / 2.0,
        )
    } else {
        // Nothing to centre. Draw from the corner of the box, which at least puts what
        // there is where it can be seen.
        egui::Vec2::ZERO
    };

    Fit {
        ratio,
        origin,
        corner: inner.min + slack,
    }
}

/// Which monitor a point in the window falls on, or `None` for the background.
///
/// Bezel included: the object the user aims at is the screen, not its lit part. Where
/// two frames overlap the first wins, which a placed layout does not produce — screens
/// are solved apart — but a layout mid-drag can.
pub fn hit<'a>(monitors: &[MapMonitor<'a>], fit: &Fit, at: egui::Pos2) -> Option<&'a str> {
    monitors
        .iter()
        .find(|m| fit.place(m).outside.contains(at))
        .map(|m| m.id)
}

/// What a click leaves selected.
///
/// **A click on the background changes nothing**, as in the C#, and for a reason worth
/// keeping: the options panel is a view of the selected monitor, so dropping the
/// selection on a stray click empties the panel the user is working in.
///
/// **Selection by exact pixel equality, not carried over.** `FrameMover.cs:111-116`
/// selects only when the release point equals the press point — `_guiStartPosition == p`
/// on a `Point` of doubles. A tenth of a pixel of drift, which a touchpad produces
/// constantly, turns the click into a zero-length drag and the monitor is never
/// selected. Whether a press and a release are one click is the caller's question here
/// (it owns the gesture); this function only says what the answer means.
pub fn select<'a>(current: Option<&'a str>, clicked: Option<&'a str>) -> Option<&'a str> {
    clicked.or(current)
}

/// Draws the whole map and gives back the monitor the user clicked, if any.
pub fn draw<'a>(
    ui: &mut egui::Ui,
    monitors: &[MapMonitor<'a>],
    fit: &Fit,
    selected: Option<&str>,
) -> Option<&'a str> {
    let mut clicked = None;
    for m in monitors {
        let drawn = fit.place(m);
        // Sensed before it is painted: the rect has to be claimed for the pointer
        // whether or not anything is drawn inside it, and a screen too small to be
        // labelled is still a screen you can click.
        let response = ui.interact(
            drawn.outside,
            ui.id().with(("monitor", m.id)),
            egui::Sense::click(),
        );
        frame::monitor(
            ui,
            &drawn,
            &Look {
                name: m.name,
                selected: selected == Some(m.id),
                logo: m.logo,
            },
        );
        if response.clicked() {
            clicked = Some(m.id);
        }
    }
    clicked
}

/// The union of every monitor's outside bounds — the map's extent, when the caller does
/// not have a layout to ask.
///
/// A layout that has one should pass `physical_bounds()` instead: it is the same union,
/// computed where the model can keep it up to date, and it is what every other consumer
/// of the extent already reads.
pub fn extent(monitors: &[MapMonitor]) -> Rect {
    let mut it = monitors.iter().map(|m| m.mm_outside);
    match it.next() {
        Some(first) => it.fold(first, |acc, r| acc.union(&r)),
        None => Rect::EMPTY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two 620x360 mm screens side by side, the second starting at 700: an extent of
    /// 1320 x 360 with its corner at the origin.
    fn two_screens() -> Vec<MapMonitor<'static>> {
        vec![
            MapMonitor {
                id: "left",
                name: "Left",
                mm_outside: Rect::new(0.0, 0.0, 620.0, 360.0),
                mm_content: Rect::new(10.0, 10.0, 600.0, 340.0),
                logo: None,
            },
            MapMonitor {
                id: "right",
                name: "Right",
                mm_outside: Rect::new(700.0, 0.0, 620.0, 360.0),
                mm_content: Rect::new(710.0, 10.0, 600.0, 340.0),
                logo: None,
            },
        ]
    }

    fn window(w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))
    }

    #[test]
    fn the_whole_desktop_fits_and_touches_the_tighter_side() {
        let all = extent(&two_screens());
        let f = fit(all, window(800.0, 600.0));

        // 740 points of usable width for 1320 mm, 540 for 360: width is the binding
        // constraint, so the map is exactly as wide as the box and shorter than it.
        assert!((f.ratio - 740.0 / 1320.0).abs() < 1e-9);
        let drawn_w = all.width() * f.ratio;
        assert!((drawn_w - 740.0).abs() < 1e-6);
        assert!(all.height() * f.ratio < 540.0);
    }

    #[test]
    fn one_ratio_on_both_axes_so_a_screen_is_never_stretched() {
        let screens = two_screens();
        // A window of a quite different shape from the desktop.
        let f = fit(extent(&screens), window(1600.0, 300.0));
        let drawn = f.place(&screens[0]);

        let mm = screens[0].mm_content;
        assert!(
            (drawn.content.width() as f64 / drawn.content.height() as f64
                - mm.width() / mm.height())
            .abs()
                < 1e-6,
            "the lit part changed shape: {:?}",
            drawn.content
        );
    }

    #[test]
    fn the_slack_is_shared_between_the_two_sides() {
        let screens = two_screens();
        let win = window(800.0, 600.0);
        let f = fit(extent(&screens), win);

        let left = f.place(&screens[0]).outside;
        let right = f.place(&screens[1]).outside;
        let top_gap = left.top() - win.top();
        let bottom_gap = win.bottom() - left.bottom();
        assert!(
            (top_gap - bottom_gap).abs() < 1e-3,
            "letterboxed to one side: {top_gap} above, {bottom_gap} below"
        );
        // And the constrained axis is flush against the margin on both sides.
        assert!((left.left() - (win.left() + MARGIN)).abs() < 1e-3);
        assert!((win.right() - MARGIN - right.right()).abs() < 1e-3);
    }

    #[test]
    fn the_map_corner_is_the_extent_corner_even_when_it_is_negative() {
        // A second screen to the *left* of the primary: the extent starts at -700.
        let screens = vec![
            MapMonitor {
                id: "primary",
                name: "Primary",
                mm_outside: Rect::new(0.0, 0.0, 620.0, 360.0),
                mm_content: Rect::new(10.0, 10.0, 600.0, 340.0),
                logo: None,
            },
            MapMonitor {
                id: "left-of-it",
                name: "Left of it",
                mm_outside: Rect::new(-700.0, 0.0, 620.0, 360.0),
                mm_content: Rect::new(-690.0, 10.0, 600.0, 340.0),
                logo: None,
            },
        ];
        let win = window(800.0, 600.0);
        let f = fit(extent(&screens), win);

        assert_eq!(f.origin, (-700.0, 0.0));
        // The leftmost screen is the one that starts at the margin, not the primary.
        let far = f.place(&screens[1]).outside;
        let primary = f.place(&screens[0]).outside;
        assert!((far.left() - (win.left() + MARGIN)).abs() < 1e-3);
        assert!(primary.left() > far.left());
    }

    /// The C# fallback, and the hole in the guard that guards it.
    #[test]
    fn nothing_to_fit_leaves_the_scale_alone() {
        let win = window(800.0, 600.0);

        // No monitors at all: the union is `Rect::EMPTY`, whose width and height are
        // both -inf. The C# test `Width * Height > 0.0` is `+inf > 0` — true — and the
        // division that follows gives a ratio of zero.
        let empty = extent(&[]);
        assert!(
            empty.width() * empty.height() > 0.0,
            "the C# guard says yes"
        );
        assert_eq!(fit(empty, win).ratio, 1.0, "and it must still fall back");

        // A degenerate extent, which is what `default(Rect)` gives before the first
        // measurement: the C# guard catches this one.
        assert_eq!(fit(Rect::new(0.0, 0.0, 0.0, 0.0), win).ratio, 1.0);
    }

    /// The fallback has to leave a map that can be drawn, not one placed at infinity.
    /// An extent is `Rect::EMPTY` while its monitors still exist — that is what the
    /// union quirk produces from a single unmeasured screen — so the frames are drawn,
    /// and they have to land somewhere on the window.
    #[test]
    fn a_map_with_no_usable_extent_is_still_placed_somewhere_finite() {
        let screens = two_screens();
        let f = fit(Rect::EMPTY, window(800.0, 600.0));

        assert!(
            f.corner.is_finite(),
            "the map's corner is not a place: {:?}",
            f.corner
        );
        let drawn = f.place(&screens[0]);
        assert!(
            drawn.outside.is_finite() && drawn.content.is_finite(),
            "a frame drawn nowhere: {:?}",
            drawn.outside
        );
    }

    #[test]
    fn a_window_smaller_than_its_own_margins_does_not_fold_the_map_inside_out() {
        let screens = two_screens();
        // 40 points across, for 30 of margin on each side.
        let f = fit(extent(&screens), window(40.0, 40.0));

        assert!(f.ratio >= 0.0, "a mirrored map: {}", f.ratio);
        let drawn = f.place(&screens[0]).outside;
        assert!(drawn.width() >= 0.0 && drawn.height() >= 0.0);
    }

    /// The list mode is this same fit with a different extent — one screen instead of
    /// all of them — which is why there is no second function.
    #[test]
    fn the_list_mode_fits_the_one_screen_it_shows() {
        let screens = two_screens();
        let win = window(800.0, 600.0);

        let one = fit(screens[1].mm_outside, win);
        let all = fit(extent(&screens), win);
        assert!(
            one.ratio > all.ratio * 2.0,
            "one screen of a two-screen desktop should be drawn much bigger"
        );
        // Its own corner, so it is drawn at the corner of the box whatever its
        // millimetre position — the expanded frame drops the margin binding entirely.
        assert_eq!(one.origin, (700.0, 0.0));
        assert!((one.place(&screens[1]).outside.left() - (win.left() + MARGIN)).abs() < 1e-3);

        // And nothing selected is `Rect::EMPTY`, the fallback again.
        assert_eq!(fit(Rect::EMPTY, win).ratio, 1.0);
    }

    #[test]
    fn a_click_lands_on_the_screen_under_it_bezel_included() {
        let screens = two_screens();
        let f = fit(extent(&screens), window(800.0, 600.0));

        let right = f.place(&screens[1]).outside;
        assert_eq!(hit(&screens, &f, right.center()), Some("right"));
        // A point on the bezel, inside the outside rect but outside the lit part.
        let bezel = egui::pos2(right.left() + 1.0, right.center().y);
        assert!(!f.place(&screens[1]).content.contains(bezel));
        assert_eq!(hit(&screens, &f, bezel), Some("right"));
    }

    #[test]
    fn the_gap_between_two_screens_is_background() {
        let screens = two_screens();
        let f = fit(extent(&screens), window(800.0, 600.0));

        let left = f.place(&screens[0]).outside;
        let right = f.place(&screens[1]).outside;
        let between = egui::pos2((left.right() + right.left()) / 2.0, left.center().y);
        assert!(between.x > left.right() && between.x < right.left());
        assert_eq!(hit(&screens, &f, between), None);
    }

    #[test]
    fn clicking_the_background_keeps_the_selection() {
        assert_eq!(select(Some("left"), None), Some("left"));
        assert_eq!(select(Some("left"), Some("right")), Some("right"));
        assert_eq!(select(None, Some("right")), Some("right"));
        assert_eq!(select(None, None), None);
    }
}
