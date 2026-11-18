//! Hardware controls live entirely in the selected monitor's property panel.
use crate::theme;
use egui::{Color32, RichText};
use lbm_ipc::vcp::{self, Feature, Monitor, Write};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Refresh,
    Set(Write),
}
#[derive(Default)]
pub struct Panel {
    confirmation: Option<(u8, u16)>,
}
impl Panel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        mut monitor: Option<&mut Monitor>,
        busy: bool,
        error: Option<&str>,
    ) -> Option<Action> {
        let mut action = None;
        ui.horizontal(|ui| {
            if busy {
                ui.spinner();
                ui.label("Communicating with monitor…");
            } else if monitor.is_some() {
                theme::status(ui, "DDC/CI connected", theme::ACCENT);
            } else {
                ui.weak("Monitor controls");
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh"))
                .clicked()
            {
                action = Some(Action::Refresh);
            }
        });
        if let Some(error) = error {
            theme::card().show(ui, |ui| {
                ui.colored_label(theme::AMBER, "Monitor unavailable");
                ui.label(error);
            });
        }
        let Some(state) = monitor.as_mut() else {
            return action;
        };
        ui.small(&state.connection);
        ui.weak("Changes apply directly to this monitor.");
        if let Some(note) = &state.note {
            ui.label(RichText::new(note).small().color(theme::AMBER));
        }
        let token = state.token;
        ui.add_enabled_ui(!busy, |ui| {
            for (title, subtitle, codes) in [
                ("Picture", "Light and contrast", &[0x10, 0x12][..]),
                ("Colour", "Monitor colour preset", &[0x14][..]),
                (
                    "RGB gain",
                    "White balance · highlights",
                    &[0x16, 0x18, 0x1a][..],
                ),
                (
                    "RGB black level",
                    "Colour balance · shadows",
                    &[0x6c, 0x6e, 0x70][..],
                ),
                ("Audio", "Monitor speakers", &[0x62, 0x8d][..]),
                ("Monitor", "Input and power", &[0x60, 0xd6][..]),
            ] {
                if !state.features.iter().any(|f| codes.contains(&f.code)) {
                    continue;
                }
                theme::card().show(ui, |ui| {
                    ui.set_min_width((ui.available_width()).max(0.));
                    ui.strong(title);
                    ui.label(RichText::new(subtitle).small().color(theme::MUTED));
                    for code in codes {
                        let Some(feature) = state.features.iter_mut().find(|f| f.code == *code)
                        else {
                            continue;
                        };
                        ui.push_id(*code, |ui| {
                            if let Some(value) = control(ui, feature) {
                                if matches!(code, 0x60 | 0xd6) {
                                    self.confirmation = Some((*code, value));
                                } else {
                                    action = Some(Action::Set(Write {
                                        token,
                                        code: *code,
                                        value,
                                    }));
                                }
                            }
                        });
                    }
                });
                ui.add_space(3.);
            }
            if let Some((code, value)) = self.confirmation {
                theme::card().show(ui, |ui| {
                    let label = state
                        .features
                        .iter()
                        .find(|f| f.code == code)
                        .and_then(|f| f.choices.iter().find(|c| c.value == value))
                        .map(|c| c.label.as_str())
                        .unwrap_or("Selected value");
                    ui.strong(format!("{} → {}", vcp::name(code), label));
                    ui.label("This may interrupt the picture or the connection to the monitor.");
                    ui.horizontal(|ui| {
                        if ui.button("Apply to monitor").clicked() {
                            action = Some(Action::Set(Write { token, code, value }));
                            self.confirmation = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirmation = None;
                        }
                    });
                });
            }
        });
        action
    }
}
fn control(ui: &mut egui::Ui, f: &mut Feature) -> Option<u16> {
    let name = vcp::name(f.code);
    let color = match f.code {
        0x16 | 0x6c => Color32::from_rgb(237, 133, 133),
        0x18 | 0x6e => theme::ACCENT,
        0x1a | 0x70 => Color32::from_rgb(135, 179, 242),
        _ => theme::TEXT,
    };
    if vcp::continuous(f.code) {
        let mut commit = false;
        ui.horizontal(|ui| {
            ui.add_sized(
                [65., 28.],
                egui::Label::new(RichText::new(name).color(color)),
            );
            ui.scope(|ui| {
                ui.style_mut().spacing.slider_width = (ui.available_width() - 53.).max(30.);
                ui.visuals_mut().selection.bg_fill = color.gamma_multiply(0.3);
                ui.visuals_mut().selection.stroke.color = color;
                let response = ui.add(
                    egui::Slider::new(&mut f.value, 0..=f.maximum)
                        .show_value(false)
                        .trailing_fill(true),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Slider, ui.is_enabled(), name)
                });
                commit |= response.drag_stopped() || (response.changed() && !response.dragged());
            });
            let response = ui
                .add_sized(
                    [45., 28.],
                    egui::DragValue::new(&mut f.value)
                        .range(0..=f.maximum)
                        .speed(1.)
                        .update_while_editing(false),
                )
                .on_hover_text(format!("Hardware range: 0–{}", f.maximum));
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::DragValue,
                    ui.is_enabled(),
                    format!("{name} value"),
                )
            });
            commit |= response.drag_stopped() || (response.changed() && !response.dragged());
        });
        commit.then_some(f.value)
    } else {
        ui.label(name);
        let mut value = f.value;
        let label = f
            .choices
            .iter()
            .find(|c| c.value == value)
            .map(|c| c.label.clone())
            .unwrap_or_else(|| format!("0x{value:02X}"));
        egui::ComboBox::from_id_salt("value")
            .width(ui.available_width())
            .selected_text(label)
            .show_ui(ui, |ui| {
                for choice in &f.choices {
                    ui.selectable_value(&mut value, choice.value, &choice.label);
                }
            })
            .response
            .widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, ui.is_enabled(), name)
            });
        (value != f.value).then_some(value)
    }
}
