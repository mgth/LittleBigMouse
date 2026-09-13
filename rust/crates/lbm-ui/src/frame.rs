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
    /// Where the name goes: the top bezel, across the lit width — the frame's cell
    /// (0, 1). It is printed *on the plastic*, where a real monitor prints it, not over
    /// the picture.
    pub name_band: egui::Rect,
    /// The height to set the name in, or `None` when it would be too small to read.
    pub name_height: Option<f32>,
}

impl Drawn {
    /// The same monitor, moved. Every rectangle is listed rather than spread from
    /// `self`, so that a rectangle added to `Drawn` later cannot be quietly left behind
    /// — the compiler asks about it here, next to the field it belongs to.
    pub fn translate(self, by: egui::Vec2) -> Drawn {
        Drawn {
            outside: self.outside.translate(by),
            content: self.content.translate(by),
            name_band: self.name_band.translate(by),
            name_height: self.name_height,
        }
    }
}

/// Names below this are not information any more. This floor is a judgement of this
/// port, not a rule of the Avalonia frame, which draws the name at whatever size the
/// arithmetic gives.
pub const LEGIBLE: f32 = 7.0;

/// A name is set at half the height of the bezel it is printed on.
///
/// `MonitorFrameView.axaml:184-186` binds the label's font size to `TopRow.Bounds.Height`
/// through a `Scale` converter with a parameter of `0.5`, and `TopRow` is the border
/// stretched into the frame grid's cell (0, 0) — whose height is `Unrotated.TopBorder`
/// (`:155-158`, rows `Auto,*,Auto`). So the name is measured against **the bezel**, and
/// a screen with a thin bezel gets a small name however large its panel is.
const NAME_SHARE_OF_TOP_BEZEL: f64 = 0.5;

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
    let outside = place(mm_outside);
    let content = place(mm_content);
    // The top bezel, across the lit width: the frame grid's cell (0, 1), whose row is
    // the border's height and whose column is the panel's width.
    let name_band = egui::Rect::from_min_max(
        egui::pos2(content.left(), outside.top()),
        egui::pos2(content.right(), content.top()),
    );
    let name_height = (name_band.height() as f64 * NAME_SHARE_OF_TOP_BEZEL) as f32;
    Drawn {
        outside,
        content,
        name_band,
        name_height: (name_height >= LEGIBLE).then_some(name_height),
    }
}

/// The bezel's colour, which is how a selected screen is told apart.
///
/// A function rather than two lines inside the painting, so that "selected looks
/// different" is something a test can ask without a pixel: the drawing itself can only
/// be checked by eye, but the choice it makes can be checked here.
pub fn bezel_fill(visuals: &egui::Visuals, selected: bool) -> egui::Color32 {
    if selected {
        visuals.selection.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    }
}

/// Draws one monitor. Returns the rectangle it took, so a caller can lay several out.
pub fn monitor(ui: &mut egui::Ui, drawn: &Drawn, name: &str, selected: bool) -> egui::Rect {
    let fill = bezel_fill(ui.visuals(), selected);
    let painter = ui.painter();
    // The bezel is what is between the two rectangles; drawing the outside first and
    // the lit part over it is the whole of it.
    painter.rect_filled(drawn.outside, 2.0, fill);
    painter.rect_filled(drawn.content, 0.0, ui.visuals().extreme_bg_color);

    if let Some(height) = drawn.name_height {
        // In the bezel band, at its own size, not stretched to fill it: the rectangle it
        // ends up with has to be the text's, or nothing downstream — a test, a screen
        // reader — can tell a big name from a small one in a big frame.
        //
        // Sat on the bottom of the band, as `VerticalAlignment="Bottom"` does: the name
        // rests on the edge of the screen rather than floating in the plastic.
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(drawn.name_band)
                .layout(egui::Layout::bottom_up(egui::Align::LEFT)),
            |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(name)
                            .size(height)
                            .color(ui.visuals().text_color()),
                    )
                    .selectable(false),
                );
            },
        );
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
        // Above the legibility floor at both ratios: a name of 10 points and one of 20,
        // out of the 10 mm bezel. At 1:1 this screen's name would be 5 points and get
        // dropped, which is the floor's business and tested on its own below.
        let one = draw(outside, content, (0.0, 0.0), Ratio { x: 2.0, y: 2.0 });
        let two = draw(outside, content, (0.0, 0.0), Ratio { x: 4.0, y: 4.0 });

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

    /// Whatever the theme, the selected screen is not painted like the others — the
    /// map is read at a glance, and a selection you have to infer is not one.
    #[test]
    fn a_selected_screen_is_not_painted_like_the_rest() {
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            assert_ne!(
                bezel_fill(&visuals, true),
                bezel_fill(&visuals, false),
                "selection is invisible in this theme"
            );
        }
    }

    /// A name too small to read is ink that looks like information.
    #[test]
    fn a_name_that_would_be_unreadable_is_not_drawn() {
        let (outside, content) = screen();

        let tiny = draw(outside, content, (0.0, 0.0), Ratio { x: 0.01, y: 0.01 });
        assert_eq!(tiny.name_height, None);

        // And just above the floor it is there, so the rule is a threshold and not a
        // blanket refusal. The floor is measured against the 10 mm top bezel, not the
        // 340 mm panel, so it bites at a far larger ratio than a panel-sized rule would.
        let ratio = LEGIBLE as f64 / (10.0 * NAME_SHARE_OF_TOP_BEZEL);
        let just = draw(outside, content, (0.0, 0.0), Ratio { x: ratio, y: ratio });
        assert!(just.name_height.is_some());
    }

    /// The name is printed on the plastic, where a monitor prints it — not over the
    /// picture. Getting this wrong is not a matter of taste: a name laid over the lit
    /// part covers the thing the map is about, and on a small frame it covers all of it.
    #[test]
    fn the_name_is_printed_on_the_bezel_and_not_on_the_screen() {
        let (outside, content) = screen();
        let drawn = draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 });

        assert!(
            drawn.name_band.bottom() <= drawn.content.top(),
            "the name band reaches over the lit part: {:?} against {:?}",
            drawn.name_band,
            drawn.content
        );
        assert_eq!(drawn.name_band.top(), drawn.outside.top());
        // Across the lit width, which is the frame grid's middle column.
        assert_eq!(drawn.name_band.left(), drawn.content.left());
        assert_eq!(drawn.name_band.right(), drawn.content.right());
    }

    /// Half the bezel it is printed on, and nothing to do with the size of the panel.
    /// A wide screen with a thin bezel gets a small name; that is the Avalonia rule.
    #[test]
    fn the_name_is_measured_against_the_bezel_not_the_panel() {
        // Zoomed in far enough that both names clear the legibility floor, so that what
        // is compared is the rule and not the floor.
        let ratio = Ratio { x: 20.0, y: 20.0 };
        // Same 600x340 panel, two different bezels.
        let thin = draw(
            Rect::new(0.0, 0.0, 604.0, 344.0),
            Rect::new(2.0, 2.0, 600.0, 340.0),
            (0.0, 0.0),
            ratio,
        );
        let thick = draw(
            Rect::new(0.0, 0.0, 640.0, 380.0),
            Rect::new(20.0, 20.0, 600.0, 340.0),
            (0.0, 0.0),
            ratio,
        );

        assert_eq!(
            thin.name_height,
            Some(20.0),
            "half of a 2 mm bezel, at 20:1"
        );
        assert_eq!(thick.name_height, Some(200.0), "half of a 20 mm bezel");
        assert_eq!(
            thin.content.height(),
            thick.content.height(),
            "the panels are the same, so a panel-sized rule would give the same name"
        );
    }
}
