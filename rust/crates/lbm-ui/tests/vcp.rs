use egui::{vec2, Key};
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_ipc::vcp::{Choice, Feature, Monitor};
use lbm_ui::vcp::{Action, Panel};

struct State {
    panel: Panel,
    monitor: Monitor,
    busy: bool,
    actions: Vec<Action>,
}
fn harness(width: f32) -> Harness<'static, State> {
    Harness::builder()
        .with_size(vec2(width, 1070.))
        .build_ui_state(
            |ui, state| {
                lbm_ui::theme::install(ui.ctx());
                if let Some(action) =
                    state
                        .panel
                        .show(ui, Some(&mut state.monitor), state.busy, None)
                {
                    state.actions.push(action);
                }
            },
            State {
                panel: Default::default(),
                busy: false,
                actions: vec![],
                monitor: Monitor {
                    token: 8,
                    connection: "DP-2 · DDC/CI · I²C 9".into(),
                    note: None,
                    features: [0x10, 0x12, 0x16, 0x18, 0x1a, 0x6c, 0x6e, 0x70, 0x62]
                        .into_iter()
                        .map(|code| Feature {
                            code,
                            value: 30,
                            minimum: 0,
                            maximum: if code == 0x10 { 80 } else { 100 },
                            choices: vec![],
                        })
                        .chain([
                            Feature {
                                code: 0x60,
                                value: 15,
                                minimum: 0,
                                maximum: 255,
                                choices: vec![
                                    Choice {
                                        value: 15,
                                        label: "DisplayPort-1".into(),
                                    },
                                    Choice {
                                        value: 17,
                                        label: "HDMI-1".into(),
                                    },
                                ],
                            },
                            Feature {
                                code: 0xd6,
                                value: 1,
                                minimum: 0,
                                maximum: 255,
                                choices: vec![
                                    Choice {
                                        value: 1,
                                        label: "On".into(),
                                    },
                                    Choice {
                                        value: 4,
                                        label: "Off".into(),
                                    },
                                ],
                            },
                        ])
                        .collect(),
                },
            },
        )
}
#[test]
fn brightness_uses_hardware_range_and_keyboard_commits_a_correlated_write() {
    let mut h = harness(300.);
    h.run();
    let slider = h
        .get_all_by_label("Brightness")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Slider)
        .unwrap();
    assert_eq!(slider.accesskit_node().max_numeric_value(), Some(80.));
    slider.focus();
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    assert!(
        matches!(h.state().actions.last(), Some(Action::Set(w)) if w.code == 0x10 && w.token == 8 && w.value > 30 && w.value <= 80)
    );
    h.state_mut().busy = true;
    h.run_steps(2);
    assert!(h.get_by_label("Refresh").accesskit_node().is_disabled());
}
#[test]
fn input_switch_waits_for_apply_and_cancel_sends_nothing() {
    let mut h = harness(300.);
    h.run();
    h.get_all_by_label("Input source")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::ComboBox)
        .unwrap()
        .click();
    h.run();
    h.get_by_label("HDMI-1").click();
    h.run();
    assert!(h.state().actions.is_empty());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().actions.is_empty());
    h.get_all_by_label("Input source")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::ComboBox)
        .unwrap()
        .click();
    h.run();
    h.get_by_label("HDMI-1").click();
    h.run();
    h.get_by_label("Apply to monitor").click();
    h.run();
    assert!(
        matches!(h.state().actions.last(), Some(Action::Set(w)) if w.code == 0x60 && w.value == 17)
    );
}
#[test]
fn a_special_volume_is_shown_without_being_rewritten_and_can_be_chosen() {
    let mut h = harness(300.);
    {
        let volume = h
            .state_mut()
            .monitor
            .features
            .iter_mut()
            .find(|f| f.code == lbm_ipc::vcp::VOLUME)
            .unwrap();
        volume.value = 255;
        volume.minimum = 1;
        volume.maximum = 100;
        volume.choices = lbm_ipc::vcp::volume_choices();
    }
    h.run();
    assert!(h.state().actions.is_empty(), "drawing it writes nothing");
    let volume = |h: &Harness<'static, State>| {
        h.state()
            .monitor
            .features
            .iter()
            .find(|f| f.code == lbm_ipc::vcp::VOLUME)
            .unwrap()
            .value
    };
    assert_eq!(volume(&h), 255);
    let slider = h
        .get_all_by_label("Volume")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Slider)
        .unwrap();
    assert_eq!(slider.accesskit_node().min_numeric_value(), Some(1.));
    h.get_by_label("Default level").click();
    h.run();
    assert!(matches!(
        h.state().actions.last(),
        Some(Action::Set(w)) if w.code == lbm_ipc::vcp::VOLUME && w.value == 0
    ));
}
#[test]
#[cfg(not(windows))]
fn property_panel_snapshot() {
    let mut h = harness(300.);
    h.run();
    h.snapshot("vcp-properties");
}
#[test]
#[cfg(not(windows))]
fn narrow_property_panel_snapshot() {
    let mut h = harness(250.);
    h.run();
    h.snapshot("vcp-properties-narrow");
}
