//! The mode bar, through the accessibility tree.
//!
//! The enum and its words are tested beside the code. What is asked here is what a table
//! of strings cannot answer: that the buttons are really there and really disabled, that
//! pressing the one already on goes back to the default, and that a mode which stops
//! being available does not stay on.

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::frame::{self, Ratio};
use lbm_ui::mode::{self, Mode, Offered};

/// The bar, drawn once; gives back what it chose and the harness to look at.
fn bar<'a>(
    current: Mode,
    offered: Offered,
) -> (Harness<'a>, std::rc::Rc<std::cell::Cell<Option<Mode>>>) {
    let chosen = std::rc::Rc::new(std::cell::Cell::new(None));
    let out = chosen.clone();
    let harness = Harness::builder()
        .with_size(egui::vec2(900.0, 60.0))
        .build_ui(move |ui| {
            // Kept rather than overwritten: `run` draws more than one frame, and only
            // the frame the click lands on answers anything. A plain `set` would be
            // cleared by the frame after it.
            if let Some(chosen) = mode::bar(ui, current, &offered) {
                out.set(Some(chosen));
            }
        });
    (harness, chosen)
}

#[test]
fn every_mode_the_c_sharp_registers_has_a_button() {
    let (harness, _) = bar(Mode::Default, Offered::default());
    for mode in Mode::ALL {
        assert!(
            harness.query_by_label(mode.label()).is_some(),
            "{} has no button",
            mode.label()
        );
    }
    assert!(
        harness.query_by_label(Mode::Default.label()).is_none(),
        "the default is not a mode you choose, it is where the bar goes back to"
    );
}

/// The point of keeping the modes that are not ported: they are visible, and they are
/// visibly not available. A bar that simply omitted them would say the features never
/// existed.
#[test]
fn what_is_not_ported_is_shown_and_disabled() {
    let (harness, _) = bar(Mode::Default, Offered::default());
    let disabled = |label: &str| harness.get_by_label(label).accesskit_node().is_disabled();
    assert!(!disabled(Mode::Location.label()), "Location is drawn today");
    assert!(!disabled(Mode::About.label()), "About is drawn today");
    for mode in [Mode::Info, Mode::Resistance, Mode::Size, Mode::Vcp] {
        assert!(disabled(mode.label()), "{} is not ported", mode.label());
    }
}

#[test]
fn choosing_a_mode_gives_it_back() {
    let (mut harness, chosen) = bar(Mode::Default, Offered::default());
    harness.get_by_label(Mode::Location.label()).click();
    harness.run();
    assert_eq!(chosen.get(), Some(Mode::Location));
}

/// `MainPluginsViewModelExtension.cs:22-26`, which is what makes a row of toggles behave
/// like a radio group — and the only way back to the default, since it has no button.
#[test]
fn pressing_the_mode_that_is_already_on_goes_back_to_the_default() {
    let (mut harness, chosen) = bar(Mode::Location, Offered::default());
    harness.get_by_label(Mode::Location.label()).click();
    harness.run();
    assert_eq!(chosen.get(), Some(Mode::Default));
}

/// `:49-50`. The case is real: VCP's button follows a setting the user can turn off from
/// the settings view while the VCP mode is the one on.
#[test]
fn a_mode_that_stops_being_available_does_not_stay_on() {
    let (_, chosen) = bar(Mode::Vcp, Offered::default());
    assert_eq!(
        chosen.get(),
        Some(Mode::Default),
        "the mode was dropped without anyone pressing anything"
    );
}

//==========================================================================//
// What a mode writes in a frame                                            //
//==========================================================================//

fn frame_with(details: &[(&str, String)], ratio: f64, size: (f32, f32)) -> Harness<'static> {
    let outside = Rect::new(0.0, 0.0, 640.0, 380.0);
    let content = Rect::new(20.0, 20.0, 600.0, 340.0);
    let drawn = frame::draw(outside, content, (0.0, 0.0), Ratio { x: ratio, y: ratio });
    let rows: Vec<(String, String)> = details
        .iter()
        .map(|(l, v)| ((*l).to_owned(), v.clone()))
        .collect();
    Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .build_ui(move |ui| {
            let borrowed: Vec<(&str, String)> =
                rows.iter().map(|(l, v)| (l.as_str(), v.clone())).collect();
            frame::monitor(
                ui,
                &drawn,
                &frame::Look {
                    name: "Odyssey G80SD",
                    details: &borrowed,
                    ..Default::default()
                },
            );
        })
}

#[test]
fn the_rows_a_mode_asks_for_are_written_in_the_lit_part() {
    let harness = frame_with(&[("at", "40, 20 mm".to_owned())], 0.6, (500.0, 320.0));
    assert!(harness.query_by_label("at").is_some());
    assert!(harness.query_by_label("40, 20 mm").is_some());
}

/// A screen drawn too small to hold text gets none of it. The alternative is ink that
/// looks like information, or one screen's numbers written across its neighbour.
#[test]
fn a_frame_too_small_for_the_rows_is_left_alone() {
    let harness = frame_with(&[("at", "40, 20 mm".to_owned())], 0.02, (500.0, 320.0));
    assert!(harness.query_by_label("40, 20 mm").is_none());
}

/// And the default writes nothing inside: the frame stays a picture of a screen.
///
/// Drawn big enough for the name, which is the *bezel's* text and does scale — so this
/// also pins the two apart: the name follows the frame, the rows do not.
#[test]
fn the_default_mode_writes_nothing_inside() {
    let harness = frame_with(&[], 1.5, (1100.0, 700.0));
    assert!(
        harness.query_by_label("Odyssey G80SD").is_some(),
        "the name is still there"
    );
    assert!(harness.query_by_label("at").is_none());
}
