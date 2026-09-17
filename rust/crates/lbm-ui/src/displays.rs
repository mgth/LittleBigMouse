//! System outputs stay reachable even when detached from the desktop/map.
use lbm_ipc::displays::{Change, Outputs};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Did {
    Refresh,
    Stop,
    Change(String, Change),
}

#[derive(Default)]
pub struct Panel {
    pub confirm: Option<(String, Change)>,
}

pub fn caption(change: Change) -> &'static str {
    match change {
        Change::Primary => "Make primary",
        Change::Attach => "Attach",
        Change::Detach => "Detach",
    }
}

impl Panel {
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        outputs: Option<&Outputs>,
        blocked: Option<&str>,
        busy: bool,
        warn: bool,
        can_stop: bool,
    ) -> Option<Did> {
        let mut did = None;
        ui.heading("System displays");
        ui.label("These actions change the desktop configuration immediately.");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh outputs"))
                .clicked()
            {
                did = Some(Did::Refresh);
            }
            if can_stop
                && ui
                    .add_enabled(!busy, egui::Button::new("Stop engine"))
                    .clicked()
            {
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
        egui::ScrollArea::vertical().show(ui, |ui| {
            for output in &outputs.outputs {
                ui.push_id(&output.connector, |ui| {
                    ui.group(|ui| {
                        ui.label(format!("{} · {}", output.name, output.connector));
                        ui.label(if !output.enabled {
                            "Detached"
                        } else if output.primary {
                            "Primary"
                        } else {
                            "Attached"
                        });
                        ui.horizontal(|ui| {
                            for (change, available) in [
                                (Change::Primary, output.enabled && !output.primary),
                                (Change::Attach, !output.enabled),
                                (Change::Detach, output.enabled && enabled > 1),
                            ] {
                                if ui
                                    .add_enabled(
                                        available && blocked.is_none() && !busy,
                                        egui::Button::new(caption(change)),
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
                ui.horizontal(|ui| {
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
