//! The map, through the accessibility tree.
//!
//! The arithmetic is tested beside the code. What is asked here is what arithmetic
//! cannot answer: that the frames egui actually laid out sit where the fit says, and
//! that a click on a screen reaches the screen — the name is drawn on top of the
//! rectangle that senses the pointer, and whether the label swallows the click is a
//! question about egui, not about the formula.

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::map::{self, MapMonitor};

/// Two screens with the 20 mm bezel the oracle's desktops actually have — the name is
/// half of that bezel, so a thinner one would put every name under the legibility floor
/// and these tests would be asserting about text that is not drawn.
fn two_screens() -> Vec<MapMonitor<'static>> {
    vec![
        MapMonitor {
            id: "left",
            name: "Left screen",
            mm_outside: Rect::new(0.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(20.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
            details: &[],
        },
        MapMonitor {
            id: "right",
            name: "Right screen",
            mm_outside: Rect::new(700.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(720.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
            details: &[],
        },
    ]
}

/// A window of a stated size, because the name's size follows the fit and the fit
/// follows the window: a harness left at whatever size it defaults to would make these
/// tests depend on that default.
fn harness<'a>(app: impl FnMut(&mut egui::Ui) + 'a) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(app)
}

/// The left edge of a laid-out name, as the tree reports it.
fn left_of(harness: &Harness, label: &str) -> f64 {
    harness
        .get_by_label(label)
        .accesskit_node()
        .bounding_box()
        .expect("the name was laid out")
        .min_x()
}

/// Not merely "the right one is on the right" — that survives losing the scale
/// altogether. The gap between the two names, on screen, has to be the gap between the
/// two screens in millimetres taken at the fitted ratio.
#[test]
fn the_screens_land_where_the_fit_says_they_do() {
    let screens = two_screens();
    let fitted = std::cell::Cell::new(0.0);
    let harness = harness(|ui| {
        let fit = map::fit(map::extent(&screens), ui.max_rect());
        fitted.set(fit.ratio);
        map::draw(ui, &screens, &fit, None);
    });

    let apart = left_of(&harness, "Right screen") - left_of(&harness, "Left screen");
    let expected = 700.0 * fitted.get();
    assert!(
        (apart - expected).abs() < 1.0,
        "the names are {apart} apart on screen where the screens are {expected}"
    );
}

/// The frame is claimed for the pointer before the name is painted over it, so the
/// click has to reach the frame and not stop at the label.
#[test]
fn clicking_a_screen_selects_it() {
    let screens = two_screens();
    let clicked = std::cell::Cell::new(None);

    let mut harness = harness(|ui| {
        let fit = map::fit(map::extent(&screens), ui.max_rect());
        if let Some(map::Gesture::Clicked(id)) = map::draw(ui, &screens, &fit, None) {
            clicked.set(Some(id));
        }
    });

    harness.get_by_label("Right screen").click();
    harness.run();

    assert_eq!(clicked.get(), Some("right"));
}

/// A screen drawn too small to be labelled is still a screen you can click: the
/// rectangle is sensed whether or not a name goes into it.
#[test]
fn a_screen_with_no_room_for_its_name_is_still_a_target() {
    let screens = two_screens();
    let fit = map::fit(
        map::extent(&screens),
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 100.0)),
    );

    let drawn = fit.place(&screens[1]);
    assert_eq!(drawn.name_height, None, "the name should have been dropped");
    assert_eq!(
        map::hit(&screens, &fit, drawn.outside.center()),
        Some("right")
    );
}
