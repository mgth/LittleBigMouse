//! Dragging one screen across the map: where it lands, and what it lines up with.
//!
//! Ported from `FrameMover.cs`, which is the whole of the gesture in the Avalonia app.
//! Two halves, and they are worth keeping apart:
//!
//! * **The snap** ([`snap`]) — five anchors per axis on every screen, and the smallest
//!   offset that puts one of the dragged screen's onto one of another's. Millimetres in,
//!   millimetres out; no window, no pointer, no widget state.
//! * **The guides** ([`draw`]) — the lines that say *why* it snapped there. The user is
//!   moving a screen by its picture, so the rule that caught it has to be visible or the
//!   jump looks like the map fighting back.
//!
//! * **The drop** ([`drop_screen`]) — the only part that writes anything down. It has a
//!   branch in it that is easy to get silently wrong (the primary does not move;
//!   everything else does) and it ends in a compaction that can shift screens the user
//!   never touched, so it is here, next to a test, rather than inline in a window.
//!
//! **The map does not rescale during a drag.** `FrameMover.cs:166` asks the presenter for
//! the ratio on every move, and `MultiMonitorsLayoutPresenterView.axaml.cs:58-72` computes
//! it from `Layout.PhysicalBounds` — the *published* extent, which the layout only
//! republishes when a location is written, and no location is written until the drop. So
//! the scale and the corner the caller passes in are the ones from before the gesture
//! started, and a screen dragged past the edge of the desktop goes off the map rather
//! than shrinking it.

use crate::map::{Fit, MapMonitor};
use lbm_layout::geo::{Point, Rect, Vector};
use lbm_layout::model::Layout;

/// Beyond this, in millimetres, the snap lets go: `FrameMover.cs:164`,
/// `const double maxSnapDistance = 10.0`. Compared with `>`, so exactly ten still snaps.
pub const MAX_SNAP: f64 = 10.0;

/// Two candidate offsets this close together are the same offset, and both their lines
/// are drawn: `FrameMover.cs:194` and `:217`, `Math.Abs(offset - snapOffset) < 0.01`.
///
/// This is what shows a whole column of screens lining up at once instead of only the
/// last one considered.
const SAME_OFFSET: f64 = 0.01;

/// Which edge of a screen an anchor is, which is how its line is drawn.
///
/// The three come with their own brush and dash pattern in
/// `MonitorLocationView.axaml.cs:42-61` — the code that makes the anchors is the code
/// that says what they look like, so the pair travels together here too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The outer edge of the bezel — the object's real extent.
    Outside,
    /// The edge of the lit part, which is what the layout is about.
    Inside,
    /// The middle of the lit part: two screens centred on each other.
    Middle,
}

/// One position a screen offers to line up on, on one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub at: f64,
    pub kind: Kind,
}

/// One screen as the drag sees it: the bezel and the lit part, in millimetres.
///
/// Deliberately less than [`MapMonitor`] — the snap has no use for a name, a logo or a
/// texture, and a function that took the whole of one would be claiming otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    /// `DepthProjection.OutsideBounds`.
    pub outside: Rect,
    /// `DepthProjection.Bounds`.
    pub content: Rect,
}

impl Screen {
    /// The same screen, moved — the dragged one at the pointer, before any snap.
    pub fn moved(self, by: (f64, f64)) -> Screen {
        let v = Vector::new(by.0, by.1);
        Screen {
            outside: self.outside.translate(v),
            content: self.content.translate(v),
        }
    }
}

impl From<&MapMonitor<'_>> for Screen {
    fn from(m: &MapMonitor<'_>) -> Screen {
        Screen {
            outside: m.mm_outside,
            content: m.mm_content,
        }
    }
}

