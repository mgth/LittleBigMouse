use egui_kittest::{kittest::Queryable, Harness};
use lbm_ipc::displays::{Change, Output, Outputs};
use lbm_ui::displays::{Did, Panel};
use std::cell::RefCell;

fn outputs() -> Outputs {
    Outputs {
        signature: "desktop".into(),
        outputs: vec![
            Output {
                connector: "DP-1".into(),
                name: "Main".into(),
                enabled: true,
                primary: true,
            },
            Output {
                connector: "HDMI-1".into(),
                name: "Side".into(),
                enabled: false,
                primary: false,
            },
        ],
    }
}

#[test]
fn a_detached_output_is_reachable_and_confirmation_can_be_cancelled() {
    let outputs = outputs();
    let mut panel = Panel {
        selected: Some("HDMI-1".into()),
        ..Default::default()
    };
    let actions = RefCell::new(Vec::new());
    let mut harness = Harness::new_ui(|ui| {
        if let Some(did) = panel.draw(ui, Some(&outputs), None, false, true, false) {
            actions.borrow_mut().push(did);
        }
    });
    assert!(harness.query_by_label("Side · HDMI-1").is_some());
    let buttons: Vec<_> = harness.get_all_by_label("Attach").collect();
    buttons[0].click();
    harness.run();
    assert!(actions.borrow().is_empty());
    assert!(harness.query_by_label("Attach HDMI-1?").is_some());
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(actions.borrow().is_empty());
    assert!(harness.query_by_label("Confirm change").is_none());
    harness.get_by_label("Attach").click();
    harness.run();
    harness.get_by_label("Confirm change").click();
    harness.run();
    assert_eq!(
        *actions.borrow(),
        vec![Did::Change("HDMI-1".into(), Change::Attach)]
    );
}

#[test]
fn disabling_warnings_does_not_disable_the_last_screen_guard() {
    let outputs = outputs();
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (Panel, Option<Did>)| {
            if let Some(did) = state.0.draw(ui, Some(&outputs), None, false, false, false) {
                state.1 = Some(did);
            }
        },
        (
            Panel {
                selected: Some("DP-1".into()),
                ..Default::default()
            },
            None,
        ),
    );
    // In inventory order: the selected output, then the detached one.
    harness.get_all_by_label("Detach").next().unwrap().click();
    harness.run();
    assert!(harness.state().1.is_none());
    harness.get_all_by_label("Attach").last().unwrap().click();
    harness.run();
    assert_eq!(
        harness.state().1,
        Some(Did::Change("HDMI-1".into(), Change::Attach))
    );
}

#[test]
fn a_configuration_change_cancels_a_confirmation_and_blocks_its_action() {
    let outputs = outputs();
    let mut panel = Panel {
        selected: Some("HDMI-1".into()),
        confirm: Some(("HDMI-1".into(), Change::Attach)),
    };
    let mut harness = Harness::new_ui(|ui| {
        assert!(panel
            .draw(
                ui,
                Some(&outputs),
                Some("Reload the displays"),
                false,
                true,
                false
            )
            .is_none());
    });
    harness.get_by_label("Attach").click();
    harness.run();
    assert!(harness.query_by_label("Confirm change").is_none());
    drop(harness);
    assert!(panel.confirm.is_none());
}

#[test]
fn selection_changes_cancel_confirmation_and_count_all_active_outputs() {
    let mut outputs = outputs();
    outputs.outputs[1].enabled = true;
    let mut h = Harness::builder().build_ui_state(
        |ui, state: &mut (Panel, Option<Did>)| {
            if let Some(did) = state.0.draw(ui, Some(&outputs), None, false, true, false) {
                state.1 = Some(did);
            }
        },
        (
            Panel {
                selected: Some("DP-1".into()),
                ..Default::default()
            },
            None,
        ),
    );
    h.get_by_label("Detach").click();
    h.run();
    assert!(
        h.query_by_label("Confirm change").is_some(),
        "the other enabled output still counts"
    );
    h.state_mut().0.selected = Some("HDMI-1".into());
    h.run();
    assert!(h.query_by_label("Confirm change").is_none());
    assert!(h.query_by_label("Main · DP-1").is_none());
    h.state_mut().0.selected = None;
    h.run();
    assert!(h.query_by_label("Attach").is_none());
    assert!(h.query_by_label("Detach").is_none());
    assert!(h.state().1.is_none());
}

#[test]
fn a_detached_output_stays_reachable_whatever_the_map_has_selected() {
    let outputs = outputs();
    for selected in [None, Some("DP-1")] {
        let mut h = Harness::builder().build_ui_state(
            |ui, state: &mut (Panel, Option<Did>)| {
                if let Some(did) = state.0.draw(ui, Some(&outputs), None, false, true, false) {
                    state.1 = Some(did);
                }
            },
            (
                Panel {
                    selected: selected.map(str::to_owned),
                    ..Default::default()
                },
                None,
            ),
        );
        h.run();
        assert!(h.state().0.wanted(Some(&outputs)));
        assert!(h.query_by_label("Side · HDMI-1").is_some());
        assert_eq!(
            h.query_by_label("Main · DP-1").is_some(),
            selected.is_some()
        );
        h.get_all_by_label("Attach").last().unwrap().click();
        h.run();
        h.get_by_label("Confirm change").click();
        h.run();
        assert_eq!(
            h.state().1,
            Some(Did::Change("HDMI-1".into(), Change::Attach)),
            "selected: {selected:?}"
        );
    }
}

#[test]
fn nothing_is_offered_without_a_selection_or_a_detached_output() {
    let mut outputs = outputs();
    outputs.outputs[1].enabled = true;
    let panel = Panel::default();
    assert!(!panel.wanted(Some(&outputs)));
    assert!(!panel.wanted(None));
}
