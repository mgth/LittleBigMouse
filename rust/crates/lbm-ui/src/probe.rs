//! The prober's strips: where the cursor leaves a screen, and where it cannot.
//!
//! The agent walks its own engine along every edge of every zone and reports, run of
//! pixels by run of pixels, whether the cursor hits a wall there or crosses into another
//! screen. The map draws that as coloured strips inside each frame's lit part, flush
//! against the bezel — red for a wall, green for a crossing, and the tooltip says where
//! to. It is the one view in the product that shows what the engine will actually *do*,
//! as opposed to what the layout says.
//!
//! Ported from `MonitorFrameViewModel.GetProbeStrips` (`:280-329`) and
//! `ProbeStripViewModel:17-20`. The arithmetic is the interesting part: the runs are in
//! **desktop pixels along the edge**, and the strips are in the frame's points, so each
//! run is normalised against the source's own pixel rectangle and stretched back over
//! the lit part.
//!
//! The types here are this crate's own rather than `lbm_engine::probe`'s. Nothing is
//! shared today in any case — the agent's API hands the report over as an XML document
//! in a string (`api.rs`, `"Payload": "<ProbeReport/>"`), a shape inherited from the C#
//! frontend — so something has to parse into something. Where that parser lives, and
//! whether the agent should grow a JSON shape beside the XML for its Rust frontend, is
//! not settled here.

use lbm_layout::geo::Rect;

/// Which edge of a screen a run belongs to.
///
/// An enum where the C# matches on the string and lets **anything unrecognised fall
/// through to Bottom** (`_ =>` in the switch at `:329`). Unreachable today — the
/// producer writes exactly these four (`lbm-engine/src/probe.rs:139-166`) — but a
/// mislabelled edge drawn as a bottom strip is wrong information rather than none, and
/// there is no reason to be able to express it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The side runs along the screen's height rather than its width.
    fn vertical(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    /// As the report spells it, and as the tooltip says it.
    pub fn name(self) -> &'static str {
        match self {
            Side::Left => "Left",
            Side::Right => "Right",
            Side::Top => "Top",
            Side::Bottom => "Bottom",
        }
    }
}

/// One homogeneous stretch of an edge, in desktop pixels, `from..=to` **inclusive**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub from: i32,
    pub to: i32,
    /// The zone the cursor crosses into, or `None` for a wall — the report's
    /// `TargetId < 0`.
    pub target: Option<i32>,
}

/// One edge of one screen, and what the engine does along it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub side: Side,
    pub runs: Vec<Run>,
}

/// A strip to draw, and what it says when hovered.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub rect: egui::Rect,
    pub wall: bool,
    pub tip: String,
}

/// How thick a strip is drawn, in points.
///
/// Not scaled by the map's ratio, in the C# and here: it is a mark on the drawing, not a
/// measurement of the desktop. A strip that shrank with the map would vanish on the
/// screens that need it most.
pub const THICKNESS: f32 = 5.0;

/// A wall: the cursor stops. `#E5484D` at 0.9, from `ProbeStripViewModel:17-20`.
pub const WALL: egui::Color32 = egui::Color32::from_rgba_premultiplied(0xCE, 0x41, 0x45, 0xE6);

/// A crossing: the cursor passes into another screen. `#46A758` at 0.7.
pub const CROSSING: egui::Color32 = egui::Color32::from_rgba_premultiplied(0x31, 0x75, 0x3E, 0xB3);

