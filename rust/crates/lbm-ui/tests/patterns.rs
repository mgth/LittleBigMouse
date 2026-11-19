use egui_kittest::{kittest::Queryable, Harness};
use lbm_pattern::Pattern;
use lbm_ui::patterns::{Action, Family, Strip, Target};

#[derive(Default)]
struct State {
    strip: Strip,
    active: Option<Pattern>,
    blocked: bool,
    selected: String,
    targets: Vec<Target>,
    changes: usize,
}
fn harness() -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(320., 520.))
        .build_ui_state(
            |ui, state: &mut State| {
                lbm_ui::theme::install(ui.ctx());
                if let Some(action) = state.strip.show(
                    ui,
                    state.active,
                    true,
                    state.blocked.then_some("Select a screen"),
                    &state.targets,
                    Some(&state.selected),
                ) {
                    state.changes += 1;
                    match action {
                        Action::Show | Action::Update => {
                            state.active = Some(state.strip.settings.pattern)
                        }
                        Action::Hide => state.active = None,
                    }
                }
                ui.strong("DDC/CI controls");
                ui.add(egui::Slider::new(&mut 50., 0. ..=100.).text("Brightness"));
            },
            State {
                selected: "DP-2".into(),
                targets: vec![
                    Target {
                        connector: "DP-2".into(),
                        name: "Philips · DP-2".into(),
                    },
                    Target {
                        connector: "DP-3".into(),
                        name: "Samsung · DP-3".into(),
                    },
                ],
                ..Default::default()
            },
        )
}

#[test]
fn families_keep_variants_and_parameters_reachable() {
    let mut h = harness();
    h.run();
    for family in Family::ALL {
        h.get_by_label(family.label()).click();
        h.run();
        assert_eq!(h.state().active, Some(family.patterns()[0]));
    }
    h.get_by_label("Levels").click();
    h.run();
    h.get_by_label("Alternate patches").click();
    h.run();
    assert!(h.state().strip.settings.blink);
    h.get_by_label("Sharpness").click();
    h.run();
    h.get_by_label("3 px").click();
    h.run();
    assert_eq!(h.state().strip.settings.line_width, 3);
    h.get_by_label("Invert black / white").click();
    h.run();
    assert!(h.state().strip.settings.invert);
    h.get_by_label("Hide").click();
    h.run();
    assert_eq!(h.state().active, None);
    h.get_by_label("Show").click();
    h.run();
    assert_eq!(h.state().active, Some(Pattern::Sharpness));
}
#[test]
fn comparison_keeps_reference_while_vcp_selection_changes() {
    let mut h = harness();
    h.run();
    h.get_by_label("Compare screens").click();
    h.run();
    assert!(h.state().strip.matching);
    assert_eq!(h.state().strip.reference.as_deref(), Some("DP-2"));
    assert_eq!(h.state().active, Some(Pattern::Solid));
    h.state_mut().selected = "DP-3".into();
    h.run();
    let s = h.state();
    assert_eq!(
        s.strip.targets(&s.targets, Some(&s.selected)),
        ["DP-2", "DP-3"]
    );
    h.get_by_label("White").click();
    h.run();
    assert_eq!(h.state().strip.settings.solid_rgb(), [255; 3]);
    h.get_by_label("50%").click();
    h.run();
    assert_eq!(h.state().strip.settings.solid_rgb(), [128; 3]);
    h.state_mut().blocked = true;
    h.run();
    h.get_by_label("Hide").click();
    h.run();
    assert_eq!(h.state().active, None);
}
#[test]
fn leaving_comparison_restores_the_previous_pattern() {
    let mut h = harness();
    h.run();
    h.get_by_label("Sharpness").click();
    h.run();
    h.get_by_label("3 px").click();
    h.run();
    h.get_by_label("Compare screens").click();
    h.run();
    h.get_by_label("White").click();
    h.run();
    h.get_by_label("Selected screen").click();
    h.run();
    assert!(!h.state().strip.matching);
    assert_eq!(h.state().active, Some(Pattern::Sharpness));
    assert_eq!(h.state().strip.settings.line_width, 3);
    assert_eq!(
        h.state().strip.targets(&h.state().targets, Some("DP-3")),
        ["DP-3"]
    );
}
#[test]
fn removed_outputs_are_never_targeted() {
    let mut h = harness();
    h.run();
    h.get_by_label("Compare screens").click();
    h.run();
    h.state_mut().targets.remove(0);
    h.run();
    let s = h.state();
    assert_eq!(s.strip.targets(&s.targets, Some(&s.selected)), ["DP-3"]);
    assert_eq!(s.strip.reference.as_deref(), Some("DP-3"));
}
#[test]
#[cfg(not(windows))]
fn strip_snapshots() {
    let mut h = harness();
    h.run();
    h.get_by_label("Levels").click();
    h.run();
    h.snapshot("display-lab-patterns");
    h.get_by_label("Sharpness").click();
    h.run();
    h.snapshot("display-lab-sharpness");
    h.get_by_label("Compare screens").click();
    h.run();
    h.snapshot("display-lab-match");
}
