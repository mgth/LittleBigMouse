//! Name visibility and containment through the accessibility tree.

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
    // The name uses the increasing bezel height.
    let small = name_height_drawn(2.0).expect("drawn at 2.0");
    let large = name_height_drawn(4.0).expect("drawn at 4.0");

    assert!(
        large > small * 1.5,
        "the name did not follow the frame: {small} then {large}"
    );
}

/// **However small.** The legibility floor this port carried is gone (maintainer's call,
/// 2026-09-16), so even a subpixel name keeps its accessible label. Asked
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

#[test]
fn a_long_name_fits_one_bezel_line_and_keeps_its_full_accessible_name() {
    let drawn = frame::draw(
        Rect::new(0., 0., 100., 80.),
        Rect::new(5., 15., 90., 60.),
        (0., 0.),
        Ratio { x: 1., y: 1. },
    );
    let name = "A very long manufacturer and monitor model name";
    let harness = Harness::new_ui(|ui| {
        frame::monitor(
            ui,
            &drawn,
            &frame::Look {
                name,
                ..Default::default()
            },
        );
    });
    let bounds = harness
        .get_by_label(name)
        .accesskit_node()
        .bounding_box()
        .unwrap();
    assert!(
        bounds.x0 >= drawn.name_band.left() as f64 && bounds.x1 <= drawn.name_band.right() as f64
    );
    assert!(
        bounds.y0 >= drawn.name_band.top() as f64 && bounds.y1 <= drawn.name_band.bottom() as f64
    );
}
