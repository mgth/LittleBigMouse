//! Compact network remotes in the monitor properties, with pairing kept alongside.
use crate::theme;
use egui::{vec2, RichText};
use lbm_ipc::remote::{Action, Brand, Command, Config, State};

pub enum Did {
    Command(Command),
    Brand(Brand),
}
pub struct Panel {
    pub brand: Brand,
    ip: String,
    mac: String,
    certificate: String,
    controller: String,
    uuid: String,
    pin: String,
    sequence: String,
    setup: bool,
    menu_id: i32,
    menu_value: i32,
    platform: String,
    level: i32,
    volume: Option<i32>,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            brand: Brand::Samsung,
            ip: String::new(),
            mac: String::new(),
            certificate: String::new(),
            controller: String::new(),
            uuid: String::new(),
            pin: String::new(),
            sequence: String::new(),
            setup: true,
            menu_id: 1,
            menu_value: 0,
            platform: "changelaserluminance".into(),
            level: 5,
            volume: None,
        }
    }
}
impl Panel {
    pub fn observed(&mut self, state: &State) {
        self.volume = state.volume;
    }
    pub fn load(&mut self, state: &State) {
        let c = state.config(self.brand);
        self.ip = c.ip_address.clone();
        self.mac = c.mac_address.clone();
        self.certificate = c.certificate_path.clone();
        self.controller = c.controller_mac.clone();
        self.uuid = c.device_uuid.clone();
        self.sequence = c.r#macro.clone();
        self.setup = !c.paired;
        self.pin.clear();
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: Option<&State>,
        busy: bool,
        error: Option<&str>,
    ) -> Option<Did> {
        let mut action = None;
        let mut switch = None;
        ui.horizontal_wrapped(|ui| {
            for brand in [Brand::Samsung, Brand::Hisense] {
                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new(brand.label()).selected(self.brand == brand),
                    )
                    .clicked()
                    && brand != self.brand
                {
                    switch = Some(brand);
                }
            }
        });
        if let Some(brand) = switch {
            return Some(Did::Brand(brand));
        }
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Contacting remote…");
            });
        }
        if let Some(e) = error {
            ui.colored_label(theme::AMBER, e);
            if ui
                .add_enabled(!busy, egui::Button::new("Reload association"))
                .clicked()
            {
                action = Some(Action::Info);
            }
        }
        let empty = Config::default();
        let c = state.map(|s| s.config(self.brand)).unwrap_or(&empty);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if c.paired {
                    "Paired · local network"
                } else {
                    "Network remote · no DDC/CI required"
                })
                .small()
                .color(if c.paired {
                    theme::ACCENT
                } else {
                    theme::MUTED
                }),
            );
        });
        if c.paired {
            ui.horizontal_wrapped(|ui| {
                ui.small(&c.ip_address);
                if ui
                    .add_enabled(!busy, egui::Button::new("Connect").small())
                    .clicked()
                {
                    action = Some(Action::Connect);
                }
                if ui.small_button("Connection settings").clicked() {
                    self.setup = !self.setup;
                }
            });
        }
        ui.add_enabled_ui(!busy, |ui| {
            if self.setup {
                theme::card().show(ui, |ui| {
                    ui.strong("Associate with this screen");
                    field(ui, "Device IPv4 address", &mut self.ip, "192.168.1.20");
                    if ui.button("Discover devices").clicked() {
                        action = Some(Action::Discover);
                    }
                    if let Some(s) = state {
                        for d in &s.devices {
                            if ui
                                .selectable_label(
                                    self.ip == d.ip_address,
                                    format!("{} · {}", d.name, d.ip_address),
                                )
                                .clicked()
                            {
                                self.ip = d.ip_address.clone();
                                self.mac = d.mac_address.clone();
                            }
                        }
                    }
                    field(
                        ui,
                        "Device MAC · for Wake-on-LAN",
                        &mut self.mac,
                        "AA:BB:CC:DD:EE:FF",
                    );
                    if self.brand == Brand::Hisense {
                        field(
                            ui,
                            "VIDAA certificate (.p12)",
                            &mut self.certificate,
                            "Default certificate if left empty",
                        );
                        field(
                            ui,
                            "Controller MAC · this computer",
                            &mut self.controller,
                            "Detected automatically on Linux",
                        );
                    }
                    if ui.button("Save address").clicked() {
                        action = Some(Action::Save {
                            ip_address: self.ip.clone(),
                            mac_address: self.mac.clone(),
                            certificate_path: self.certificate.clone(),
                            device_uuid: self.uuid.clone(),
                            controller_mac: self.controller.clone(),
                        });
                    }
                    if self.brand == Brand::Samsung {
                        if ui.button("Pair Samsung").clicked() {
                            action = Some(Action::Pair {
                                ip_address: self.ip.clone(),
                                mac_address: self.mac.clone(),
                                pin: String::new(),
                            });
                        }
                        ui.weak("Accept LittleBigMouse on the Samsung screen.");
                    } else {
                        // The agent talks to the saved display: the form must be showing it.
                        let saved = !c.ip_address.is_empty() && self.ip.trim() == c.ip_address;
                        if !saved {
                            ui.weak("Save this address before pairing.");
                        }
                        if ui
                            .add_enabled(saved, egui::Button::new("Request VIDAA PIN"))
                            .clicked()
                        {
                            action = Some(Action::RequestPin {
                                ip_address: self.ip.trim().to_owned(),
                            });
                        }
                        field(ui, "PIN displayed on the screen", &mut self.pin, "4 digits");
                        if ui
                            .add_enabled(
                                saved && self.pin.len() == 4,
                                egui::Button::new("Pair VIDAA"),
                            )
                            .clicked()
                        {
                            action = Some(Action::Pair {
                                ip_address: self.ip.clone(),
                                mac_address: self.mac.clone(),
                                pin: self.pin.clone(),
                            });
                        }
                    }
                });
            }
            egui::Frame::new()
                .fill(theme::BACKGROUND)
                .corner_radius(24)
                .inner_margin(14)
                .stroke(egui::Stroke::new(1., theme::BORDER))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(self.brand.label()).strong().size(14.));
                        ui.add_space(4.);
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    !c.mac_address.is_empty(),
                                    egui::Button::new("Wake").fill(theme::SELECTED),
                                )
                                .clicked()
                            {
                                action = Some(Action::Wake);
                            }
                            if ui
                                .add_enabled(
                                    c.paired,
                                    egui::Button::new("Power")
                                        .fill(egui::Color32::from_rgb(79, 37, 45)),
                                )
                                .clicked()
                            {
                                action = Some(Action::Key {
                                    key: if self.brand == Brand::Samsung {
                                        "KEY_POWEROFF"
                                    } else {
                                        "KEY_POWER"
                                    }
                                    .into(),
                                });
                            }
                        });
                        ui.add_enabled_ui(c.paired, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                key_button(ui, "Home", "KEY_HOME", &mut action);
                                key_button(ui, "Menu", "KEY_MENU", &mut action);
                                key_button(ui, "Source", "KEY_SOURCE", &mut action);
                            });
                            ui.add_space(6.);
                            ui.horizontal(|ui| {
                                ui.add_space(((ui.available_width() - 140.) * 0.5).max(0.));
                                egui::Grid::new("remote-pad").spacing(vec2(4., 4.)).show(
                                    ui,
                                    |ui| {
                                        ui.allocate_space(vec2(44., 44.));
                                        arrow(ui, 0, "Up", "KEY_UP", &mut action);
                                        ui.end_row();
                                        arrow(ui, 3, "Left", "KEY_LEFT", &mut action);
                                        if ui
                                            .add_sized(
                                                [44., 44.],
                                                egui::Button::new("OK")
                                                    .fill(theme::SELECTED)
                                                    .corner_radius(22),
                                            )
                                            .clicked()
                                        {
                                            action = Some(Action::Key {
                                                key: "KEY_ENTER".into(),
                                            });
                                        }
                                        arrow(ui, 1, "Right", "KEY_RIGHT", &mut action);
                                        ui.end_row();
                                        ui.allocate_space(vec2(44., 44.));
                                        arrow(ui, 2, "Down", "KEY_DOWN", &mut action);
                                        ui.end_row();
                                    },
                                );
                            });
                            ui.add_space(6.);
                            ui.horizontal_wrapped(|ui| {
                                key_button(ui, "Back", "KEY_RETURN", &mut action);
                                key_button(ui, "Exit", "KEY_EXIT", &mut action);
                            });
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                key_button(ui, "Vol −", "KEY_VOLDOWN", &mut action);
                                key_button(ui, "Mute", "KEY_MUTE", &mut action);
                                key_button(ui, "Vol +", "KEY_VOLUP", &mut action);
                            });
                            ui.horizontal_wrapped(|ui| {
                                media_button(ui, "Rewind", "KEY_REWIND", 0, &mut action);
                                media_button(ui, "Play / pause", "KEY_PLAYPAUSE", 1, &mut action);
                                media_button(ui, "Forward", "KEY_FF", 2, &mut action);
                                media_button(ui, "Stop", "KEY_STOP", 3, &mut action);
                            });
                            ui.horizontal_wrapped(|ui| {
                                key_button(ui, "Ch −", "KEY_CHDOWN", &mut action);
                                key_button(ui, "Ch +", "KEY_CHUP", &mut action);
                            });
                        });
                    });
                });
            if let Some(note) = state.and_then(|s| s.note.as_deref()) {
                ui.label(RichText::new(note).small().color(theme::MUTED));
            }
            ui.add_enabled_ui(c.paired, |ui| {
                if self.brand == Brand::Hisense {
                    if let Some(v) = self.volume.as_mut() {
                        let response = ui.add(egui::Slider::new(v, 0..=100).text("Volume"));
                        if response.drag_stopped() || (response.changed() && !response.dragged()) {
                            action = Some(Action::VolumeSet { value: *v });
                        }
                    }
                    if ui.button("Read volume").clicked() {
                        action = Some(Action::VolumeRead);
                    }
                }
                egui::CollapsingHeader::new("Macros and picture settings").show(ui, |ui| {
                    field(
                        ui,
                        "Remote key sequence",
                        &mut self.sequence,
                        "KEY_MENU,700,KEY_DOWN,KEY_ENTER",
                    );
                    if ui.button("Run macro").clicked() {
                        action = Some(Action::Macro {
                            sequence: self.sequence.clone(),
                        });
                    }
                    if self.brand == Brand::Hisense {
                        ui.separator();
                        if ui.button("Read VIDAA picture settings").clicked() {
                            action = Some(Action::PictureRead);
                        }
                        if let Some(s) = state {
                            for setting in &s.picture_settings {
                                if ui
                                    .selectable_label(
                                        self.menu_id == setting.menu_id,
                                        format!(
                                            "{} · {} = {}",
                                            setting.menu_id, setting.name, setting.value
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.menu_id = setting.menu_id;
                                    if let Ok(v) = setting.value.parse() {
                                        self.menu_value = v;
                                    }
                                }
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.label("Menu ID");
                            ui.add(egui::DragValue::new(&mut self.menu_id).range(1..=999));
                            ui.label("Value");
                            ui.add(egui::DragValue::new(&mut self.menu_value).range(-1000..=1000));
                        });
                        if ui.button("Apply picture value").clicked() {
                            action = Some(Action::PictureSet {
                                menu_id: self.menu_id,
                                value: self.menu_value,
                            });
                        }
                        egui::CollapsingHeader::new("Experimental laser control").show(ui, |ui| {
                            field(
                                ui,
                                "Platform action",
                                &mut self.platform,
                                "changelaserluminance",
                            );
                            ui.add(egui::Slider::new(&mut self.level, 0..=10).text("Level"));
                            if ui.button("Send laser level").clicked() {
                                action = Some(Action::Platform {
                                    name: self.platform.clone(),
                                    value: self.level,
                                });
                            }
                        });
                    }
                });
            });
        });
        action.map(|action| {
            Did::Command(Command {
                brand: self.brand,
                action,
            })
        })
    }
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).small().color(theme::MUTED));
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(f32::INFINITY)
            .hint_text(hint),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label)
    });
}
fn key_button(ui: &mut egui::Ui, label: &str, key: &str, action: &mut Option<Action>) {
    if ui.button(label).clicked() {
        *action = Some(Action::Key { key: key.into() });
    }
}
fn arrow(ui: &mut egui::Ui, rotation: u8, label: &str, key: &str, action: &mut Option<Action>) {
    let response = ui.add_sized([44., 44.], egui::Button::new("").corner_radius(22));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let c = response.rect.center();
    let transform = |x: f32, y: f32| {
        let (x, y) = match rotation {
            1 => (-y, x),
            2 => (-x, -y),
            3 => (y, -x),
            _ => (x, y),
        };
        c + vec2(x, y)
    };
    ui.painter().add(egui::Shape::line(
        vec![transform(-6., 3.), transform(0., -3.), transform(6., 3.)],
        egui::Stroke::new(
            1.8,
            if ui.is_enabled() {
                theme::TEXT
            } else {
                theme::MUTED.gamma_multiply(0.4)
            },
        ),
    ));
    if response.clicked() {
        *action = Some(Action::Key { key: key.into() });
    }
}

fn media_button(ui: &mut egui::Ui, label: &str, key: &str, kind: u8, action: &mut Option<Action>) {
    let r = ui.add_sized([42., 30.], egui::Button::new(""));
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    let c = r.rect.center();
    let color = if ui.is_enabled() {
        theme::TEXT
    } else {
        theme::MUTED.gamma_multiply(0.4)
    };
    let triangle = |x: f32, reverse: bool| {
        let sign = if reverse { -1. } else { 1. };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + vec2(x - 3. * sign, -5.),
                c + vec2(x - 3. * sign, 5.),
                c + vec2(x + 4. * sign, 0.),
            ],
            color,
            egui::Stroke::NONE,
        ));
    };
    match kind {
        0 | 2 => {
            triangle(-4., kind == 0);
            triangle(4., kind == 0);
        }
        1 => {
            triangle(-4., false);
            ui.painter().line_segment(
                [c + vec2(5., -5.), c + vec2(5., 5.)],
                egui::Stroke::new(2., color),
            );
        }
        _ => {
            ui.painter()
                .rect_filled(egui::Rect::from_center_size(c, vec2(9., 9.)), 1., color);
        }
    }
    if r.clicked() {
        *action = Some(Action::Key { key: key.into() });
    }
}
