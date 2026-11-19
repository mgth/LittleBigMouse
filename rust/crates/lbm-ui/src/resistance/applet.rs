//! A stable, section-anchored editor. Values always come from the same model as
//! the properties panel; locks never overwrite the remembered resistance.
use super::{band, segment, Action, Edit, Editor, Monitor, Selection, Side, Span};
use crate::{icons, theme};
use egui::{pos2, vec2, FontId, Rect, Stroke};
use lbm_layout::model::{BorderSection, ResistanceDefaults, ResistanceMode};

#[derive(Default)]
pub(super) struct State {
    selection: Option<Selection>,
    collapsed: bool,
}

impl Editor {
    pub(super) fn applet(
        &mut self,
        ui: &mut egui::Ui,
        monitors: &[Monitor],
        placements: &[(String, Rect)],
    ) -> Option<Action> {
        let selected = self.selected.as_ref()?;
        let monitor = monitors.iter().find(|m| m.id == selected.monitor)?;
        let original = *selected
            .side
            .sections(&monitor.resistance)
            .get(selected.index)?;
        let content = placements.iter().find(|(id, _)| *id == selected.monitor)?.1;
        let length = selected.side.length(monitor.bounds);
        if !length.is_finite() || length <= 0. || !content.is_positive() {
            return None;
        }
        if self.applet.selection.as_ref() != Some(selected) {
            self.applet.selection = Some(selected.clone());
            self.applet.collapsed = content.width() < 266. || content.height() < 160.;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.applet.collapsed = true;
        }
        let bounds = ui.clip_rect().intersect(ui.max_rect()).shrink(4.);
        if bounds.width() < 100. || bounds.height() < 40. {
            return None;
        }
        let compact = self.applet.collapsed || bounds.width() < 244. || bounds.height() < 166.;
        let size = if compact {
            vec2(202., 34.)
        } else {
            vec2(248., 152.)
        };
        let section = segment(
            band(content, selected.side),
            selected.side,
            Span::from(&original),
            length,
        );
        let anchor = section.center();
        let rect = placement(bounds, content, section, selected.side, size);
        let mut edited = original;
        let mut changed = false;
        let id = egui::Id::new((
            "resistance-applet",
            &selected.monitor,
            selected.side,
            selected.index,
        ));
        let area = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .movable(false)
            .constrain_to(bounds)
            .show(ui.ctx(), |popup| {
                if !ui.is_enabled() {
                    popup.disable();
                }
                popup.style_mut().spacing.item_spacing = vec2(5., 3.);
                popup.style_mut().spacing.button_padding = vec2(5., 2.);
                popup.style_mut().spacing.interact_size.y = 20.;
                popup.style_mut().override_font_id = Some(FontId::proportional(12.));
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1., theme::BORDER))
                    .corner_radius(8)
                    .inner_margin(8)
                    .show(popup, |popup| {
                        popup.set_width(rect.width() - 18.);
                        if compact {
                            let value = |v: f64, blocked| {
                                if blocked {
                                    "blocked".into()
                                } else {
                                    format!("{v:.1}")
                                }
                            };
                            let text = format!(
                                "Move {} · Drag {}",
                                value(
                                    original.resolved(self.defaults).move_resistance(),
                                    original.move_block()
                                ),
                                value(
                                    original.resolved(self.defaults).drag(),
                                    original.drag_block()
                                )
                            );
                            if popup
                                .add(egui::Button::new(text).frame(false))
                                .on_hover_text("Expand resistance controls (values in mm)")
                                .clicked()
                            {
                                self.applet.collapsed = false;
                            }
                        } else {
                            popup.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · section {}",
                                        selected.side.label(),
                                        selected.index + 1
                                    ))
                                    .color(theme::MUTED),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let response = icons::button(
                                            ui,
                                            icons::Icon::Collapse,
                                            "",
                                            None,
                                            true,
                                        );
                                        response.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::WidgetType::Button,
                                                ui.is_enabled(),
                                                "Collapse resistance controls",
                                            )
                                        });
                                        if response
                                            .on_hover_text("Collapse resistance controls")
                                            .clicked()
                                        {
                                            self.applet.collapsed = true;
                                        }
                                    },
                                );
                            });
                            changed |=
                                channel(popup, "Move", &mut edited, self.defaults, false, false);
                            changed |=
                                channel(popup, "Drag", &mut edited, self.defaults, true, false);
                        }
                    });
            });
        // Draw the tether below the popup, ending at its nearest edge.
        ui.painter().line_segment(
            [anchor, area.response.rect.clamp(anchor)],
            Stroke::new(1., theme::ACCENT),
        );
        ui.painter().circle_filled(anchor, 2., theme::ACCENT);
        (changed && edited != original).then(|| {
            Action::new(
                monitor,
                selected.side,
                Edit::Replace {
                    index: selected.index,
                    section: edited,
                },
            )
        })
    }
}

