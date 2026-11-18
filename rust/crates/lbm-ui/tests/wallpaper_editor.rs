use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_store::wallpaper_settings::{LayoutWallpaperSettings, ScreenWallpaperKind, WallpaperMode};
use lbm_ui::wallpaper_editor::{panel, Action};
use std::cell::RefCell;
#[test]
fn opening_does_not_configure_a_screen_and_apply_is_an_explicit_action() {
    let settings = RefCell::new(LayoutWallpaperSettings::default());
    let actions = RefCell::new(Vec::new());
    let mut h = Harness::builder()
        .with_size(egui::vec2(620., 600.))
        .build_ui(|ui| {
            lbm_ui::theme::install(ui.ctx());
            if let Some(a) = panel(ui, &mut settings.borrow_mut(), Some("A"), false, true) {
                actions.borrow_mut().push(a);
            }
        });
    h.run();
    assert!(settings.borrow().per_screen.is_empty());
    assert!(actions.borrow().is_empty());
    assert!(h
        .get_by_label("Apply wallpaper")
        .accesskit_node()
        .is_disabled());
    h.get_by_label("Configure this screen").click();
    h.run();
    h.get_by_label("Color").click();
    h.run();
    assert_eq!(
        settings.borrow().per_screen["A"].kind,
        ScreenWallpaperKind::Color
    );
    assert!(actions.borrow().is_empty());
    h.get_by_label("Preview wallpaper").click();
    h.run();
    assert_eq!(*actions.borrow(), [Action::Preview]);
    h.get_by_label("Physical panorama").click();
    h.run();
    assert_eq!(settings.borrow().mode, WallpaperMode::Span);
    assert!(h.query_by_label("Image file").is_some());
}
fn snapshot(width: f32) {
    let mut settings = LayoutWallpaperSettings {
        mode: WallpaperMode::Span,
        span_image_path: Some("/Pictures/été panorama.png".into()),
        ..Default::default()
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 640.))
        .build_ui(|ui| {
            lbm_ui::theme::install(ui.ctx());
            lbm_ui::properties::panel(ui, |ui| {
                ui.heading("Wallpaper");
                panel(ui, &mut settings, None, true, true);
            });
            egui::CentralPanel::default().show(ui, |ui| {
                ui.label("Local preview");
            });
        });
    h.run();
    #[cfg(not(windows))]
    h.snapshot(if width < 500. {
        "wallpaper-editor-narrow"
    } else {
        "wallpaper-editor-wide"
    });
}

#[test]
fn wallpaper_editor_wide() {
    snapshot(1100.);
}
#[test]
fn wallpaper_editor_narrow() {
    snapshot(480.);
}

#[test]
fn browse_is_an_explicit_action_and_cannot_open_two_choosers() {
    for can_browse in [false, true] {
        let settings = RefCell::new(LayoutWallpaperSettings {
            mode: WallpaperMode::Span,
            ..Default::default()
        });
        let actions = RefCell::new(Vec::new());
        let mut h = Harness::builder()
            .with_size(egui::vec2(620., 600.))
            .build_ui(|ui| {
                lbm_ui::theme::install(ui.ctx());
                if let Some(a) = panel(ui, &mut settings.borrow_mut(), None, false, can_browse) {
                    actions.borrow_mut().push(a);
                }
            });
        h.run();
        assert_eq!(
            h.get_by_label("Browse…").accesskit_node().is_disabled(),
            !can_browse
        );
        if can_browse {
            h.get_by_label("Browse…").click();
            h.run();
            assert_eq!(*actions.borrow(), [Action::Browse]);
        } else {
            assert!(actions.borrow().is_empty());
        }
        assert!(
            settings.borrow().span_image_path.is_none(),
            "opening the chooser does not set an image"
        );
    }
}