/// The strips for one screen.
///
/// `content` is the lit part on the map, in points; `pixels` is the source's own
/// rectangle in desktop pixels, which is the space the runs are measured in; `name_of`
/// turns a target zone id into something to say, as the C#'s `TargetName` does (the
/// zone's name, else its device id, else `zone {id}`).
///
/// Empty when the source has no pixels to speak of — the C# guards on a rectangle at
/// least one pixel each way, and a zero span would divide by nothing.
pub fn strips(
    content: egui::Rect,
    pixels: Rect,
    edges: &[Edge],
    name_of: impl Fn(i32) -> String,
) -> Vec<Strip> {
    if pixels.width() < 1.0 || pixels.height() < 1.0 {
        return Vec::new();
    }
    let mut strips = Vec::new();
    for edge in edges {
        let vertical = edge.side.vertical();
        let span = if vertical {
            pixels.height()
        } else {
            pixels.width()
        };
        let origin = if vertical {
            pixels.top()
        } else {
            pixels.left()
        };

        for run in &edge.runs {
            let at = |p: f64| ((p - origin) / span).clamp(0.0, 1.0) as f32;
            let from = at(run.from as f64);
            // `+ 1` because a run names its last pixel, not the edge past it: a run of
            // the single pixel 0 covers the width of one pixel, not nothing.
            let to = at(run.to as f64 + 1.0);
            if to <= from {
                continue;
            }

            let w = content.width();
            let h = content.height();
            let rect = match edge.side {
                Side::Left => egui::Rect::from_min_size(
                    egui::pos2(content.left(), content.top() + from * h),
                    egui::vec2(THICKNESS, (to - from) * h),
                ),
                Side::Right => egui::Rect::from_min_size(
                    egui::pos2(content.right() - THICKNESS, content.top() + from * h),
                    egui::vec2(THICKNESS, (to - from) * h),
                ),
                Side::Top => egui::Rect::from_min_size(
                    egui::pos2(content.left() + from * w, content.top()),
                    egui::vec2((to - from) * w, THICKNESS),
                ),
                Side::Bottom => egui::Rect::from_min_size(
                    egui::pos2(content.left() + from * w, content.bottom() - THICKNESS),
                    egui::vec2((to - from) * w, THICKNESS),
                ),
            };

            let side = edge.side.name();
            strips.push(Strip {
                rect,
                wall: run.target.is_none(),
                tip: match run.target {
                    None => format!("{side}: wall — the cursor stops here"),
                    Some(id) => format!("{side}: crosses into {}", name_of(id)),
                },
            });
        }
    }
    strips
}

