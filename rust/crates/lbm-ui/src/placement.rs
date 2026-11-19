//! Whole-layout placement commands, separate from selected-output properties.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    FromSystem,
    Preview,
    Apply,
}

#[derive(Default)]
pub struct Panel {
    pub adjust_scale: bool,
    confirm: bool,
}

impl Panel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        blocked: Option<&str>,
        needs_stop: bool,
        offer_scale: bool,
    ) -> Option<Action> {
        let mut action = None;
        if blocked.is_some() || needs_stop {
            self.confirm = false;
        }
        if !offer_scale {
            self.adjust_scale = false;
        }
        egui::CollapsingHeader::new("System arrangement")
            .default_open(true)
            .show(ui, |ui| {
                ui.small("All screens · synchronize their positions");
                ui.add_enabled_ui(blocked.is_none(), |ui| {
                    if crate::icons::action(ui, crate::icons::Icon::Import, "System » LBM", true).on_hover_text(
                        "Place all monitors from the system configuration, keeping their physical dimensions and bezels. Save to keep the result; Undo to restore the previous layout."
                    ).clicked() {
                        self.confirm = false;
                        action = Some(Action::FromSystem);
                    }
                    ui.add_enabled_ui(!needs_stop, |ui| {
                        if crate::icons::action(ui, crate::icons::Icon::Export, "LBM » System", true).on_hover_text(
                            "Apply the physical arrangement to the system display settings."
                        ).clicked() {
                            self.confirm = !self.confirm;
                        }
                        if self.confirm {
                            ui.small("This saves the layout and moves the system displays. Bezels and physical gaps become touching edges.");
                            if offer_scale {
                                ui.checkbox(&mut self.adjust_scale, "Also adjust display scales");
                            }
                            ui.horizontal_wrapped(|ui| {
                                if crate::icons::action(ui, crate::icons::Icon::Eye, "Preview changes", true).clicked() {
                                    action = Some(Action::Preview);
                                }
                                if crate::icons::action(ui, crate::icons::Icon::Display, "Apply to system", true).clicked() {
                                    self.confirm = false;
                                    action = Some(Action::Apply);
                                }
                                if ui.button("Cancel").clicked() {
                                    self.confirm = false;
                                }
                            });
                        }
                    });
                });
                if let Some(reason) = blocked {
                    ui.small(reason);
                } else if needs_stop {
                    ui.small("Stop the engine and end Live preview before applying to Windows.");
                }
            });
        action
    }
}