/// The five vertical lines a screen offers — so, five positions on the x axis.
///
/// `FrameMover.cs:71-82` (`VerticalAnchors`), in its order: the outside edge, the lit
/// edge, the middle of the lit part, the far lit edge, the far outside edge. The middle
/// is the middle of the **lit part** and not of the object — `s.X + s.Width / 2`, where
/// `Width` is `Bounds`' — so two screens with different bezels centre on their pictures.
pub fn vertical_anchors(screen: Screen) -> [Anchor; 5] {
    let (outside, content) = (screen.outside, screen.content);
    [
        Anchor {
            at: outside.left(),
            kind: Kind::Outside,
        },
        Anchor {
            at: content.left(),
            kind: Kind::Inside,
        },
        Anchor {
            at: content.left() + content.width() / 2.0,
            kind: Kind::Middle,
        },
        Anchor {
            at: content.right(),
            kind: Kind::Inside,
        },
        Anchor {
            at: outside.right(),
            kind: Kind::Outside,
        },
    ]
}

/// The five horizontal lines, so five positions on the y axis:
/// `FrameMover.cs:84-95` (`HorizontalAnchors`), the same five turned a quarter.
pub fn horizontal_anchors(screen: Screen) -> [Anchor; 5] {
    let (outside, content) = (screen.outside, screen.content);
    [
        Anchor {
            at: outside.top(),
            kind: Kind::Outside,
        },
        Anchor {
            at: content.top(),
            kind: Kind::Inside,
        },
        Anchor {
            at: content.top() + content.height() / 2.0,
            kind: Kind::Middle,
        },
        Anchor {
            at: content.bottom(),
            kind: Kind::Inside,
        },
        Anchor {
            at: outside.bottom(),
            kind: Kind::Outside,
        },
    ]
}

/// One line to draw, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub kind: Kind,
    /// Where the line sits on its own axis — after the snap, so the pair of lines a match
    /// produces are at the same place.
    pub at: f64,
    /// The other axis: the line is drawn across the screen it belongs to and no further
    /// (`FrameMover.cs:279-280`, `:315-316`, which run from that monitor's `OutsideY` to
    /// its `OutsideBounds.Bottom`). A guide across the whole window would say the rule
    /// holds everywhere; it holds between two screens.
    pub across: (f64, f64),
    /// Whether it belongs to the screen being dragged, which is drawn stronger.
    pub dragged: bool,
}

/// What the snap decided.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    /// Millimetres to add to the free drag offset. Zero on an axis that found nothing
    /// within [`MAX_SNAP`].
    pub offset: (f64, f64),
    /// Vertical lines — the x axis.
    pub vertical: Vec<Guide>,
    /// Horizontal lines — the y axis.
    pub horizontal: Vec<Guide>,
}

/// Where the drag really puts the screen, and why.
///
/// `free` is the pointer's own offset in millimetres (the gesture in points divided by
/// the map's ratio, `FrameMover.cs:172`). `others` is every screen but the dragged one;
/// with none the offset is `free` untouched, which falls out of the algorithm rather than
/// being a case: the best offset starts at infinity and infinity is further than
/// [`MAX_SNAP`].
///
/// The two axes are decided independently — a screen can catch a neighbour's left edge
/// while staying free vertically — which is `FrameMover.cs:237-247`, two separate tests.
pub fn snap(dragged: Screen, free: (f64, f64), others: &[Screen]) -> Snap {
    let moved = dragged.moved(free);
    let mut offset = (f64::INFINITY, f64::INFINITY);
    let mut vertical: Vec<Guide> = Vec::new();
    let mut horizontal: Vec<Guide> = Vec::new();

    for other in others {
        consider(
            &mut offset.0,
            &mut vertical,
            vertical_anchors(moved),
            vertical_anchors(*other),
            (moved.outside.top(), moved.outside.bottom()),
            (other.outside.top(), other.outside.bottom()),
        );
        consider(
            &mut offset.1,
            &mut horizontal,
            horizontal_anchors(moved),
            horizontal_anchors(*other),
            (moved.outside.left(), moved.outside.right()),
            (other.outside.left(), other.outside.right()),
        );
    }

    // `FrameMover.cs:237-247`: too far is not a weak snap, it is no snap — the lines go
    // too, or the map would show a rule it did not apply. The infinity a desktop of one
    // screen leaves behind comes out here, and so would a NaN from a degenerate rectangle.
    if !offset.0.is_finite() || offset.0.abs() > MAX_SNAP {
        vertical.clear();
        offset.0 = 0.0;
    }
    if !offset.1.is_finite() || offset.1.abs() > MAX_SNAP {
        horizontal.clear();
        offset.1 = 0.0;
    }

    // The dragged screen's own lines were measured across it at the *free* position,
    // because the snap on the other axis was not known yet. It is now, and it moves them:
    // a vertical line runs down the frame, so it follows the frame's vertical snap.
    for guide in &mut vertical {
        if guide.dragged {
            guide.across = (guide.across.0 + offset.1, guide.across.1 + offset.1);
        }
    }
    for guide in &mut horizontal {
        if guide.dragged {
            guide.across = (guide.across.0 + offset.0, guide.across.1 + offset.0);
        }
    }

    Snap {
        offset,
        vertical,
        horizontal,
    }
}

