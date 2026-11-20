//! System commands for the selected monitor and for every detached output, which no
//! map or selector can reach. Guards use the entire output inventory.
use crate::icons::{self, Icon};
use lbm_ipc::displays::{Change, Outputs};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Did {
    Refresh,
    Stop,
    Change(String, Change),
}

#[derive(Default)]
pub struct Panel {
    pub selected: Option<String>,
    pub confirm: Option<(String, Change)>,
}

fn icon(change: Change) -> Icon {
    match change {
        Change::Primary => Icon::Star,
        Change::Attach => Icon::Attach,
        Change::Detach => Icon::Detach,
    }
}

pub fn caption(change: Change) -> &'static str {
    match change {
        Change::Primary => "Make primary",
        Change::Attach => "Attach",
        Change::Detach => "Detach",
    }
}

impl Panel {
    /// A detached output is not on the map, so it stays offered without any selection.
    pub fn wanted(&self, outputs: Option<&Outputs>) -> bool {
        self.selected.is_some()
            || outputs.is_some_and(|outputs| outputs.outputs.iter().any(|o| !o.enabled))
    }

    fn shows(&self, output: &lbm_ipc::displays::Output) -> bool {
        !output.enabled || self.selected.as_ref() == Some(&output.connector)
    }

    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        outputs: Option<&Outputs>,
        blocked: Option<&str>,
        busy: bool,
        warn: bool,
        can_stop: bool,
    ) -> Option<Did> {
        let confirmable = self.confirm.as_ref().is_none_or(|(connector, _)| {
            outputs.is_some_and(|outputs| {
                outputs
                    .outputs
                    .iter()
                    .any(|o| &o.connector == connector && self.shows(o))
            })
        });
        if !confirmable {
            self.confirm = None;
        }
        if !self.wanted(outputs) {
            return None;
        }
        let mut did = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
            icons::paint(ui.painter(), rect, Icon::Display, ui.visuals().text_color());
            ui.heading("System display");
        });
        ui.label("These actions change the desktop configuration immediately.");
        ui.horizontal_wrapped(|ui| {
            if icons::action(ui, Icon::Refresh, "Refresh outputs", !busy).clicked() {
                did = Some(Did::Refresh);
            }
            if can_stop && icons::action(ui, Icon::Stop, "Stop engine", !busy).clicked() {
                did = Some(Did::Stop);
            }
            if busy {
                ui.spinner();
                ui.label("Waiting for the agent…");
            }
        });
        if let Some(reason) = blocked {
            ui.label(reason);
            self.confirm = None;
        }
        let Some(outputs) = outputs else {
            return did;
        };
        let enabled = outputs.outputs.iter().filter(|o| o.enabled).count();
        ui.vertical(|ui| {
            let shown: Vec<_> = outputs.outputs.iter().filter(|o| self.shows(o)).collect();
            for output in shown {
                ui.push_id(&output.connector, |ui| {
                    ui.group(|ui| {
                        icons::wrapped_title(
                            ui,
                            Icon::Display,
                            &format!("{} · {}", output.name, output.connector),
                        );
                        let (state, color) = if !output.enabled {
                            ("Detached", crate::theme::MUTED)
                        } else if output.primary {
                            ("Primary", crate::theme::ACCENT)
                        } else {
                            ("Attached", crate::theme::ACCENT)
                        };
                        crate::theme::status(ui, state, color);
                        ui.horizontal_wrapped(|ui| {
                            for (change, available) in [
                                (Change::Primary, output.enabled && !output.primary),
                                (Change::Attach, !output.enabled),
                                (Change::Detach, output.enabled && enabled > 1),
                            ] {
                                if icons::action(
                                    ui,
                                    icon(change),
                                    caption(change),
                                    available && blocked.is_none() && !busy,
                                )
                                .clicked()
                                {
                                    if warn {
                                        self.confirm = Some((output.connector.clone(), change));
                                    } else {
                                        did = Some(Did::Change(output.connector.clone(), change));
                                    }
                                }
                            }
                        });
                        if output.enabled && enabled == 1 {
                            ui.label("The last enabled screen cannot be detached.");
                        }
                    });
                });
            }
            if let Some((connector, change)) = self.confirm.clone() {
                ui.separator();
                ui.label(format!("{} {connector}?", caption(change)));
                ui.label(match change {
                    Change::Detach => "This screen will turn off. Use Attach to bring it back.",
                    Change::Attach => "This screen will join the desktop.",
                    Change::Primary => "This screen will become the primary display.",
                });
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !busy && blocked.is_none(),
                            egui::Button::new("Confirm change"),
                        )
                        .clicked()
                    {
                        did = Some(Did::Change(connector, change));
                        self.confirm = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm = None;
                    }
                });
            }
        });
        did
    }
}
