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
    let mut panel = Panel::default();
    let actions = RefCell::new(Vec::new());
    let mut harness = Harness::new_ui(|ui| {
        if let Some(did) = panel.draw(ui, Some(&outputs), None, false, true, false) {
            actions.borrow_mut().push(did);
        }
    });
    assert!(harness.query_by_label("Side · HDMI-1").is_some());
    let buttons: Vec<_> = harness.get_all_by_label("Attach").collect();
    buttons[1].click();
    harness.run();
    assert!(actions.borrow().is_empty());
    assert!(harness.query_by_label("Attach HDMI-1?").is_some());
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(actions.borrow().is_empty());
    assert!(harness.query_by_label("Confirm change").is_none());
    harness.get_all_by_label("Attach").nth(1).unwrap().click();
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
    let mut panel = Panel::default();
    let actions = RefCell::new(Vec::new());
    let mut harness = Harness::new_ui(|ui| {
        if let Some(did) = panel.draw(ui, Some(&outputs), None, false, false, false) {
            actions.borrow_mut().push(did);
        }
    });
    harness.get_all_by_label("Detach").next().unwrap().click();
    harness.run();
    assert!(actions.borrow().is_empty());
    harness.get_all_by_label("Attach").nth(1).unwrap().click();
    harness.run();
    assert_eq!(
        *actions.borrow(),
        vec![Did::Change("HDMI-1".into(), Change::Attach)]
    );
}

#[test]
fn a_configuration_change_cancels_a_confirmation_and_blocks_its_action() {
    let outputs = outputs();
    let mut panel = Panel {
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
    harness.get_all_by_label("Attach").nth(1).unwrap().click();
    harness.run();
    assert!(harness.query_by_label("Confirm change").is_none());
    drop(harness);
    assert!(panel.confirm.is_none());
}
