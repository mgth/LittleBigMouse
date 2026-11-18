//! Shared support link, matching Avalonia's SupportLinks.KofiUrl.
use crate::{settings_icons::Icon, theme};

const KOFI_URL: &str = "https://ko-fi.com/mgth";

pub(crate) fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let id = ui.make_persistent_id("support-coffee");
    let button = egui::Button::new((egui::Atom::custom(id, egui::vec2(18., 18.)), label))
        .fill(theme::SELECTED)
        .atom_ui(ui);
    if let Some(rect) = button.rect(id) {
        Icon::Coffee.paint(ui, rect, theme::ACCENT);
    }
    let response = button.response.on_hover_text(KOFI_URL);
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(KOFI_URL));
    }
    response
}

pub(crate) fn card(ui: &mut egui::Ui, compact: bool) {
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 86.).max(80.);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 0.),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(egui::RichText::new("Support Little Big Mouse").strong());
                    if !compact {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    "Free and open source — buy me a coffee if it helps you.",
                                )
                                .small()
                                .color(theme::MUTED),
                            )
                            .wrap(),
                        );
                    }
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                button(ui, "Ko-fi");
            });
        });
    });
}
