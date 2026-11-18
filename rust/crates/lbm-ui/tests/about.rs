use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_ui::mode::{self, Mode, Offered, ToolbarAction};
struct State {
    open: bool,
    mode: Mode,
}
fn harness(width: f32) -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(width, 600.))
        .build_ui_state(
            |ui, state| {
                lbm_ui::theme::install(ui.ctx());
                egui::Panel::top("toolbar").show(ui, |ui| {
                    if let Some(action) = mode::toolbar(ui, state.mode, &Offered::default(), false)
                    {
                        match action {
                            ToolbarAction::About => state.open = true,
                            ToolbarAction::Mode(mode) => state.mode = mode,
                            ToolbarAction::Options => {}
                        }
                    }
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.label("Monitor layout");
                });
                lbm_ui::about::window(ui.ctx(), &mut state.open, "0.1.0");
            },
            State {
                open: false,
                mode: Mode::Location,
            },
        )
}
#[test]
fn about_is_at_the_right_and_opens_without_changing_mode() {
    let mut h = harness(480.);
    h.run();
    let about = h
        .get_by_label("About")
        .accesskit_node()
        .bounding_box()
        .unwrap();
    let wallpaper = h
        .get_by_label("Wallpaper")
        .accesskit_node()
        .bounding_box()
        .unwrap();
    assert!(about.x0 > wallpaper.x1);
    assert!(about.x1 > 460. && about.x1 <= 480.);
    h.get_by_label("About").click();
    h.run();
    assert_eq!(h.state().mode, Mode::Location);
    assert!(h.query_by_label("Little Big Mouse").is_some());
    assert!(h.query_by_label("Mathieu GRENET").is_some());
    assert!(h.query_by_label("egui · Version 0.1.0").is_some());
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().open);
    h.get_by_label("About").click();
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!h.state().open);
    assert_eq!(h.state().mode, Mode::Location);
}
#[test]
#[cfg(not(windows))]
fn about_window_snapshot() {
    let mut h = harness(760.);
    h.run();
    h.get_by_label("About").click();
    h.run();
    h.snapshot("about-window");
}
