use super::{
    geometry::{self, Grab, Span},
    *,
};

/// Bands lie inside the visible panel, matching the coordinates used by the engine.
pub fn band(content: egui::Rect, side: Side) -> egui::Rect {
    let thickness = 12.0_f32
        .min(content.width() / 3.0)
        .min(content.height() / 3.0)
        .max(0.0);
    match side {
        Side::Left => egui::Rect::from_min_max(
            content.min,
            egui::pos2(content.left() + thickness, content.bottom()),
        ),
        Side::Right => egui::Rect::from_min_max(
            egui::pos2(content.right() - thickness, content.top()),
            content.max,
        ),
        Side::Top => egui::Rect::from_min_max(
            content.min,
            egui::pos2(content.right(), content.top() + thickness),
        ),
        Side::Bottom => egui::Rect::from_min_max(
            egui::pos2(content.left(), content.bottom() - thickness),
            content.max,
        ),
    }
}
pub fn segment(band: egui::Rect, side: Side, span: Span, length: f64) -> egui::Rect {
    let a = (span.from / length).clamp(0.0, 1.0) as f32;
    let b = (span.to / length).clamp(0.0, 1.0) as f32;
    if side.vertical() {
        egui::Rect::from_min_max(
            egui::pos2(band.left(), band.top() + a * band.height()),
            egui::pos2(band.right(), band.top() + b * band.height()),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(band.left() + a * band.width(), band.top()),
            egui::pos2(band.left() + b * band.width(), band.bottom()),
        )
    }
}
fn mm(at: egui::Pos2, band: egui::Rect, side: Side, length: f64) -> f64 {
    if side.vertical() {
        f64::from(at.y - band.top()) / f64::from(band.height()) * length
    } else {
        f64::from(at.x - band.left()) / f64::from(band.width()) * length
    }
}
#[derive(Clone)]
pub(super) struct Gesture {
    monitor: String,
    side: Side,
    expected: Vec<BorderSection>,
    grab: Grab,
    press: f64,
    anchor: f64,
    original: BorderSection,
    preview: Option<BorderSection>,
    /// Resizing the window invalidates the pointer's millimetre conversion.
    band: egui::Rect,
    length: f64,
}
impl Gesture {
    fn update(&mut self, raw: f64, length: f64, scale: f64, targets: &[f64], snap: bool) {
        let index = self.grab.index();
        let reference = if index.is_some() {
            (self.original.from() + self.original.to()) / 2.0
        } else {
            self.anchor
        };
        let free = geometry::gap(&self.expected, index, reference, length);
        let boundary = |v| geometry::snap(v, length, 8.0 / scale, targets, snap);
        let wanted = match self.grab {
            Grab::Draw => geometry::clamp(Span::new(self.anchor, boundary(raw)), free),
            Grab::From(_) => geometry::clamp(
                Span::new(
                    boundary(self.original.from() + raw - self.press),
                    self.original.to(),
                ),
                free,
            ),
            Grab::To(_) => geometry::clamp(
                Span::new(
                    self.original.from(),
                    boundary(self.original.to() + raw - self.press),
                ),
                free,
            ),
            Grab::Move(_) => {
                let from = self.original.from() + raw - self.press;
                let from = if snap {
                    geometry::snap_move(
                        from,
                        self.original.to() - self.original.from(),
                        8.0 / scale,
                        targets,
                    )
                } else {
                    from
                };
                geometry::shifted(Span::from(&self.original), from, free)
            }
        };
        // A sliver is not a new section, and an invalid resize retains the last valid span.
        if wanted.is_some() || self.grab == Grab::Draw {
            self.preview = wanted.map(|s| s.section(self.original));
        }
    }
    fn action(&self) -> Option<Action> {
        let section = self.preview?;
        let edit = if let Some(index) = self.grab.index() {
            if section == self.original {
                return None;
            }
            Edit::Replace { index, section }
        } else {
            if self.expected.len() >= geometry::MAX_SECTIONS {
                return None;
            }
            Edit::Insert(section)
        };
        Some(Action {
            monitor: self.monitor.clone(),
            side: self.side,
            expected: self.expected.clone(),
            edit,
        })
    }
}
#[derive(Default)]
pub struct Interaction {
    pub monitor: Option<String>,
    pub action: Option<Action>,
}
impl Editor {
    /// Preview stays local until release. Escape, focus loss, a changed layout or a
    /// changed projection discard it, so cancelled gestures never reach Save/Live.
    pub fn bands(
        &mut self,
        ui: &mut egui::Ui,
        monitors: &[Monitor],
        placements: &[(String, egui::Rect)],
    ) -> Interaction {
        let mut result = Interaction::default();
        if !ui.is_enabled() || ui.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused) {
            self.cancel();
        }
        if let Some(g) = &self.gesture {
            let valid = monitors
                .iter()
                .find(|m| m.id == g.monitor)
                .is_some_and(|m| {
                    g.side.sections(&m.resistance) == g.expected
                        && g.side.length(m.bounds) == g.length
                })
                && placements
                    .iter()
                    .find(|(id, _)| *id == g.monitor)
                    .is_some_and(|(_, r)| band(*r, g.side) == g.band);
            if !valid {
                self.cancel();
            }
        }
        for (id, content) in placements {
            let Some(monitor) = monitors.iter().find(|m| m.id == *id) else {
                continue;
            };
            for side in Side::ALL {
                let length = side.length(monitor.bounds);
                let rect = band(*content, side);
                let pixels = if side.vertical() {
                    rect.height()
                } else {
                    rect.width()
                };
                if !length.is_finite() || length <= 0.0 || pixels <= 0.0 {
                    continue;
                }
                let scale = f64::from(pixels) / length;
                let sections = side.sections(&monitor.resistance);
                let response = ui.interact(
                    rect,
                    ui.id().with(("resistance-band", id, side)),
                    egui::Sense::click_and_drag(),
                );
                ui.painter()
                    .rect_filled(rect, 0.0, egui::Color32::from_gray(50));
                let snap = !ui.input(|i| i.modifiers.ctrl);
                if response.clicked() || response.double_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let at = mm(pos, rect, side, length);
                        let grab = geometry::grab(sections, at, scale);
                        self.selected = grab.index().map(|index| Selection {
                            monitor: id.clone(),
                            side,
                            index,
                        });
                        self.side = side;
                        result.monitor = Some(id.clone());
                        if response.double_clicked() {
                            let span = geometry::gap(sections, grab.index(), at, length);
                            if span.length() >= geometry::MIN_LENGTH {
                                let edit = if let Some(index) = grab.index() {
                                    Edit::Replace {
                                        index,
                                        section: span.section(sections[index]),
                                    }
                                } else {
                                    Edit::Insert(span.section(BorderSection::new(
                                        0.0, 0.0, 0.0, true, 0.0, false,
                                    )))
                                };
                                result.action = Some(Action::new(monitor, side, edit));
                            }
                        }
                    }
                }
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                        let raw = mm(press, rect, side, length);
                        let grab = geometry::grab(sections, raw, scale);
                        let targets = geometry::targets(monitors, id, side, grab.index());
                        let original = grab
                            .index()
                            .map(|i| sections[i])
                            .unwrap_or_else(|| BorderSection::new(0.0, 0.0, 0.0, true, 0.0, false));
                        self.selected = grab.index().map(|index| Selection {
                            monitor: id.clone(),
                            side,
                            index,
                        });
                        self.side = side;
                        result.monitor = Some(id.clone());
                        self.gesture = Some(Gesture {
                            monitor: id.clone(),
                            side,
                            expected: sections.to_vec(),
                            grab,
                            press: raw,
                            anchor: geometry::snap(raw, length, 8.0 / scale, &targets, snap),
                            original,
                            preview: grab.index().map(|_| original),
                            band: rect,
                            length,
                        });
                    }
                }
                let mut preview = None;
                let mut replacing = None;
                if let Some(g) = self
                    .gesture
                    .as_mut()
                    .filter(|g| g.monitor == *id && g.side == side)
                {
                    let targets = geometry::targets(monitors, id, side, g.grab.index());
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        g.update(mm(pos, rect, side, length), length, scale, &targets, snap);
                    }
                    preview = g.preview;
                    replacing = g.grab.index();
                    if snap {
                        if let Some(s) = preview {
                            for boundary in [s.from(), s.to()] {
                                if targets.iter().any(|v| (*v - boundary).abs() < 0.001) {
                                    let r = segment(
                                        rect,
                                        side,
                                        Span {
                                            from: boundary,
                                            to: boundary,
                                        },
                                        length,
                                    );
                                    let area = ui.max_rect();
                                    let line = if side.vertical() {
                                        [
                                            egui::pos2(area.left(), r.top()),
                                            egui::pos2(area.right(), r.top()),
                                        ]
                                    } else {
                                        [
                                            egui::pos2(r.left(), area.top()),
                                            egui::pos2(r.left(), area.bottom()),
                                        ]
                                    };
                                    ui.painter().line_segment(
                                        line,
                                        egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE),
                                    );
                                }
                            }
                        }
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        result.action = g.action();
                    }
                }
                for (index, s) in sections.iter().enumerate() {
                    if replacing == Some(index) {
                        continue;
                    }
                    let selected = self
                        .selected
                        .as_ref()
                        .is_some_and(|k| k.monitor == *id && k.side == side && k.index == index);
                    paint(ui, rect, side, length, scale, s, selected);
                }
                if let Some(s) = preview {
                    paint(ui, rect, side, length, scale, &s, true);
                }
                if let Some(at) = response.hover_pos() {
                    let value = mm(at, rect, side, length);
                    let tip = geometry::grab(sections, value, scale)
                        .index()
                        .map(|i| {
                            format!(
                                "{} · {:.1}–{:.1} mm\n{}",
                                side.label(),
                                sections[i].from(),
                                sections[i].to(),
                                description(&sections[i])
                            )
                        })
                        .unwrap_or_else(|| format!("{} · drag to draw a section", side.label()));
                    response.on_hover_text(tip);
                }
            }
        }
        if ui.input(|i| !i.pointer.primary_down()) {
            self.cancel();
        }
        if ui.is_enabled()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
        {
            if let Some(s) = &self.selected {
                if let Some(m) = monitors.iter().find(|m| m.id == s.monitor) {
                    result.action = Some(Action::new(m, s.side, Edit::Delete(s.index)));
                }
            }
        }
        result
    }
}
fn paint(
    ui: &egui::Ui,
    rect: egui::Rect,
    side: Side,
    length: f64,
    scale: f64,
    s: &BorderSection,
    selected: bool,
) {
    let drawn = segment(rect, side, Span::from(s), length);
    ui.painter().rect_filled(drawn, 0.0, color(s));
    if selected {
        ui.painter().rect_stroke(
            drawn,
            0.0,
            egui::Stroke::new(1.5, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
    let grip = geometry::grip(s, scale);
    for span in [
        Span {
            from: s.from(),
            to: s.from() + grip,
        },
        Span {
            from: s.to() - grip,
            to: s.to(),
        },
    ] {
        ui.painter().rect_filled(
            segment(rect, side, span, length).shrink(2.0),
            0.0,
            egui::Color32::from_white_alpha(100),
        );
    }
}
