//! The settings panel and the wire, where they meet.
//!
//! `lbm-ui` draws the panel and `lbm-store` owns `GlobalOptionsDto`; neither can check
//! that they agree, because neither depends on the other. The window does, and this is
//! the thing that would otherwise go wrong quietly: a setting the user moves, written to
//! a DTO field that does not exist or is never read back, looks like it worked and is
//! gone on the next launch.

use lbm_layout::model::{LayoutOptions, PER_MONITOR};
use lbm_store::layout_dto_mapper::{apply_global_options, to_global_options_dto};

/// Every app-wide option the panel can move survives the round trip the window performs:
/// edited here, turned into the DTO `SaveOptions` carries, and applied back the way the
/// agent applies it (`world.rs`, `apply_global_options`).
///
/// A field the mapper forgets in either direction silently reverts, and the user sees a
/// switch flick back at the next start.
#[test]
fn every_setting_the_panel_moves_survives_the_wire() {
    // Deliberately all different from the defaults, so a field the mapper drops shows up
    // as the default rather than as the value we set.
    let mut edited = LayoutOptions::default();
    edited.auto_update = !edited.auto_update;
    edited.start_minimized = !edited.start_minimized;
    edited.hide_tray_icon = !edited.hide_tray_icon;
    edited.bound_to_agent = !edited.bound_to_agent;
    edited.start_elevated = !edited.start_elevated;
    edited.vcp_control = !edited.vcp_control;
    edited.experimental_features = !edited.experimental_features;
    edited.debug_tools = !edited.debug_tools;
    edited.show_monitor_action_warning = !edited.show_monitor_action_warning;
    edited.priority = "High".to_owned();
    edited.priority_unhooked = "Idle".to_owned();
    edited.border_values = PER_MONITOR.to_owned();

    let dto = to_global_options_dto(&edited, None);
    let mut landed = LayoutOptions::default();
    apply_global_options(&mut landed, Some(&dto));

    for (name, got, want) in [
        ("auto_update", landed.auto_update, edited.auto_update),
        (
            "start_minimized",
            landed.start_minimized,
            edited.start_minimized,
        ),
        (
            "hide_tray_icon",
            landed.hide_tray_icon,
            edited.hide_tray_icon,
        ),
        (
            "bound_to_agent",
            landed.bound_to_agent,
            edited.bound_to_agent,
        ),
        (
            "start_elevated",
            landed.start_elevated,
            edited.start_elevated,
        ),
        ("vcp_control", landed.vcp_control, edited.vcp_control),
        (
            "experimental_features",
            landed.experimental_features,
            edited.experimental_features,
        ),
        ("debug_tools", landed.debug_tools, edited.debug_tools),
        (
            "show_monitor_action_warning",
            landed.show_monitor_action_warning,
            edited.show_monitor_action_warning,
        ),
    ] {
        assert_eq!(got, want, "{name} did not survive the round trip");
    }
    assert_eq!(landed.priority, edited.priority);
    assert_eq!(landed.priority_unhooked, edited.priority_unhooked);
    assert_eq!(landed.border_values, edited.border_values);
}

/// The two settings the panel shows but does not edit still have to come back untouched:
/// the window sends the **whole** set, so anything it read and did not draw is written
/// back as it was — or the panel would quietly reset it.
#[test]
fn what_the_panel_does_not_draw_is_still_carried_through() {
    let stored = LayoutOptions {
        rescue_shortcut: "Ctrl+Alt+Q".to_owned(),
        home_cinema: true,
        pinned: true,
        ..Default::default()
    };

    let dto = to_global_options_dto(&stored, None);
    let mut landed = LayoutOptions::default();
    apply_global_options(&mut landed, Some(&dto));

    assert_eq!(
        landed.rescue_shortcut, "Ctrl+Alt+Q",
        "the rescue shortcut is not drawn, so it must at least be preserved"
    );
    assert!(landed.home_cinema);
    assert!(landed.pinned);
}