/// One axis of the search: `FrameMover.cs:187-232`, which is these twenty lines written
/// twice, once per axis.
///
/// Every pair of anchors proposes the offset that would make them meet. The smallest wins;
/// an offset equal to the winner joins it rather than replacing it, so two screens that
/// line up on the same rule both show their line.
fn consider(
    best: &mut f64,
    guides: &mut Vec<Guide>,
    mine: [Anchor; 5],
    theirs: [Anchor; 5],
    my_span: (f64, f64),
    their_span: (f64, f64),
) {
    for a in mine {
        for b in theirs {
            let offset = b.at - a.at;
            let same = (offset - *best).abs() < SAME_OFFSET;
            if !same {
                if offset.abs() >= best.abs() {
                    continue;
                }
                guides.clear();
            }
            *best = offset;
            // Both lines land on the other screen's anchor, because that is where the
            // dragged one is about to be. The dragged screen keeps its *own* kind — it is
            // its edge that is being matched, and the line says which one.
            guides.push(Guide {
                kind: b.kind,
                at: b.at,
                across: their_span,
                dragged: false,
            });
            guides.push(Guide {
                kind: a.kind,
                at: b.at,
                across: my_span,
                dragged: true,
            });
        }
    }
}

/// Where a dropped screen really ends up: `FrameMover.cs:118-146` (`EndMove`).
///
/// `by` is the whole gesture in millimetres, snap included — the offset from where the
/// screen was when the user took hold of it.
///
/// The branch in the middle is the whole of it. **The primary is the origin** of the
/// millimetre space, so it cannot be moved within it: dragging the primary moves every
/// *other* screen the opposite way, which looks the same on screen and leaves the space
/// anchored where the rest of the product expects it. The C# uses the drag's delta rather
/// than the primary's absolute position for this, deliberately and with a comment saying
/// why (`:126-127`), so that it stays right if the primary is not exactly at the origin —
/// and so does this.
///
/// Then a compaction, because a drop can leave a gap and the layout does not allow one
/// unless its options say so. Compaction can move **any** screen, including ones the user
/// never touched, so a caller must read the whole layout back afterwards rather than
/// assume it knows where the dropped screen went.
pub fn drop_screen(layout: &mut Layout, id: &str, by: (f64, f64)) {
    let primary = layout.primary_monitor().map(|m| m.id.clone());
    if primary.as_deref() == Some(id) {
        let others: Vec<String> = layout
            .monitors()
            .iter()
            .map(|m| m.id.clone())
            .filter(|other| other != id)
            .collect();
        for other in others {
            layout.offset_location(&other, Vector::new(-by.0, -by.1));
        }
    } else {
        let Some(from) = layout.monitor(id).map(|m| m.location()) else {
            return;
        };
        layout.set_location(id, Point::new(from.x + by.0, from.y + by.1));
    }
    // `_layout.Compact()`. `UpdatePhysicalMonitors()` follows it in the C#; here every
    // write above republishes the extent by itself when it moves something
    // (`Layout::republish_if_moved`), so saying it again would say nothing.
    layout.compact();
}

/// `Brushes.Chartreuse`, `Colors.LightGreen` and `Brushes.Red`, as
/// `MonitorLocationView.axaml.cs:52-59` names them.
pub fn colour(kind: Kind) -> egui::Color32 {
    match kind {
        Kind::Outside => egui::Color32::from_rgb(127, 255, 0),
        Kind::Inside => egui::Color32::from_rgb(144, 238, 144),
        Kind::Middle => egui::Color32::from_rgb(255, 0, 0),
    }
}