/// Draws the strips and offers their tooltips.
pub fn draw(ui: &mut egui::Ui, strips: &[Strip]) {
    for strip in strips {
        ui.painter()
            .rect_filled(strip.rect, 1.0, if strip.wall { WALL } else { CROSSING });
        // Sensed for hovering only: the strips sit over the lit part, and the frame
        // underneath is what answers a click.
        ui.interact(
            strip.rect,
            ui.id()
                .with(("strip", strip.rect.min.x as i32, strip.rect.min.y as i32)),
            egui::Sense::hover(),
        )
        .on_hover_text(&strip.tip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1920x1080 source at the desktop's origin, drawn as a 400x225 lit part.
    fn one() -> (egui::Rect, Rect) {
        (
            egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(400.0, 225.0)),
            Rect::new(0.0, 0.0, 1920.0, 1080.0),
        )
    }

    fn edge(side: Side, runs: &[Run]) -> Vec<Edge> {
        vec![Edge {
            side,
            runs: runs.to_vec(),
        }]
    }

    fn wall(from: i32, to: i32) -> Run {
        Run {
            from,
            to,
            target: None,
        }
    }

    fn crosses(from: i32, to: i32, into: i32) -> Run {
        Run {
            from,
            to,
            target: Some(into),
        }
    }

    #[test]
    fn a_run_down_the_whole_edge_covers_the_whole_side() {
        let (content, pixels) = one();
        let got = strips(content, pixels, &edge(Side::Left, &[wall(0, 1079)]), |_| {
            unreachable!("a wall asks for no name")
        });

        assert_eq!(got.len(), 1);
        let r = got[0].rect;
        assert_eq!(r.left(), content.left());
        assert_eq!(r.width(), THICKNESS);
        assert!((r.top() - content.top()).abs() < 1e-3);
        assert!(
            (r.height() - content.height()).abs() < 1e-3,
            "a full edge came out {} of {}",
            r.height(),
            content.height()
        );
        assert!(got[0].wall);
    }

    /// The run names its last pixel. Without the `+ 1` a one-pixel run would be a strip
    /// of no height at all, and every run would be one pixel short.
    #[test]
    fn a_run_of_one_pixel_is_one_pixel_wide_and_not_nothing() {
        let (content, pixels) = one();
        let got = strips(
            content,
            pixels,
            &edge(Side::Top, &[wall(0, 0)]),
            |_| unreachable!(),
        );

        assert_eq!(got.len(), 1);
        let expected = content.width() / 1920.0;
        assert!(
            (got[0].rect.width() - expected).abs() < 1e-3,
            "one pixel of 1920 came out {} where {expected} was due",
            got[0].rect.width()
        );
    }

    /// The runs are in desktop pixels, so a screen that does not start at the desktop's
    /// origin has to be shifted before it is scaled — otherwise every strip on every
    /// secondary screen is somewhere else entirely.
    #[test]
    fn a_screen_away_from_the_origin_counts_from_its_own_corner() {
        let content = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 225.0));
        // The second screen of a side-by-side pair.
        let pixels = Rect::new(1920.0, 0.0, 1920.0, 1080.0);

        let got = strips(
            content,
            pixels,
            &edge(Side::Top, &[crosses(1920, 2879, 7)]),
            |id| format!("zone {id}"),
        );

        assert_eq!(got.len(), 1);
        // The first half of its own width, not something off the left of the map.
        assert!((got[0].rect.left() - content.left()).abs() < 1e-3);
        assert!((got[0].rect.width() - content.width() / 2.0).abs() < 0.5);
    }

    #[test]
    fn a_run_outside_the_screen_is_clamped_away_rather_than_drawn_outside() {
        let (content, pixels) = one();
        // Entirely above the screen: clamped to zero length and dropped.
        let got = strips(
            content,
            pixels,
            &edge(Side::Left, &[wall(-500, -1)]),
            |_| unreachable!(),
        );
        assert!(got.is_empty(), "a strip was drawn off the frame: {got:?}");

        // Half off the bottom: kept, but not drawn past the edge.
        let half = strips(
            content,
            pixels,
            &edge(Side::Left, &[wall(540, 5000)]),
            |_| unreachable!(),
        );
        assert_eq!(half.len(), 1);
        assert!(content.contains_rect(half[0].rect), "{:?}", half[0].rect);
    }

    #[test]
    fn every_strip_sits_inside_the_lit_part_against_its_own_edge() {
        let (content, pixels) = one();
        let all = [
            (Side::Left, wall(0, 1079)),
            (Side::Right, wall(0, 1079)),
            (Side::Top, wall(0, 1919)),
            (Side::Bottom, wall(0, 1919)),
        ];
        for (side, run) in all {
            let got = strips(content, pixels, &edge(side, &[run]), |_| unreachable!());
            let r = got[0].rect;
            assert!(content.contains_rect(r), "{side:?} strip at {r:?}");
            let flush = match side {
                Side::Left => r.left() == content.left(),
                Side::Right => r.right() == content.right(),
                Side::Top => r.top() == content.top(),
                Side::Bottom => r.bottom() == content.bottom(),
            };
            assert!(flush, "{side:?} strip is not against its edge: {r:?}");
        }
    }

    #[test]
    fn a_crossing_says_where_it_goes_and_a_wall_says_it_stops() {
        let (content, pixels) = one();
        let got = strips(
            content,
            pixels,
            &[
                Edge {
                    side: Side::Left,
                    runs: vec![wall(0, 539)],
                },
                Edge {
                    side: Side::Right,
                    runs: vec![crosses(0, 539, 3)],
                },
            ],
            |id| format!("Odyssey {id}"),
        );

        assert_eq!(got[0].tip, "Left: wall — the cursor stops here");
        assert!(got[0].wall);
        assert_eq!(got[1].tip, "Right: crosses into Odyssey 3");
        assert!(!got[1].wall);
    }

    /// A source with no pixels divides by nothing.
    #[test]
    fn a_screen_with_no_pixels_has_no_strips() {
        let (content, _) = one();
        for pixels in [
            Rect::new(0.0, 0.0, 0.0, 1080.0),
            Rect::new(0.0, 0.0, 1920.0, 0.0),
        ] {
            assert!(
                strips(content, pixels, &edge(Side::Left, &[wall(0, 10)]), |_| {
                    unreachable!()
                })
                .is_empty()
            );
        }
    }

    /// Red and green have to be told apart by something other than the reader's eyes
    /// too — the tooltip carries the same fact, which is what makes the strips readable
    /// to someone who cannot separate the two colours.
    #[test]
    fn a_wall_and_a_crossing_are_not_the_same_colour_nor_the_same_words() {
        assert_ne!(WALL, CROSSING);
        let (content, pixels) = one();
        let got = strips(
            content,
            pixels,
            &edge(Side::Top, &[wall(0, 100), crosses(200, 300, 1)]),
            |id| format!("zone {id}"),
        );
        assert!(got[0].tip.contains("wall"));
        assert!(got[1].tip.contains("crosses into"));
    }
}
