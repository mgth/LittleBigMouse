//! The settings pane overlays the workspace without replacing it or forwarding clicks.
use egui_kittest::{kittest::Queryable, Harness};
use lbm_ui::options::{self, Page};

struct State {
    open: bool,
    page: Page,
    clicks: usize,
    recording: lbm_ui::shortcut::Recording,
    options: lbm_layout::model::LayoutOptions,
}

fn drawer() -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(800.0, 700.0))
        .build_ui_state(
            |ui, state| {
                lbm_ui::theme::install(ui.ctx());
                let bounds = ui.max_rect();
                let response = ui.put(
                    egui::Rect::from_min_size(egui::pos2(600.0, 300.0), egui::vec2(140.0, 40.0)),
                    egui::Button::new("Workspace"),
                );
                if response.clicked() {
                    state.clicks += 1;
                }
                options::drawer(
                    ui.ctx(),
                    bounds,
                    &mut state.open,
                    &mut state.page,
                    state.recording != lbm_ui::shortcut::Recording::Yes,
                    |ui, page| {
                        options::panel(
                            ui,
                            &mut state.options,
                            false,
                            &mut state.recording,
                            None,
                            page,
                        );
                    },
                );
            },
            State {
                open: true,
                page: Page::General,
                clicks: 0,
                recording: Default::default(),
                options: Default::default(),
            },
        )
}

#[test]
fn backdrop_dismisses_without_clicking_the_workspace() {
    let mut h = drawer();
    h.run();
    let pos = egui::pos2(650.0, 320.0);
    h.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        h.run();
    }
    assert!(!h.state().open);
    assert_eq!(h.state().clicks, 0);
}

#[test]
fn categories_and_close_keep_the_workspace_state() {
    let mut h = drawer();
    h.get_by_label("Advanced").click();
    h.run();
    assert_eq!(h.state().page, Page::Advanced);
    assert!(h.state().open);
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().open);
    assert_eq!(h.state().clicks, 0);
}

#[test]
fn escape_closes_the_drawer_but_is_reserved_while_recording() {
    let mut h = drawer();
    h.state_mut().recording = lbm_ui::shortcut::Recording::Yes;
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().open);
    h.state_mut().recording = lbm_ui::shortcut::Recording::No;
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().open);
}
