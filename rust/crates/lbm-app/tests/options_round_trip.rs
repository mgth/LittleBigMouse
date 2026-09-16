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

    let (method, extra) = lbm_app::settings::save_options(&edited, None);
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

/// A preview is the same document as a save — including the excluded list it must not
/// carry. The trap is one the preview inherits: `preview` is built on `save_layout`
/// precisely so it cannot drift away from that.
#[test]
fn a_preview_carries_the_same_document_as_a_save() {
    let mut layout = lbm_layout::model::Layout::new(LayoutOptions::default());
    layout.id = "TESTMON1".to_owned();

    let (save_method, save_extra) = lbm_app::settings::save_layout(&layout);
    let (preview_method, preview_extra) = lbm_app::settings::preview(&layout);
    assert_eq!(save_method, "SaveLayout");
    assert_eq!(preview_method, "Preview");
    assert_eq!(
        save_extra, preview_extra,
        "a preview and a save describe the same layout; only the agent's answer differs"
    );

    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame(3, preview_method, preview_extra))
            .expect("the agent could not parse the window's preview");
    match parsed.request {
        lbm_agent::api::Request::Preview {
            layout_id,
            document,
        } => {
            assert_eq!(layout_id, "TESTMON1");
            assert_eq!(
                document.excluded, None,
                "a preview would apply an excluded list this window never read"
            );
            assert!(document.layout.is_some());
        }
        other => panic!("the frame parsed as something else: {other:?}"),
    }
}

/// And ending one is a request the agent knows, with nothing in it to get wrong.
#[test]
fn ending_a_preview_is_a_request_the_agent_understands() {
    let (method, extra) = lbm_app::settings::end_preview();
    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame(4, method, extra)).expect("parsed");
    assert!(matches!(
        parsed.request,
        lbm_agent::api::Request::EndPreview
    ));
}

/// The frame the writer thread puts on the wire: the id, the method, and the rest.
fn frame(id: u64, method: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut frame = serde_json::json!({ "Id": id, "Method": method });
    let serde_json::Value::Object(rest) = extra else {
        panic!("an object");
    };
    for (key, value) in rest {
        frame[key] = value;
    }
    frame
}

//==========================================================================//
// The excluded list                                                        //
//==========================================================================//

/// A window that has not read the list still says so on the wire. `None` is not an empty
/// list: the agent leaves the list alone for it, and writes the empty one for `Some([])`.
#[test]
fn an_unread_excluded_list_is_absent_and_a_read_empty_one_is_present() {
    let options = LayoutOptions::default();

    let (_, unread) = lbm_app::settings::save_options(&options, None);
    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame(1, "SaveOptions", unread)).expect("parsed");
    match parsed.request {
        lbm_agent::api::Request::SaveOptions { excluded, .. } => assert_eq!(
            excluded, None,
            "a window that never read the list claimed it was empty"
        ),
        other => panic!("{other:?}"),
    }

    let empty: Vec<String> = Vec::new();
    let (_, read) = lbm_app::settings::save_options(&options, Some(&empty));
    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame(2, "SaveOptions", read)).expect("parsed");
    match parsed.request {
        lbm_agent::api::Request::SaveOptions { excluded, .. } => assert_eq!(
            excluded,
            Some(Vec::new()),
            "a list read and emptied on purpose must reach the agent as empty"
        ),
        other => panic!("{other:?}"),
    }
}

/// The list travels as written, in order.
#[test]
fn the_excluded_list_reaches_the_agent_as_it_is() {
    let list = vec![r"\steamapps\".to_owned(), "game.exe".to_owned()];
    let (_, extra) = lbm_app::settings::save_options(&LayoutOptions::default(), Some(&list));
    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame(5, "SaveOptions", extra)).expect("parsed");
    match parsed.request {
        lbm_agent::api::Request::SaveOptions { excluded, .. } => {
            assert_eq!(excluded, Some(list));
        }
        other => panic!("{other:?}"),
    }
}

/// The defaults top-up is separator-insensitive, so a list seeded with the Windows-style
/// entries is not doubled with their Linux twins. This is the store's rule, and the one
/// the window leans on rather than re-deciding.
#[test]
fn topping_up_the_defaults_does_not_duplicate_the_other_spelling() {
    use lbm_store::excluded_process_defaults as defaults;

    // Whatever this platform's defaults are, spelled the other way round.
    let other_spelling: Vec<String> = defaults::ALL.iter().map(|e| e.replace('\\', "/")).collect();
    for entry in defaults::ALL {
        assert!(
            defaults::contains_entry(other_spelling.iter(), entry),
            "{entry} was not recognised in its other spelling, so a top-up would \
             duplicate it"
        );
    }
}

/// **A missing file is read as "not read", and nothing is created.** The parser the
/// window borrows writes the file when it is absent; this is the guard in front of it,
/// and the thing that would be silently wrong is that the file appears.
#[test]
fn a_missing_excluded_file_is_unread_and_stays_missing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Excluded.txt");

    assert_eq!(lbm_app::settings::read_excluded(file.clone()), None);
    assert!(
        !file.exists(),
        "reading an absent excluded list created it: this window creates nothing"
    );
}

/// And a file that exists is read by the daemon's own rules: blank lines are nothing,
/// `:` lines are comments, everything else is an exclusion.
#[test]
fn an_excluded_file_is_read_the_way_the_daemon_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Excluded.txt");
    std::fs::write(
        &file,
        ":Excluded processes\n\\Epic Games\\\n\n/Heroic/\n:a note\ngame.exe\n",
    )
    .unwrap();

    let list = lbm_app::settings::read_excluded(file).expect("the file is there");
    assert_eq!(
        list,
        vec![
            r"\Epic Games\".to_owned(),
            "/Heroic/".to_owned(),
            "game.exe".to_owned()
        ],
        "the comments or the blank line were taken for exclusions"
    );
}

/// The apply request the window sends, parsed by the agent's own type.
///
/// This is the one request in the window whose success **moves the user's real screens**,
/// so what it says has to be exactly what it means: the document that will be saved, and
/// the scale choice the user actually made.
#[test]
fn the_agent_understands_the_apply_the_window_sends() {
    let mut layout = lbm_layout::model::Layout::new(LayoutOptions::default());
    layout.id = "TESTMON1".to_owned();

    let (method, extra) = lbm_app::settings::apply_topology(&layout, true);
    assert_eq!(method, "ApplyTopology");
    let parsed: lbm_agent::api::RequestFrame =
        serde_json::from_value(frame(11, method, extra)).expect("parsed");
    match parsed.request {
        lbm_agent::api::Request::ApplyTopology {
            layout_id,
            document,
            adjust_scale,
            dry_run,
        } => {
            assert!(!dry_run, "the window's Apply really applies");
            assert_eq!(layout_id, "TESTMON1");
            assert!(adjust_scale);
            assert!(document.layout.is_some(), "nothing to apply");
            assert_eq!(
                document.excluded, None,
                "an apply saves the document too, so it would erase the exclusions"
            );
        }
        other => panic!("{other:?}"),
    }
}

/// And the default is not to rescale: someone who asked for their screens to be
/// rearranged did not necessarily ask for every monitor's scale to change.
#[test]
fn an_apply_only_adjusts_scales_when_it_was_asked_to() {
    let layout = lbm_layout::model::Layout::new(LayoutOptions::default());
    let (_, extra) = lbm_app::settings::apply_topology(&layout, false);
    assert_eq!(extra["AdjustScale"], serde_json::json!(false));
}
