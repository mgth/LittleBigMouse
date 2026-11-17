use lbm_store::wallpaper_settings::{
    LayoutWallpaperSettings, ScreenWallpaperKind, WallpaperMode, WallpaperStyle,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Preview,
    Apply,
    Reset,
}
pub fn panel(
    ui: &mut egui::Ui,
    settings: &mut LayoutWallpaperSettings,
    selected: Option<&str>,
    can_apply: bool,
) -> Option<Action> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut settings.mode, WallpaperMode::PerScreen, "Per screen");
        ui.selectable_value(&mut settings.mode, WallpaperMode::Span, "Physical panorama");
    });
    let path = if settings.mode == WallpaperMode::Span {
        ui.small("One picture across the physical layout, including gaps and bezels.");
        Some(&mut settings.span_image_path)
    } else if let Some(id) = selected {
        // Merely opening the editor does not configure an otherwise untouched screen.
        if !settings.per_screen.contains_key(id) && ui.button("Configure this screen").clicked() {
            settings.per_screen.insert(id.into(), Default::default());
        }
        if let Some(screen) = settings.per_screen.get_mut(id) {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut screen.kind, ScreenWallpaperKind::Image, "Image");
                ui.selectable_value(&mut screen.kind, ScreenWallpaperKind::Color, "Color");
            });
            ui.horizontal(|ui| {
                let label = ui.label("Background color");
                ui.add(egui::TextEdit::singleline(&mut screen.color).desired_width(90.))
                    .labelled_by(label.id);
            });
            if screen.kind == ScreenWallpaperKind::Image {
                egui::ComboBox::from_id_salt("wallpaper-style")
                    .selected_text(format!("{:?}", screen.style))
                    .show_ui(ui, |ui| {
                        for style in [
                            WallpaperStyle::Fill,
                            WallpaperStyle::Fit,
                            WallpaperStyle::Stretch,
                            WallpaperStyle::Tile,
                            WallpaperStyle::Center,
                        ] {
                            ui.selectable_value(&mut screen.style, style, format!("{style:?}"));
                        }
                    });
                Some(&mut screen.image_path)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        ui.label("Select a screen to configure its wallpaper.");
        None
    };
    if let Some(path) = path {
        let mut value = path.clone().unwrap_or_default();
        let label = ui.label("Image file");
        if ui
            .add(egui::TextEdit::singleline(&mut value).desired_width(f32::INFINITY))
            .labelled_by(label.id)
            .changed()
        {
            *path = Some(value);
        }
        ui.small("Paste a local PNG, JPEG, BMP or GIF path, or drop an image here.");
        if ui.is_enabled() {
            if let Some(dropped) =
                ui.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
            {
                *path = Some(dropped.to_string_lossy().into_owned());
            }
        }
    }
    ui.separator();
    ui.small("Preview changes the map only. Apply saves wallpaper settings and updates the desktop independently of layout Save/Undo.");
    ui.horizontal_wrapped(|ui| {
        if ui.button("Preview wallpaper").clicked() {
            action = Some(Action::Preview);
        }
        if ui
            .add_enabled(can_apply, egui::Button::new("Apply wallpaper"))
            .clicked()
        {
            action = Some(Action::Apply);
        }
        if ui.button("Undo wallpaper edits").clicked() {
            action = Some(Action::Reset);
        }
    });
    action
}
