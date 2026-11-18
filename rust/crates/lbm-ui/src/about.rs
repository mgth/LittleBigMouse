//! Application information formerly repeated by the C# About monitor plugin.
use crate::theme;

pub fn window(ctx: &egui::Context, open: &mut bool, version: &str) {
    if !*open {
        return;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        *open = false;
        return;
    }
    let mut close = false;
    egui::Window::new("About Little Big Mouse")
        .id(egui::Id::new("about-lbm"))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_width(360.)
        .default_height(520.)
        .max_height((ctx.content_rect().height() - 60.).max(160.))
        .vscroll(true)
        .default_pos(ctx.content_rect().center() - egui::vec2(180., 260.))
        .show(ctx, |ui| {
            ui.add_space(12.);
            ui.vertical_centered(|ui| {
                let pixels = (80. * ctx.pixels_per_point()).ceil() as u32;
                let key = egui::Id::new("about-original-logo");
                let cached = ctx.data(|d| d.get_temp::<(u32, egui::TextureHandle)>(key));
                let logo = match cached {
                    Some((size, texture)) if size == pixels => texture,
                    _ => {
                        let svg = include_bytes!("../../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon/lbm_logo.svg");
                        let tree = lbm_icons::load(svg, lbm_icons::Rgba(226, 235, 240, 255)).expect("bundled LBM logo");
                        let rgba = lbm_icons::render(&tree, pixels).expect("LBM logo dimensions");
                        let texture = ctx.load_texture("about-logo", egui::ColorImage::from_rgba_premultiplied([pixels as usize; 2], &rgba), egui::TextureOptions::LINEAR);
                        ctx.data_mut(|d| d.insert_temp(key, (pixels, texture.clone())));
                        texture
                    }
                };
                ui.image((logo.id(), egui::vec2(80., 80.)));
                ui.add_space(10.);
                ui.label(egui::RichText::new("Little Big Mouse").size(24.).strong());
                ui.label(egui::RichText::new(format!("egui · Version {version}")).color(theme::MUTED));
                ui.add_space(16.);
                ui.label("DPI-aware mouse movement across screens");
                ui.label(egui::RichText::new("Windows · Linux").small().color(theme::MUTED));
                ui.add_space(16.);
                ui.label("Copyright © 2015–2026");
                ui.hyperlink_to("Mathieu GRENET", "https://github.com/mgth/LittleBigMouse");
                ui.add_space(8.);
                ui.label(egui::RichText::new("GNU General Public License v3").small().color(theme::MUTED));
                ui.add_space(16.);
                crate::support::button(ui, "Support on Ko-fi");
                ui.add_space(20.);
                close = ui.button("Close").clicked();
                ui.add_space(6.);
            });
        });
    if close {
        *open = false;
    }
}
