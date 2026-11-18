use egui_kittest::{kittest::Queryable, Harness};
use lbm_ui::placement::{Action, Panel};

#[derive(Default)]
struct State {
    panel: Panel,
    blocked: bool,
    needs_stop: bool,
    actions: Vec<Action>,
}

fn harness() -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(290., 350.))
        .build_ui_state(
            |ui, state: &mut State| {
                lbm_ui::theme::install(ui.ctx());
                if let Some(action) = state.panel.show(
                    ui,
                    state.blocked.then_some("Reload first"),
                    state.needs_stop,
                    true,
                ) {
                    state.actions.push(action);
                }
            },
            State::default(),
        )
}

#[test]
fn import_is_direct_and_export_has_preview_scale_and_cancel() {
    let mut h = harness();
    h.run();
    h.get_by_label("System → LBM").click();
    h.run();
    assert_eq!(h.state().actions, [Action::FromSystem]);
    h.get_by_label("LBM → System").click();
    h.run();
    assert_eq!(h.state().actions.len(), 1);
    h.get_by_label("Also adjust display scales").click();
    h.get_by_label("Preview changes").click();
    h.run();
    assert!(h.state().panel.adjust_scale);
    assert_eq!(h.state().actions.last(), Some(&Action::Preview));
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().actions.len(), 2);
    h.get_by_label("LBM → System").click();
    h.run();
    h.get_by_label("Apply to system").click();
    h.run();
    assert_eq!(h.state().actions.last(), Some(&Action::Apply));
}

#[test]
fn stale_confirmation_is_cleared_when_displays_change() {
    let mut h = harness();
    h.run();
    h.get_by_label("LBM → System").click();
    h.run();
    h.state_mut().blocked = true;
    h.run();
    assert!(h.query_by_label("Apply to system").is_none());
    h.state_mut().blocked = false;
    h.state_mut().needs_stop = true;
    h.run();
    h.get_by_label("System → LBM").click();
    h.run();
    assert_eq!(h.state().actions, [Action::FromSystem]);
}