fn placement(
    bounds: Rect,
    content: Rect,
    section: Rect,
    side: Side,
    requested: egui::Vec2,
) -> Rect {
    let size = requested.min(bounds.size());
    let anchor = section.center();
    let min = match side {
        Side::Left => anchor + vec2(22., -size.y / 2.),
        Side::Right => anchor - vec2(size.x + 22., size.y / 2.),
        Side::Top => anchor + vec2(-size.x / 2., 22.),
        Side::Bottom => anchor - vec2(size.x / 2., size.y + 22.),
    };
    let interior = content.shrink(22.).intersect(bounds);
    if interior.width() >= size.x && interior.height() >= size.y {
        return Rect::from_min_size(min.clamp(interior.min, interior.max - size), size);
    }
    let clamp =
        |min: egui::Pos2| Rect::from_min_size(min.clamp(bounds.min, bounds.max - size), size);
    let preferred = clamp(min);
    if !preferred.intersects(section.expand(8.)) {
        return preferred;
    }
    // On a miniature monitor, move beside the screen before covering its handles.
    [
        pos2(anchor.x - size.x / 2., content.bottom() + 18.),
        pos2(anchor.x - size.x / 2., content.top() - size.y - 18.),
        pos2(content.right() + 18., anchor.y - size.y / 2.),
        pos2(content.left() - size.x - 18., anchor.y - size.y / 2.),
    ]
    .into_iter()
    .map(clamp)
    .find(|r| !r.intersects(section.expand(8.)))
    .unwrap_or(preferred)
}

pub(super) fn channel(
    ui: &mut egui::Ui,
    name: &str,
    section: &mut BorderSection,
    defaults: ResistanceDefaults,
    drag: bool,
    // The properties panel has room for the pictogram; the on-map applet has not.
    pictogram: bool,
) -> bool {
    ui.push_id(name, |ui| {
        let previous = if drag {
            section.drag_mode()
        } else {
            section.move_mode()
        };
        let mut mode = previous;
        let mut value = if drag {
            section.drag()
        } else {
            section.move_resistance()
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.;
            ui.spacing_mut().button_padding = vec2(4., 3.);
            if pictogram {
                icons::glyph(
                    ui,
                    if drag {
                        icons::Icon::Drag
                    } else {
                        icons::Icon::Pointer
                    },
                );
                ui.add_space(4.);
            }
            ui.add_sized(vec2(34., 20.), egui::Label::new(name))
                .on_hover_text(if drag {
                    "Crossing with a mouse button held"
                } else {
                    "Crossing without a mouse button held"
                });
            for choice in [
                ResistanceMode::Block,
                ResistanceMode::Resist,
                ResistanceMode::Free,
                ResistanceMode::Custom,
            ] {
                let response = ui.selectable_value(&mut mode, choice, choice.name());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        ui.is_enabled(),
                        mode == choice,
                        format!("{name}: {}", choice.name()),
                    )
                });
                response.on_hover_text(match choice {
                    ResistanceMode::Block => "Prevent crossing this section",
                    ResistanceMode::Resist => "Use the layout's default resistance",
                    ResistanceMode::Free => "Cross freely, without resistance",
                    ResistanceMode::Custom => "Set a resistance just for this section",
                });
            }
        });
        let mut value_changed = false;
        ui.horizontal(|ui| {
            ui.set_min_height(22.);
            if mode == ResistanceMode::Custom {
                ui.spacing_mut().slider_width = (ui.available_width() - 94.).max(50.);
                let response = ui.add(
                    egui::Slider::new(&mut value, 0.0..=50.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Edits)
                        .step_by(0.1)
                        .trailing_fill(false),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::slider(ui.is_enabled(), value, format!("{name} resistance"))
                });
                value_changed |= response.changed();
                let response = ui.add(
                    egui::DragValue::new(&mut value)
                        .range(0.0..=f64::MAX)
                        .clamp_existing_to_range(false)
                        .speed(if ui.input(|i| i.modifiers.ctrl) {
                            0.01
                        } else {
                            0.1
                        })
                        .max_decimals(2)
                        .suffix(" mm"),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::DragValue,
                        ui.is_enabled(),
                        format!("{name} custom value"),
                    )
                });
                value_changed |= response.changed();
            } else {
                let hint = match mode {
                    ResistanceMode::Resist => format!(
                        "Default · {:.1} mm",
                        if drag {
                            defaults.drag
                        } else {
                            defaults.movement
                        }
                    ),
                    ResistanceMode::Block => "Crossing blocked".into(),
                    ResistanceMode::Free => "No resistance".into(),
                    ResistanceMode::Custom => unreachable!(),
                };
                ui.label(egui::RichText::new(hint).small().color(theme::MUTED));
            }
        });
        if mode != previous {
            if drag {
                section.set_drag_mode(mode);
            } else {
                section.set_move_mode(mode);
            }
        }
        if value_changed {
            if drag {
                section.set_drag(value);
                section.set_drag_mode(ResistanceMode::Custom);
            } else {
                section.set_move_resistance(value);
                section.set_move_mode(ResistanceMode::Custom);
            }
        }
        mode != previous || value_changed
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_stays_in_the_canvas_and_keeps_corner_handles_clear_at_both_scales() {
        let bounds = Rect::from_min_size(pos2(8., 8.), vec2(700., 450.));
        for size in [vec2(600., 300.), vec2(170., 100.)] {
            let content = Rect::from_min_size(pos2(40., 40.), size);
            for side in Side::ALL {
                for span in [Span::new(0., 30.), Span::new(70., 100.)] {
                    let selected = segment(band(content, side), side, span, 100.);
                    let popup = placement(bounds, content, selected, side, vec2(248., 152.));
                    assert!(bounds.contains_rect(popup));
                    assert!(!popup.intersects(selected.expand(8.)), "{side:?}, {size:?}");
                }
            }
        }
    }
}
