//! A monitor on the map: how big it is drawn, and what has to shrink with it.
//!
//! The map draws every screen at one ratio of millimetres to points, and everything
//! inside a screen follows: the bezel, the name, the logo. Avalonia does it with a
//! `VisualRatio` threaded through bindings; here it is arithmetic on the way in, so the
//! view has nothing to decide.
//!
//! The one judgement in it is the floor. A screen drawn small enough makes its name
//! unreadable, and a name too small to read is worse than no name: it is ink that looks
//! like information. Below the floor the name is dropped rather than drawn illegibly.

use lbm_layout::geo::Rect;

/// Millimetres to points, per axis — the two differ when the map is stretched to fit a
/// window that is not the desktop's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratio {
    pub x: f64,
    pub y: f64,
}

/// What a monitor looks like on the map, in points, relative to the map's own corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    /// The screen with its bezel: what the user sees as the object.
    pub outside: egui::Rect,
    /// The lit part, inside the bezel: what the layout is actually about.
    pub content: egui::Rect,
    /// The height to set the name in, or `None` when it would be too small to read.
    pub name_height: Option<f32>,
}

/// Names below this are not information any more.
pub const LEGIBLE: f32 = 7.0;

/// A name takes this much of the lit height, as the Avalonia frame does.
const NAME_SHARE: f64 = 0.14;

/// Where a monitor lands on the map, and how big its parts are.
///
/// `mm_outside` is the screen with its bezel and `mm_content` the lit part, both in the
/// layout's millimetre space; `origin` is the map's own corner in that space, so that
/// what comes out is relative to the map.
pub fn draw(mm_outside: Rect, mm_content: Rect, origin: (f64, f64), ratio: Ratio) -> Drawn {
    let place = |r: Rect| {
        egui::Rect::from_min_size(
            egui::pos2(
                ((r.left() - origin.0) * ratio.x) as f32,
                ((r.top() - origin.1) * ratio.y) as f32,
            ),
            egui::vec2((r.width() * ratio.x) as f32, (r.height() * ratio.y) as f32),
        )
    };
    let content = place(mm_content);
    let name_height = (mm_content.height() * ratio.y * NAME_SHARE) as f32;
    Drawn {
        outside: place(mm_outside),
        content,
        name_height: (name_height >= LEGIBLE).then_some(name_height),
    }
}

/// Draws one monitor. Returns the rectangle it took, so a caller can lay several out.
pub fn monitor(ui: &mut egui::Ui, drawn: &Drawn, name: &str) -> egui::Rect {
    let painter = ui.painter();
    // The bezel is what is between the two rectangles; drawing the outside first and
    // the lit part over it is the whole of it.
    painter.rect_filled(drawn.outside, 2.0, ui.visuals().widgets.inactive.bg_fill);
    painter.rect_filled(drawn.content, 0.0, ui.visuals().extreme_bg_color);

    if let Some(height) = drawn.name_height {
        // Laid out inside the lit part at its own size, not stretched to fill it: the
        // rectangle it ends up with has to be the text's, or nothing downstream — a
        // test, a screen reader — can tell a big name from a small one in a big frame.
        ui.scope_builder(egui::UiBuilder::new().max_rect(drawn.content), |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(name)
                        .size(height)
                        .color(ui.visuals().text_color()),
                )
                .selectable(false),
            );
        });
    }
    drawn.outside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> (Rect, Rect) {
        // A 600x340 mm screen with a 10 mm bezel all round.
        let outside = Rect::new(0.0, 0.0, 620.0, 360.0);
        let content = Rect::new(10.0, 10.0, 600.0, 340.0);
        (outside, content)
    }

    #[test]
    fn everything_scales_with_the_ratio() {
        let (outside, content) = screen();
        let one = draw(outside, content, (0.0, 0.0), Ratio { x: 0.5, y: 0.5 });
        let two = draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 });

        assert_eq!(two.outside.width(), one.outside.width() * 2.0);
        assert_eq!(two.content.height(), one.content.height() * 2.0);
        assert_eq!(two.name_height.unwrap(), one.name_height.unwrap() * 2.0);
    }

    #[test]
    fn the_lit_part_sits_inside_the_bezel_by_the_bezel() {
        let (outside, content) = screen();
        let ratio = Ratio { x: 0.5, y: 0.5 };

        let drawn = draw(outside, content, (0.0, 0.0), ratio);

        // 10 mm of bezel at half a point per millimetre is 5 points, on every side.
        assert_eq!(drawn.content.left() - drawn.outside.left(), 5.0);
        assert_eq!(drawn.outside.right() - drawn.content.right(), 5.0);
        assert_eq!(drawn.content.top() - drawn.outside.top(), 5.0);
        assert_eq!(drawn.outside.bottom() - drawn.content.bottom(), 5.0);
    }

    #[test]
    fn the_map_corner_is_where_the_drawing_starts() {
        let (outside, content) = screen();
        // A second screen 700 mm to the right, on a map whose corner is that screen's.
        let right = Rect::new(700.0, 0.0, 620.0, 360.0);
        let right_content = Rect::new(710.0, 10.0, 600.0, 340.0);

        let drawn = draw(right, right_content, (700.0, 0.0), Ratio { x: 1.0, y: 1.0 });

        assert_eq!(
            drawn.outside.left(),
            0.0,
            "the map starts at its own corner"
        );
        assert_eq!(
            draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 })
                .outside
                .left(),
            0.0
        );
    }

    /// A name too small to read is ink that looks like information.
    #[test]
    fn a_name_that_would_be_unreadable_is_not_drawn() {
        let (outside, content) = screen();

        let tiny = draw(outside, content, (0.0, 0.0), Ratio { x: 0.01, y: 0.01 });
        assert_eq!(tiny.name_height, None);

        // And just above the floor it is there, so the rule is a threshold and not a
        // blanket refusal.
        let ratio = LEGIBLE as f64 / (340.0 * NAME_SHARE);
        let just = draw(outside, content, (0.0, 0.0), Ratio { x: ratio, y: ratio });
        assert!(just.name_height.is_some());
    }
}
