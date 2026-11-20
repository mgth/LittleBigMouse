//! Fixed family strip and compact parameters, above the Display Lab controls.
use lbm_pattern::{Pattern, Settings};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Family {
    #[default]
    Levels,
    Gray,
    Colours,
    Sharpness,
    Geometry,
    Gamma,
}
impl Family {
    pub const ALL: [Self; 6] = [
        Self::Levels,
        Self::Gray,
        Self::Colours,
        Self::Sharpness,
        Self::Geometry,
        Self::Gamma,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Levels => "Levels",
            Self::Gray => "Gray",
            Self::Colours => "Colours",
            Self::Sharpness => "Sharpness",
            Self::Geometry => "Geometry",
            Self::Gamma => "Gamma",
        }
    }
    pub fn patterns(self) -> &'static [Pattern] {
        match self {
            Self::Levels => &[Pattern::Contrast, Pattern::Blacks, Pattern::Whites],
            Self::Gray => &[Pattern::GraySteps, Pattern::GrayGradient, Pattern::Solid],
            Self::Colours => &[Pattern::RgbClip, Pattern::Gradient, Pattern::Solid],
            Self::Sharpness => &[Pattern::Sharpness],
            Self::Geometry => &[Pattern::Geometry, Pattern::White, Pattern::Orange],
            Self::Gamma => &[Pattern::Gamma],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Target {
    pub connector: String,
    pub name: String,
}

#[derive(Default)]
pub struct Strip {
    thumbnails: Vec<(Family, egui::TextureHandle)>,
    previous: Option<(Family, Settings)>,
    pub settings: Settings,
    pub family: Family,
    pub matching: bool,
    pub compared: Vec<String>,
    pub reference: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Show,
    Hide,
    Update,
}

impl Strip {
    /// Reference and compared outputs are independent of the VCP selection.
    pub fn targets(&self, available: &[Target], selected: Option<&str>) -> Vec<String> {
        available
            .iter()
            .filter(|target| {
                if self.matching {
                    self.compared.contains(&target.connector)
                        || self.reference.as_deref() == Some(&target.connector)
                } else {
                    selected == Some(target.connector.as_str())
                }
            })
            .map(|target| target.connector.clone())
            .collect()
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        active: Option<Pattern>,
        ready: bool,
        blocked: Option<&str>,
        available: &[Target],
        selected: Option<&str>,
    ) -> Option<Action> {
        let mut action = None;
        let mut changed = false;
        ui.add_space(5.);
        ui.horizontal(|ui| {
            ui.strong("Test patterns");
            if ui
                .add_enabled(blocked.is_none(), egui::Button::new("Show").small())
                .clicked()
            {
                action = Some(Action::Show);
            }
            if ui
                .add_enabled(active.is_some(), egui::Button::new("Hide").small())
                .clicked()
            {
                action = Some(Action::Hide);
            }
        });
        if self.thumbnails.is_empty() {
            self.thumbnails = Family::ALL
                .into_iter()
                .map(|family| {
                    let pixels =
                        lbm_pattern::render(family.patterns()[0], 120, 80).expect("thumbnail size");
                    let texture = ui.ctx().load_texture(
                        format!("pattern-family-{}", family.label()),
                        egui::ColorImage::from_rgba_unmultiplied([120, 80], &pixels),
                        egui::TextureOptions::LINEAR,
                    );
                    (family, texture)
                })
                .collect();
        }
        let width = ((ui.available_width() - 20.) / 6.).clamp(24., 55.);
        ui.add_enabled_ui(blocked.is_none(), |ui| {
            let previous_matching = self.matching;
            ui.horizontal(|ui| {
                changed |= ui
                    .selectable_value(&mut self.matching, false, "Selected screen")
                    .changed();
                if ui
                    .add_enabled(
                        available.len() >= 2,
                        egui::Button::new("Compare screens").selected(self.matching),
                    )
                    .clicked()
                {
                    changed |= !self.matching;
                    self.matching = true;
                }
            });
            if self.matching && !previous_matching {
                self.previous = Some((self.family, self.settings.clone()));
                self.compared = available.iter().map(|t| t.connector.clone()).collect();
                self.reference = selected
                    .map(str::to_owned)
                    .or_else(|| self.compared.first().cloned());
                self.family = Family::Gray;
                self.settings = Settings {
                    pattern: Pattern::Solid,
                    colour: [255; 3],
                    level: 128,
                    ..Default::default()
                };
                action = Some(Action::Show);
            } else if !self.matching && previous_matching {
                if let Some((family, settings)) = self.previous.take() {
                    self.family = family;
                    self.settings = settings;
                }
            }
            if self.matching {
                if !available
                    .iter()
                    .any(|t| Some(&t.connector) == self.reference.as_ref())
                {
                    self.reference = available.first().map(|t| t.connector.clone());
                }
                ui.small(
                    "Compare the same gray on several screens, then adjust each screen below.",
                );
                ui.horizontal(|ui| {
                    ui.label("Reference");
                    let name = available
                        .iter()
                        .find(|t| Some(&t.connector) == self.reference.as_ref())
                        .map(|t| t.name.as_str())
                        .unwrap_or("Select");
                    egui::ComboBox::from_id_salt("pattern-reference")
                        .width((ui.available_width() - 8.).max(80.))
                        .selected_text(name)
                        .truncate()
                        .show_ui(ui, |ui| {
                            for target in available {
                                changed |= ui
                                    .selectable_value(
                                        &mut self.reference,
                                        Some(target.connector.clone()),
                                        &target.name,
                                    )
                                    .changed();
                            }
                        });
                });
                ui.small("Screens to compare with the reference");
                for target in available {
                    if self.reference.as_ref() == Some(&target.connector) {
                        continue;
                    }
                    let mut checked = self.compared.contains(&target.connector);
                    let last = checked && self.targets(available, selected).len() <= 2;
                    if ui
                        .add_enabled(!last, egui::Checkbox::new(&mut checked, &target.name))
                        .changed()
                    {
                        self.compared.retain(|c| c != &target.connector);
                        if checked {
                            self.compared.push(target.connector.clone());
                        }
                        changed = true;
                    }
                }
                ui.add_space(4.);
                ui.strong("Shared gray");
            }
            if !self.matching {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.;
                    for (family, texture) in &self.thumbnails {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(width, 43.), egui::Sense::click());
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                family.label(),
                            )
                        });
                        ui.painter().rect_filled(
                            rect,
                            4.,
                            if *family == self.family {
                                crate::theme::ACCENT
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            },
                        );
                        let image = egui::Rect::from_center_size(
                            egui::pos2(rect.center().x, rect.top() + 15.),
                            egui::vec2((width - 6.).min(36.), (width - 6.).min(36.) * 2. / 3.),
                        );
                        ui.painter().image(
                            texture.id(),
                            image,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                            egui::Color32::WHITE,
                        );
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - 3.),
                            egui::Align2::CENTER_BOTTOM,
                            family.label(),
                            egui::FontId::proportional(8.5),
                            if *family == self.family {
                                egui::Color32::BLACK
                            } else {
                                ui.visuals().text_color()
                            },
                        );
                        if response.on_hover_text(family.label()).clicked() {
                            if self.family == *family && active == Some(self.settings.pattern) {
                                action = Some(Action::Hide);
                            } else {
                                self.family = *family;
                                self.settings.pattern = family.patterns()[0];
                                if *family == Family::Gray {
                                    self.settings.colour = [255; 3];
                                }
                                action = Some(Action::Show);
                            }
                        }
                    }
                });
            }
            if !self.matching {
                let before = self.settings.pattern;
                egui::ComboBox::from_id_salt("pattern-variant")
                    .width(ui.available_width())
                    .selected_text(self.settings.pattern.label())
                    .show_ui(ui, |ui| {
                        for pattern in self.family.patterns() {
                            ui.selectable_value(
                                &mut self.settings.pattern,
                                *pattern,
                                pattern.label(),
                            );
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::ComboBox,
                            ui.is_enabled(),
                            "Pattern variant",
                        )
                    });
                changed |= self.settings.pattern != before;
            }
            match self.settings.pattern {
                Pattern::Contrast | Pattern::Whites | Pattern::Blacks | Pattern::RgbClip => {
                    changed |= ui
                        .checkbox(&mut self.settings.blink, "Alternate patches")
                        .on_hover_text(
                            "Alternate with the background every 0.7 s. Numbers remain visible.",
                        )
                        .changed();
                }
                Pattern::GraySteps => {
                    changed |= ui
                        .add(egui::Slider::new(&mut self.settings.steps, 2..=32).text("Steps"))
                        .changed();
                }
                Pattern::Solid => {
                    ui.horizontal_wrapped(|ui| {
                        for (label, colour) in [
                            ("Gray", [255; 3]),
                            ("R", [255, 0, 0]),
                            ("G", [0, 255, 0]),
                            ("B", [0, 0, 255]),
                        ] {
                            if self.family != Family::Colours || self.matching {
                                continue;
                            }
                            if ui
                                .selectable_label(self.settings.colour == colour, label)
                                .clicked()
                            {
                                self.settings.colour = colour;
                                changed = true;
                            }
                        }
                        if ui.small_button("Black").clicked() {
                            self.settings.colour = [255; 3];
                            self.settings.level = 0;
                            changed = true;
                        }
                        if ui.small_button("White").clicked() {
                            self.settings.colour = [255; 3];
                            self.settings.level = 255;
                            changed = true;
                        }
                        for (label, level) in [("25%", 64), ("50%", 128), ("75%", 191)] {
                            if ui
                                .small_button(label)
                                .on_hover_text(
                                    "Percentage of the 8-bit signal code, not measured luminance",
                                )
                                .clicked()
                            {
                                self.settings.level = level;
                                changed = true;
                            }
                        }
                    });
                    if self.family == Family::Colours && !self.matching {
                        ui.horizontal(|ui| {
                            ui.label("Colour");
                            changed |= ui
                                .color_edit_button_srgb(&mut self.settings.colour)
                                .changed();
                        });
                    }
                    changed |= ui
                        .add(egui::Slider::new(&mut self.settings.level, 0..=255).text("Level"))
                        .changed();
                }
                Pattern::Sharpness => {
                    ui.horizontal(|ui| {
                        ui.label("Detail");
                        for width in 1..=3 {
                            changed |= ui
                                .selectable_value(
                                    &mut self.settings.line_width,
                                    width,
                                    format!("{width} px"),
                                )
                                .changed();
                        }
                    });
                    changed |= ui
                        .checkbox(&mut self.settings.invert, "Invert black / white")
                        .changed();
                }
                _ => {}
            }
        });
        if changed && active.is_some() && action.is_none() {
            action = Some(Action::Update);
        }
        if let Some(reason) = blocked {
            ui.small(reason);
        } else if self.matching {
            if let Some(target) = available
                .iter()
                .find(|t| selected == Some(t.connector.as_str()))
            {
                ui.label(
                    egui::RichText::new(format!("Adjusting: {}", target.name))
                        .color(crate::theme::ACCENT),
                );
            }
            ui.small("Change the selected screen above to adjust another. Only LittleBigMouse stays above the patterns.");
        } else {
            ui.small(self.settings.pattern.description());
            ui.small("LittleBigMouse stays above the pattern. Esc or Hide to close.");
        }
        if active.is_some() && !ready {
            ui.small("Updating patterns…");
        }
        ui.separator();
        action
    }
}