/// The excluded list is **not** part of this DTO, and that is what makes it safe for the
/// window to send the options without ever having read the list.
///
/// If it ever joins `GlobalOptionsDto`, the window's `SaveOptions` starts erasing the
/// user's exclusions, silently. This is the test that would notice.
#[test]
fn the_excluded_list_does_not_travel_with_the_options() {
    let stored = LayoutOptions {
        excluded_list: vec!["game.exe".to_owned(), "another.exe".to_owned()],
        ..Default::default()
    };

    let dto = to_global_options_dto(&stored, None);
    let mut landed = LayoutOptions {
        excluded_list: vec!["kept.exe".to_owned()],
        ..Default::default()
    };
    apply_global_options(&mut landed, Some(&dto));

    assert_eq!(
        landed.excluded_list,
        vec!["kept.exe".to_owned()],
        "the options DTO now carries the excluded list: the window must read the list \
         before it may send options, or it will erase it"
    );
}

/// The frame the window really sends, parsed by the **agent's own** request type.
///
/// This is where a settings panel dies quietly: a field spelled `Options` instead of
/// `options`, a DTO serialised camelCase, a method name off by a letter. The agent
/// answers with an error nobody is looking at, the switch stays where the user put it,
/// and the setting is gone at the next start. So the request is built by the same
/// function the window calls, and handed to the type the agent parses with.
#[test]
fn the_agent_understands_the_request_the_window_sends() {
    let edited = LayoutOptions {
        vcp_control: true,
        priority: "High".to_owned(),
        border_values: PER_MONITOR.to_owned(),
        load_at_startup: true,
        ..Default::default()
    };

    let (method, extra) = lbm_app::settings::save_options(&edited);
    assert_eq!(method, "SaveOptions");

    // Exactly what the writer thread puts on the wire: the method, the id, and the rest.
    let mut frame = serde_json::json!({ "Id": 7, "Method": method });
    let serde_json::Value::Object(rest) = extra else {
        panic!("the request's extra members are an object");
    };
    for (key, value) in rest {
        frame[key] = value;
    }

    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame).expect("the agent could not parse the window's frame");
    assert_eq!(parsed.id, 7);
    match parsed.request {
        lbm_agent::api::Request::SaveOptions {
            options,
            excluded,
            load_at_startup,
        } => {
            let options = options.expect("the options did not survive the frame");
            assert_eq!(options.vcp_control, Some(true));
            assert_eq!(options.priority.as_deref(), Some("High"));
            assert_eq!(options.border_values.as_deref(), Some(PER_MONITOR));
            assert_eq!(load_at_startup, Some(true));
            assert_eq!(
                excluded, None,
                "the window sent an excluded list it never read, which would erase the \
                 user's exclusions"
            );
        }
        other => panic!("the frame parsed as something else: {other:?}"),
    }
}

/// **The Save button must not erase the user's exclusions.**
///
/// `LayoutDocument::of` fills `Excluded` from `layout.options.excluded_list` — right for
/// the agent, which has read the list, and destructive for this window, which has not:
/// the empty list it holds would be applied over the real one. Harder to notice than the
/// `SaveOptions` case, because here the field is filled in for you.
#[test]
fn saving_the_layout_carries_no_excluded_list() {
    let mut layout = lbm_layout::model::Layout::new(LayoutOptions::default());
    layout.id = "TESTMON1".to_owned();

    let (method, extra) = lbm_app::settings::save_layout(&layout);
    assert_eq!(method, "SaveLayout");

    let mut frame = serde_json::json!({ "Id": 3, "Method": method });
    let serde_json::Value::Object(rest) = extra else {
        panic!("an object");
    };
    for (key, value) in rest {
        frame[key] = value;
    }

    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame).expect("the agent could not parse the window's frame");
    match parsed.request {
        lbm_agent::api::Request::SaveLayout {
            layout_id,
            document,
        } => {
            assert_eq!(layout_id, "TESTMON1");
            assert_eq!(
                document.excluded, None,
                "Save would apply an excluded list this window never read, erasing the \
                 user's exclusions"
            );
            // What it *must* carry: the layout and its options, or Save saves nothing.
            assert!(
                document.layout.is_some(),
                "the layout itself did not travel"
            );
            assert!(document.global_options.is_some());
        }
        other => panic!("the frame parsed as something else: {other:?}"),
    }
}
