//! The drawn frame, through the accessibility tree.
//!
//! The arithmetic is tested beside the code it lives in. What is tested here is the part
//! arithmetic cannot answer: that the name egui actually laid out grows with the frame,
//! and that a name refused for being too small is really absent rather than merely
//! small. Both are questions about what was drawn, and the harness is the only way to
//! ask them without a window.

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::frame::{self, Ratio};

/// A 600x340 mm screen with a 10 mm bezel, drawn at `ratio`; gives back the height the
/// name was laid out at, if it was drawn at all.
fn name_height_drawn(ratio: f64) -> Option<f64> {
    let outside = Rect::new(0.0, 0.0, 620.0, 360.0);
    let content = Rect::new(10.0, 10.0, 600.0, 340.0);
    let drawn = frame::draw(outside, content, (0.0, 0.0), Ratio { x: ratio, y: ratio });

    let harness = Harness::new_ui(|ui| {
        frame::monitor(
            ui,
            &drawn,
            &frame::Look {
                name: "Odyssey G80SD",
                ..Default::default()
            },
        );
    });
    let node = harness.query_by_label("Odyssey G80SD")?;
    let bounds = node.accesskit_node().bounding_box()?;
    Some(bounds.height())
}

#[test]
fn the_name_grows_with_the_frame() {
    // Ratios that keep a 10 mm bezel's name — half of it — above the legibility floor,
    // so what is measured is the growth and not the floor.
    let small = name_height_drawn(2.0).expect("drawn at 2.0");
    let large = name_height_drawn(4.0).expect("drawn at 4.0");

    assert!(
        large > small * 1.5,
        "the name did not follow the frame: {small} then {large}"
    );
}

/// **However small.** The legibility floor this port carried is gone (maintainer's call,
/// 2026-09-16), so a name of a twentieth of a point is laid out like any other. Asked
/// through the harness rather than of the arithmetic: what the floor used to remove was a
/// real label in a real tree, and this is the same question the other way round.
#[test]
fn a_name_is_drawn_however_small_it_comes_out() {
    let height = name_height_drawn(0.01).expect("a name, small as it is");
    assert!(
        height < 1.0,
        "the fixture was meant to produce a sub-point name, got {height}"
    );
}

/// The frame itself is still there when the name is not: a screen you cannot label is
/// still a screen you can see and click. The only way left to have no name is to have no
/// top bezel to print it on.
#[test]
fn the_screen_is_drawn_even_when_its_name_is_not() {
    let outside = Rect::new(0.0, 0.0, 620.0, 350.0);
    let content = Rect::new(10.0, 0.0, 600.0, 340.0);

    let drawn = frame::draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 });

    assert_eq!(drawn.name_height, None);
    assert!(drawn.outside.width() > 0.0 && drawn.outside.height() > 0.0);
    assert!(
        drawn.content.width() < drawn.outside.width(),
        "the side bezels are still there"
    );
}
