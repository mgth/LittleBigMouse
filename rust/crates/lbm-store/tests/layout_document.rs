//! A layout handed over as a document: what a save would write, applied elsewhere.

mod common;

use common::{
    add_monitor_with_source, design_options, temp_excluded_file, test_persistence, FakeStore,
};
use lbm_layout::geo::{Point, Rect, Size};
use lbm_layout::model::{
    BorderSection, DisplaySize, DisplaySource, Layout, Monitor, MonitorModel, Ratio,
};
use lbm_store::LayoutDocument;
use serde_json::Value;

const MONITOR_ID: &str = "MON1";

/// One monitor with one attached source (C# `NewLayout` of the persistence tests).
fn new_layout() -> Layout {
    let mut layout = Layout::new(design_options());
    layout.id = "TESTMON1".to_owned();
    let mut model = MonitorModel::new("TST1234");
    model.physical_size.set_width(600.0);
    model.physical_size.set_height(340.0);
    let monitor = Monitor::new(MONITOR_ID, &model);
    let mut source = DisplaySource::new("SRC1");
    source.attached_to_desktop = true;
    source.in_pixel = DisplaySize::from_rect(Rect::from_location_size(
        Point::new(0.0, 0.0),
        Size::new(1920.0, 1080.0),
    ));
    add_monitor_with_source(&mut layout, model, monitor, source, "DEV1");
    layout
}

/// What a user does in the editor: a move, a resistance, a size, options.
fn edited() -> Layout {
    let mut layout = new_layout();
    layout.set_location(MONITOR_ID, Point::new(123.5, -42.25));
    layout.set_depth_ratio(MONITOR_ID, Ratio::new(1.25, 1.5));
    layout.edit_border_resistance(MONITOR_ID, |br| {
        br.right
            .sections
            .push(BorderSection::new(10.0, 60.0, 5.0, true, 6.0, false));
    });
    layout.edit_model("TST1234", |size, _| {
        size.set_left_border(17.0);
    });
    layout.edit_options(|o| {
        o.loop_x = true;
        o.rescue_shortcut = "Ctrl+Alt+F12".to_owned();
        o.touch_mouse_independent = true;
        o.stylus_mouse_independent = true;

        o.restore_keyboard_focus = true;
        o.focus_restore_delay = 275;
        o.focus_restore_on_mouse_move = true;
        o.touch_all_displays = false;
        o.touch_display_ids = "SRC1".into();
        o.touch_override_modifier = "Alt".into();
        o.focus_keep_apps = "notepad.exe".into();
        o.focus_restore_apps = "control.exe".into();
        o.excluded_list = vec!["/usr/bin/steam".to_owned()];
    });
    layout
}

#[test]
fn a_document_applied_elsewhere_gives_the_edited_layout() {
    let edited = edited();
    let document = LayoutDocument::of(&edited);

    // Through the wire and back.
    let wire = serde_json::to_string(&document).unwrap();
    let document: LayoutDocument = serde_json::from_str(&wire).unwrap();

    // The agent's copy, loaded before the edit: after the document, the same layout.
    let mut agent = new_layout();
    agent.mark_saved();
    document.apply(&mut agent);
    assert_eq!(LayoutDocument::of(&agent), LayoutDocument::of(&edited));
    let monitor = agent.monitor(MONITOR_ID).unwrap();
    assert_eq!(agent.depth_projection(monitor).unwrap().x, 123.5);
    assert_eq!(monitor.depth_ratio.y, 1.5);
    assert_eq!(monitor.border_resistance.right.sections.len(), 1);
    assert_eq!(agent.options.rescue_shortcut, "Ctrl+Alt+F12");
    assert!(agent.options.touch_mouse_independent);
    assert!(agent.options.stylus_mouse_independent);

    assert!(agent.options.restore_keyboard_focus);
    assert_eq!(agent.options.focus_restore_delay, 275);
    assert!(agent.options.focus_restore_on_mouse_move);
    assert!(!agent.options.touch_all_displays);
    assert_eq!(agent.options.touch_display_ids, "SRC1");
    assert_eq!(agent.options.touch_override_modifier, "Alt");
    assert_eq!(agent.options.focus_keep_apps, "notepad.exe");
    assert_eq!(agent.options.focus_restore_apps, "control.exe");
    assert_eq!(
        lbm_layout::zoning::compute_zones(&agent).touch_display_bounds,
        "0,0,1920,1080"
    );
    assert_eq!(agent.options.excluded_list, ["/usr/bin/steam"]);
    // An edit, not a load: saved when the agent saves it.
    assert!(!agent.saved());
}

#[test]
fn the_agent_saving_a_document_writes_what_the_frontend_would_have() {
    let (_a, excluded_a) = temp_excluded_file("lbm-document-a");
    let (_b, excluded_b) = temp_excluded_file("lbm-document-b");
    let frontend_store = FakeStore::default();
    let agent_store = FakeStore::default();

    // Before v6: the frontend saves its edit itself.
    let mut edit = edited();
    test_persistence(frontend_store.clone(), &excluded_a)
        .save(&mut edit)
        .unwrap();

    // v6: it sends the document, the agent applies it to its own copy and saves.
    let mut agent = new_layout();
    LayoutDocument::of(&edited()).apply(&mut agent);
    assert!(test_persistence(agent_store.clone(), &excluded_b)
        .save(&mut agent)
        .unwrap());
    assert!(agent.saved());

    assert_eq!(
        *agent_store.layouts.borrow(),
        *frontend_store.layouts.borrow()
    );
    assert_eq!(
        *agent_store.models.borrow(),
        *frontend_store.models.borrow()
    );
    assert_eq!(
        *agent_store.global_options.borrow(),
        *frontend_store.global_options.borrow()
    );
    assert_eq!(
        std::fs::read_to_string(excluded_b).unwrap(),
        std::fs::read_to_string(excluded_a).unwrap()
    );
}

#[test]
fn on_the_wire_it_is_the_stores_json_in_one_object() {
    let value: Value = serde_json::to_value(LayoutDocument::of(&edited())).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["GlobalOptions", "Layout", "Models", "Excluded"]);
    assert_eq!(value["Layout"]["Options"]["LoopX"], true);
    assert_eq!(value["GlobalOptions"]["RescueShortcut"], "Ctrl+Alt+F12");
    assert_eq!(value["GlobalOptions"]["TouchMouseIndependent"], true);
    assert_eq!(value["GlobalOptions"]["StylusMouseIndependent"], true);
    assert_eq!(value["GlobalOptions"]["RestoreKeyboardFocus"], true);
    assert!(value["Layout"]["Monitors"][MONITOR_ID].is_object());

    // Parts left out are left alone.
    let partial: LayoutDocument = serde_json::from_str(r#"{"Excluded": []}"#).unwrap();
    let mut layout = edited();
    let before = LayoutDocument::of(&layout);
    partial.apply(&mut layout);
    let after = LayoutDocument::of(&layout);
    assert_eq!(after.layout, before.layout);
    assert_eq!(after.excluded, Some(Vec::new()));
}
