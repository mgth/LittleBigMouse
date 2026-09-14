//! The settings panel, through the accessibility tree.
//!
//! The constants beside the code check themselves. What is asked here is what they
//! cannot: that a toggle the user moves reaches the options, that a pass nobody touched
//! reports **no** change — a panel that claimed one every frame would have the window
//! saving to the agent continuously — and that the sub-option of VCP is absent rather
//! than merely greyed when VCP is off.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use lbm_layout::model::{LayoutOptions, PER_MONITOR};
use lbm_ui::options;

/// The panel over one set of options, with whatever the test wants to assert on after.
fn panel<'a>(
    options: &'a std::cell::RefCell<LayoutOptions>,
    saved: &'a std::cell::Cell<usize>,
) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(520.0, 900.0))
        .build_ui(move |ui| {
            if options::panel(ui, &mut options.borrow_mut(), true) {
                saved.set(saved.get() + 1);
            }
        })
}

#[test]
fn a_toggle_the_user_moves_reaches_the_options() {
    let options = std::cell::RefCell::new(LayoutOptions::default());
    let saved = std::cell::Cell::new(0);
    let mut harness = panel(&options, &saved);
    harness.run();

    assert!(
        !options.borrow().auto_update,
        "the default this test is built on has moved"
    );
    assert_eq!(saved.get(), 0, "nobody touched anything yet");

    harness
        .get_by_label("Check for updates automatically")
        .click();
    harness.run();

    assert!(options.borrow().auto_update, "the toggle did not reach it");
    assert_eq!(saved.get(), 1, "the caller was not told to save");
}

/// The panel must be quiet when it is only being looked at. egui redraws constantly; a
/// "changed" that came back on every frame would have the window writing to the agent
/// forever.
#[test]
fn a_panel_nobody_touches_asks_for_nothing() {
    let options = std::cell::RefCell::new(LayoutOptions::default());
    let saved = std::cell::Cell::new(0);
    let mut harness = panel(&options, &saved);

    for _ in 0..8 {
        harness.run();
    }
    assert_eq!(saved.get(), 0, "the panel reported {} changes", saved.get());
}

/// Turning VCP off takes its sub-option **away**, as `IsVisible="{Binding
/// Model.VcpControl}"` does — it only adds tools inside the VCP panel, so with VCP off
/// there is no panel for it to speak about.
#[test]
fn the_experimental_switch_is_there_only_while_vcp_is() {
    let options = std::cell::RefCell::new(LayoutOptions::default());
    let saved = std::cell::Cell::new(0);
    let mut harness = panel(&options, &saved);
    harness.run();

    assert!(
        harness
            .query_by_label("Enable experimental features")
            .is_none(),
        "the sub-option is offered with nothing to apply it to"
    );

    harness.get_by_label("Enable VCP monitor control").click();
    harness.run();
    assert!(options.borrow().vcp_control);
    assert!(
        harness
            .query_by_label("Enable experimental features")
            .is_some(),
        "VCP is on and its sub-option did not appear"
    );
}

/// A choice reaches the options spelled the way the wire spells it: `PerMonitor` goes to
/// the store as written, so a caption picked up instead of the id would be stored wrong.
#[test]
fn a_choice_stores_the_wire_spelling_and_not_the_caption() {
    let options = std::cell::RefCell::new(LayoutOptions::default());
    let saved = std::cell::Cell::new(0);
    let mut harness = panel(&options, &saved);
    harness.run();

    // Opened by its name. `ComboBox::from_label` puts the header in the node's label and
    // the current choice in its value, which is what a screen reader reads out too.
    harness
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Border values")
        .click();
    harness.run();
    harness.get_by_label(PER_MONITOR).click();
    harness.run();

    assert_eq!(options.borrow().border_values, PER_MONITOR);
    assert_eq!(saved.get(), 1);

    // And picking the one already chosen is not a change: the agent is a socket away.
    harness
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Border values")
        .click();
    harness.run();
    harness.get_by_label(PER_MONITOR).click();
    harness.run();
    assert_eq!(
        saved.get(),
        1,
        "choosing the current value asked for a save"
    );
}
