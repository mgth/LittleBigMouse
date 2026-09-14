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

/// The panel over one set of options, counting what each half asked for.
fn panel<'a>(
    options: &'a std::cell::RefCell<LayoutOptions>,
    saved: &'a std::cell::Cell<usize>,
) -> Harness<'a> {
    watching(options, saved, &EDITS)
}

/// How many times the layout half said it was edited, for the one test that cares.
static EDITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn watching<'a>(
    options: &'a std::cell::RefCell<LayoutOptions>,
    app: &'a std::cell::Cell<usize>,
    layout: &'static std::sync::atomic::AtomicUsize,
) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(560.0, 2200.0))
        .build_ui(move |ui| {
            let mut recording = lbm_ui::shortcut::Recording::default();
            let what = options::panel(ui, &mut options.borrow_mut(), true, &mut recording, None);
            if what.app {
                app.set(app.get() + 1);
            }
            if what.layout {
                layout.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

/// **The two halves are saved differently, so the panel must not confuse them.** An
/// app-wide setting goes to the agent the moment it moves; a per-layout one is an edit
/// that waits for Save with the rest of the layout. A panel that reported `app` for a
/// layout setting would write it to `options.json`, where nothing reads it back.
#[test]
fn a_layout_setting_is_an_edit_and_not_a_save() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static LAYOUT_EDITS: AtomicUsize = AtomicUsize::new(0);

    let options = std::cell::RefCell::new(LayoutOptions::default());
    let app = std::cell::Cell::new(0);
    let mut harness = watching(&options, &app, &LAYOUT_EDITS);
    harness.run();
    LAYOUT_EDITS.store(0, Ordering::Relaxed);

    harness.get_by_label("Allow discontinuity").click();
    harness.run();
    assert!(
        options.borrow().allow_discontinuity,
        "the toggle did not land"
    );
    assert_eq!(
        LAYOUT_EDITS.load(Ordering::Relaxed),
        1,
        "a layout setting was not reported as an edit"
    );
    assert_eq!(
        app.get(),
        0,
        "a layout setting asked for SaveOptions, which does not carry it"
    );

    // And the other way round: an app-wide one is not an edit of the layout.
    LAYOUT_EDITS.store(0, Ordering::Relaxed);
    harness.get_by_label("Activate debug tools").click();
    harness.run();
    assert_eq!(app.get(), 1);
    assert_eq!(
        LAYOUT_EDITS.load(Ordering::Relaxed),
        0,
        "an app-wide setting marked the layout unsaved, so Save would stay lit forever"
    );
}