/// The dash pattern, as `(dashes, gaps)` for `Shape::dashed_line_with_offset`. Empty
/// dashes mean a solid line.
///
/// `MonitorLocationView.axaml.cs:44-46`: `null` inside, `{25, 2}` outside,
/// `{20, 7, 2, 7}` in the middle — a long dash, a gap, a dot, a gap, which is why the
/// middle one needs a pattern and not a single length. Avalonia counts a dash array in
/// multiples of the stroke thickness; the source never sets one (see [`THICKNESS`]), so
/// there is no multiplier to carry over and these are points.
fn dashes(kind: Kind) -> (&'static [f32], &'static [f32]) {
    match kind {
        Kind::Outside => (&[25.0], &[2.0]),
        Kind::Inside => (&[], &[]),
        Kind::Middle => (&[20.0, 2.0], &[7.0, 7.0]),
    }
}

/// How thick a guide is drawn: the dragged screen's own lines, then everyone else's.
///
/// **Chosen here, not ported.** `FrameMover.cs:257` computes `var t = ... ? 5 : 2` and
/// then never reads it, and both `StrokeThickness` assignments below it are commented out
/// (`:281`, `:300`) — so the distinction is clearly intended but its numbers were never
/// on screen to be judged. Five points of chartreuse would be a stripe across the frame;
/// these keep the dragged line the louder of the two without painting over the picture.
pub const THICKNESS: (f32, f32) = (2.0, 1.0);

/// How much of the other screens' lines shows: `Opacity = 0.6` (`FrameMover.cs:284`,
/// `:319`). The dragged screen's are drawn full strength.
const OTHER_OPACITY: f32 = 0.6;

/// Draws the guides the snap asked for.
///
/// After the frames, so they are read over the screens they are about — the C# adds its
/// canvas to the same panel the frames are in, which puts it on top.
pub fn draw(ui: &egui::Ui, fit: &Fit, snap: &Snap) {
    let painter = ui.painter();
    let mut shapes = Vec::new();
    for guide in &snap.vertical {
        let x = fit.x(guide.at);
        line(
            &mut shapes,
            guide,
            egui::pos2(x, fit.y(guide.across.0)),
            egui::pos2(x, fit.y(guide.across.1)),
        );
    }
    for guide in &snap.horizontal {
        let y = fit.y(guide.at);
        line(
            &mut shapes,
            guide,
            egui::pos2(fit.x(guide.across.0), y),
            egui::pos2(fit.x(guide.across.1), y),
        );
    }
    painter.extend(shapes);
}

fn line(shapes: &mut Vec<egui::Shape>, guide: &Guide, from: egui::Pos2, to: egui::Pos2) {
    let (thick, opacity) = if guide.dragged {
        (THICKNESS.0, 1.0)
    } else {
        (THICKNESS.1, OTHER_OPACITY)
    };
    let stroke = egui::Stroke::new(thick, colour(guide.kind).gamma_multiply(opacity));
    let (dash, gap) = dashes(guide.kind);
    if dash.is_empty() {
        shapes.push(egui::Shape::line_segment([from, to], stroke));
    } else {
        egui::Shape::dashed_line_many_with_offset(&[from, to], stroke, dash, gap, 0.0, shapes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A screen 620 mm wide over a 600 mm picture: 10 mm of bezel all round.
    fn screen(x: f64, y: f64) -> Screen {
        Screen {
            outside: Rect::new(x, y, 620.0, 360.0),
            content: Rect::new(x + 10.0, y + 10.0, 600.0, 340.0),
        }
    }

    #[test]
    fn the_five_anchors_are_the_bezel_the_picture_and_its_middle() {
        let s = screen(0.0, 0.0);
        let x: Vec<f64> = vertical_anchors(s).iter().map(|a| a.at).collect();
        assert_eq!(x, vec![0.0, 10.0, 310.0, 610.0, 620.0]);
        let kinds: Vec<Kind> = vertical_anchors(s).iter().map(|a| a.kind).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::Outside,
                Kind::Inside,
                Kind::Middle,
                Kind::Inside,
                Kind::Outside
            ]
        );

        let y: Vec<f64> = horizontal_anchors(s).iter().map(|a| a.at).collect();
        assert_eq!(y, vec![0.0, 10.0, 180.0, 350.0, 360.0]);
    }

    /// The middle anchor is the middle of the **picture**, not of the object: a screen
    /// with a fat bezel on one side only would centre differently under the other rule.
    #[test]
    fn the_middle_is_the_middle_of_the_lit_part() {
        let lopsided = Screen {
            outside: Rect::new(0.0, 0.0, 700.0, 360.0),
            content: Rect::new(80.0, 10.0, 600.0, 340.0),
        };
        let middle = vertical_anchors(lopsided)[2];
        assert_eq!(middle.at, 380.0);
        assert_ne!(middle.at, lopsided.outside.left() + 350.0);
    }

    #[test]
    fn a_screen_let_go_near_another_edge_lands_on_it() {
        let dragged = screen(0.0, 0.0);
        let other = screen(700.0, 0.0);
        // Dropped so its right bezel is 3 mm short of the other's left bezel.
        let free = (77.0, 0.0);
        let s = snap(dragged, free, &[other]);

        assert!((s.offset.0 - 3.0).abs() < 1e-9, "{:?}", s.offset);
        assert_eq!(s.offset.1, 0.0, "already level, so nothing to correct");
        // 620 + 77 + 3 == 700: the bezels touch.
        assert_eq!(
            dragged.moved((free.0 + s.offset.0, 0.0)).outside.right(),
            700.0
        );
    }

    #[test]
    fn further_than_ten_millimetres_is_not_a_snap_at_all() {
        // Well below as well as to the left, so neither axis has anything in reach.
        let s = snap(screen(0.0, 0.0), (69.0, 0.0), &[screen(700.0, 500.0)]);
        // 11 mm short of touching, and nothing else within ten either.
        assert_eq!(s.offset, (0.0, 0.0));
        assert!(
            s.vertical.is_empty() && s.horizontal.is_empty(),
            "a line was drawn for a rule that was not applied: {s:?}"
        );
    }

    /// Exactly ten still snaps: the C# compares with `>`.
    #[test]
    fn ten_millimetres_is_still_within_reach() {
        let s = snap(screen(0.0, 0.0), (70.0, 0.0), &[screen(700.0, 0.0)]);
        assert!((s.offset.0 - 10.0).abs() < 1e-9, "{:?}", s.offset);
    }

    /// **Anchors of different kinds match each other**, and that is not an accident of
    /// the port: `FrameMover.cs:187-190` pairs all five of one screen with all five of
    /// the other, so a bezel edge can land on a neighbour's picture edge. It is what lets
    /// a screen be lined up by its plastic against another's image, and it means a screen
    /// nudged by less than a bezel's width has several rules in reach at once.
    #[test]
    fn a_bezel_edge_can_land_on_another_screens_picture_edge() {
        let dragged = screen(0.0, 0.0);
        let other = screen(700.0, 0.0);
        // Bezel right at 708, two short of the other's picture at 710 — and picture right
        // at 698, two short of the other's bezel at 700. Both rules, both cross-kind.
        let s = snap(dragged, (88.0, 0.0), &[other]);

        assert!((s.offset.0 - 2.0).abs() < 1e-9, "{:?}", s.offset);
        let matched: Vec<(Kind, bool)> = s.vertical.iter().map(|g| (g.kind, g.dragged)).collect();
        assert!(
            matched.contains(&(Kind::Inside, true)) && matched.contains(&(Kind::Outside, false)),
            "the picture edge did not meet the bezel: {matched:?}"
        );
    }

    #[test]
    fn the_two_axes_are_decided_on_their_own() {
        // Close enough to catch horizontally, nowhere near vertically.
        let s = snap(screen(0.0, 0.0), (78.0, 200.0), &[screen(700.0, 0.0)]);
        assert!((s.offset.0 - 2.0).abs() < 1e-9);
        assert_eq!(s.offset.1, 0.0);
        assert!(!s.vertical.is_empty());
        assert!(s.horizontal.is_empty());
    }

    #[test]
    fn with_nothing_else_on_the_desktop_the_screen_goes_where_it_is_put() {
        let s = snap(screen(0.0, 0.0), (123.456, -78.9), &[]);
        assert_eq!(s.offset, (0.0, 0.0));
        assert!(s.vertical.is_empty() && s.horizontal.is_empty());
    }

    /// A match draws two lines at the same place: the other screen's, across the other
    /// screen, and the dragged one's, across itself.
    #[test]
    fn a_match_draws_a_line_on_each_screen_at_the_same_place() {
        let dragged = screen(0.0, 0.0);
        let other = screen(700.0, 0.0);
        let s = snap(dragged, (77.0, 0.0), &[other]);

        assert_eq!(s.vertical.len(), 2, "{:?}", s.vertical);
        let (mine, theirs): (Vec<&Guide>, Vec<&Guide>) = s.vertical.iter().partition(|g| g.dragged);
        assert_eq!(mine.len(), 1);
        assert_eq!(theirs.len(), 1);
        assert_eq!(mine[0].at, theirs[0].at, "the lines must coincide");
        assert_eq!(mine[0].at, 700.0);
        assert_eq!(theirs[0].across, (0.0, 360.0), "across the other screen");
        assert_eq!(mine[0].across, (0.0, 360.0), "across the dragged one");
    }

    /// The dragged screen's vertical line runs down the frame, so it has to follow the
    /// frame's *vertical* snap — which is only known after both axes are settled.
    #[test]
    fn the_dragged_screens_line_follows_it_on_the_other_axis() {
        let dragged = screen(0.0, 0.0);
        let other = screen(700.0, 400.0);
        // Right bezel 3 mm short of the other's left, and top 4 mm short of its top.
        let s = snap(dragged, (77.0, 396.0), &[other]);
        assert!((s.offset.0 - 3.0).abs() < 1e-9);
        assert!((s.offset.1 - 4.0).abs() < 1e-9);

        let mine = s.vertical.iter().find(|g| g.dragged).expect("a line");
        // 0 + 396 + 4 = 400, the snapped top, not 396.
        assert_eq!(mine.across, (400.0, 760.0));
    }

    /// Two screens stacked in a column, both sharing one left edge with the dragged one:
    /// **both** show their lines, not only the last one considered. This is what
    /// `SAME_OFFSET` buys, and it is how the map says "this column lines up", not "this
    /// screen does".
    #[test]
    fn every_screen_that_lines_up_shows_its_line() {
        let dragged = screen(0.0, 900.0);
        let column = [screen(700.0, 0.0), screen(700.0, 400.0)];
        // Left bezel 2 mm short of the column's left bezel — and, the screens being
        // identical, every one of the five rules is 2 mm out at the same time.
        let s = snap(dragged, (698.0, 0.0), &column);

        assert!((s.offset.0 - 2.0).abs() < 1e-9);
        let theirs: Vec<&Guide> = s.vertical.iter().filter(|g| !g.dragged).collect();
        // Each screen is drawn across itself, not across the pair, and both are there.
        let spans: Vec<(f64, f64)> = theirs.iter().map(|g| g.across).collect();
        assert!(
            spans.contains(&(0.0, 360.0)) && spans.contains(&(400.0, 760.0)),
            "one of the two screens claimed the rule alone: {theirs:?}"
        );
    }

    /// Centring is a rule of its own: a small screen under a wide one, nothing touching
    /// and no edge within reach of another, and it still catches on the middles.
    #[test]
    fn a_screen_can_snap_on_the_middle_alone() {
        let dragged = screen(0.0, 0.0);
        let wide = Screen {
            outside: Rect::new(0.0, 1000.0, 1200.0, 400.0),
            content: Rect::new(20.0, 1020.0, 1160.0, 360.0),
        };
        // The wide screen's picture is centred on 600; put the dragged one's middle
        // (310 mm into it) four short of that. Every edge pair is hundreds out.
        let s = snap(dragged, (286.0, 0.0), &[wide]);

        assert!((s.offset.0 - 4.0).abs() < 1e-9, "{:?}", s.offset);
        let kinds: Vec<Kind> = s.vertical.iter().map(|g| g.kind).collect();
        assert_eq!(
            kinds,
            vec![Kind::Middle, Kind::Middle],
            "some other rule caught it: {:?}",
            s.vertical
        );
    }
}
