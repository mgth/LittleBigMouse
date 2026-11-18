//! Direct measurement editing on the monitor, using the same values as the panel.
use crate::properties::{Edit, Field, Location, Size};
use egui::{Align2, Color32, Rect, Stroke, Ui, UiBuilder, Vec2};

#[derive(Default)]
pub struct Interaction {
    pub edits: Vec<Edit>,
    /// A field owns the gesture or keyboard focus; the frame must not start moving.
    pub engaged: bool,
}

fn aspect_link(
    ui: &mut Ui,
    center: egui::Pos2,
    detail: Detail,
    locked: bool,
    result: &mut Interaction,
) {
    use crate::{icons, theme};
    let radius = match detail {
        Detail::Full => 15.,
        Detail::Compact => 11.,
        Detail::ReadOnly => 4.5,
    };
    let editable = detail != Detail::ReadOnly;
    let response = ui.interact(
        Rect::from_center_size(center, Vec2::splat(radius * 2.)),
        ui.id().with("aspect-link"),
        if editable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled() && editable,
            locked,
            "Keep aspect ratio",
        )
    });
    let keyboard = editable
        && response.has_focus()
        && ui.input_mut(|i| {
            i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
        });
    result.engaged |= ui.is_enabled()
        && editable
        && (response.is_pointer_button_down_on() || response.has_focus() || response.clicked());
    if ui.is_enabled() && editable && (response.clicked() || keyboard) {
        result.edits.push(Edit::Lock(!locked));
    }

    let color = if locked { theme::ACCENT } else { theme::MUTED };
    let p = ui.painter();
    if locked {
        // A common ring joins the horizontal and vertical measures around the lock.
        let ring = radius + if editable { 4. } else { 2. };
        for axis in [Vec2::X, Vec2::Y] {
            for sign in [-1., 1.] {
                p.line_segment(
                    [
                        center + axis * (ring * sign),
                        center + axis * ((ring + 7.) * sign),
                    ],
                    Stroke::new(if editable { 2. } else { 1. }, color),
                );
            }
        }
        p.circle_stroke(center, ring, Stroke::new(1., color.gamma_multiply(0.65)));
    }
    p.circle_filled(
        center,
        radius,
        if locked {
            theme::SELECTED
        } else {
            theme::SURFACE
        },
    );
    p.circle_stroke(
        center,
        radius,
        Stroke::new(
            if response.hovered() || response.has_focus() {
                2.
            } else {
                1.2
            },
            color,
        ),
    );
    if editable {
        let icon_size = if detail == Detail::Full { 16. } else { 13. };
        icons::paint(
            p,
            Rect::from_center_size(center, Vec2::splat(icon_size)),
            if locked {
                icons::Icon::Lock
            } else {
                icons::Icon::Unlock
            },
            color,
        );
    }
    response.on_hover_text(if locked {
        "Width and height are linked. Unlock to adjust them independently."
    } else {
        "Link width and height to keep the aspect ratio."
    });
}

fn arrow(ui: &Ui, a: egui::Pos2, b: egui::Pos2, stroke: Stroke) {
    let length = a.distance(b);
    if length <= f32::EPSILON {
        return;
    }
    let d = (b - a) / length;
    let scale = stroke.width.sqrt();
    let head = (5. * scale).min(length / 3.);
    let n = Vec2::new(-d.y, d.x);
    let p = ui.painter();
    p.line_segment([a, b], stroke);
    // Witness marks identify the measured edges even on very thin bezels.
    for tip in [a, b] {
        p.line_segment([tip - n * (3. * scale), tip + n * (3. * scale)], stroke);
    }
    for (tip, direction) in [(a, d), (b, -d)] {
        p.line_segment([tip, tip + direction * head + n * (head * 0.6)], stroke);
        p.line_segment([tip, tip + direction * head - n * (head * 0.6)], stroke);
    }
}

/// X on the left, Y above, per-axis scales opposite them, at every zoom level.
pub fn location(ui: &mut Ui, id: &str, content: Rect, value: &Location) -> Interaction {
    let mut result = Interaction::default();
    let rect = dimension_rect(content);
    if !rect.is_positive() {
        return result;
    }
    let detail = detail(rect);
    ui.push_id(("inline-location", id), |ui| {
        ui.set_clip_rect(ui.clip_rect().intersect(rect));
        for (index, (field, at)) in value
            .fields()
            .into_iter()
            .zip([(0., 0.5), (0.5, 0.), (1., 0.5), (0.5, 1.)])
            .enumerate()
        {
            dimension(ui, id, rect, at, (field, index >= 2), detail, &mut result);
        }
        if detail == Detail::ReadOnly {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "mm",
                egui::FontId::proportional(9.),
                crate::theme::MUTED,
            );
        }
    });
    result
}

struct Measurement {
    from: egui::Pos2,
    to: egui::Pos2,
    label: (f32, f32),
    color: Color32,
}

fn measurements(drawn: &crate::frame::Drawn) -> [Measurement; 8] {
    let panel = drawn.content;
    let outside = drawn.outside;
    let rect = dimension_rect(panel);
    let x = |t| rect.left() + t * rect.width();
    let y = |t| rect.top() + t * rect.height();
    let inside_color = Color32::from_rgb(215, 211, 181);
    let outside_color = Color32::from_rgb(108, 174, 188);
    let bezel_color = Color32::from_gray(210);
    let horizontal = |left, right, at, label, color| Measurement {
        from: egui::pos2(left, at),
        to: egui::pos2(right, at),
        label,
        color,
    };
    let vertical = |top, bottom, at, label, color| Measurement {
        from: egui::pos2(at, top),
        to: egui::pos2(at, bottom),
        label,
        color,
    };
    [
        horizontal(
            panel.left(),
            panel.right(),
            y(0.62),
            (0.36, 0.62),
            inside_color,
        ),
        vertical(
            panel.top(),
            panel.bottom(),
            x(0.60),
            (0.60, 0.34),
            inside_color,
        ),
        horizontal(
            outside.left(),
            panel.left(),
            y(0.40),
            (0., 0.40),
            bezel_color,
        ),
        vertical(outside.top(), panel.top(), x(0.5), (0.5, 0.08), bezel_color),
        horizontal(
            panel.right(),
            outside.right(),
            y(0.40),
            (1., 0.40),
            bezel_color,
        ),
        vertical(
            panel.bottom(),
            outside.bottom(),
            x(0.5),
            (0.5, 0.92),
            bezel_color,
        ),
        horizontal(
            outside.left(),
            outside.right(),
            y(0.8),
            (0.30, 0.8),
            outside_color,
        ),
        vertical(
            outside.top(),
            outside.bottom(),
            x(0.83),
            (0.83, 0.7),
            outside_color,
        ),
    ]
}

fn dimension_rect(content: Rect) -> Rect {
    content.shrink((content.width().min(content.height()) * 0.08).min(8.))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Detail {
    Full,
    Compact,
    ReadOnly,
}

fn detail(rect: Rect) -> Detail {
    if rect.width() >= 440. && rect.height() >= 300. {
        Detail::Full
    } else if rect.width() >= 200. && rect.height() >= 105. {
        Detail::Compact
    } else {
        Detail::ReadOnly
    }
}

/// Every scale keeps the same dimension geometry. Only the value widgets change.
pub fn size(ui: &mut Ui, id: &str, drawn: &crate::frame::Drawn, value: &Size) -> Interaction {
    let mut result = Interaction::default();
    let rect = dimension_rect(drawn.content);
    if !rect.is_positive() {
        return result;
    }
    let detail = detail(rect);
    ui.push_id(("inline-size", id), |ui| {
        // Scale in logical points with the displayed panel, including portrait screens.
        // Miniatures keep their fine lines; large monitors gain weight gradually.
        let stroke_width = (1. + (drawn.content.size().min_elem() - 200.) / 100.).clamp(1., 4.);
        let canvas_clip = ui.clip_rect();
        let at = |x, y| {
            egui::pos2(
                rect.left() + x * rect.width(),
                rect.top() + y * rect.height(),
            )
        };
        // Tips and witness marks always reach the actual panel and bezel edges.
        let marks_margin = 3. * stroke_width.sqrt() + stroke_width * 0.5;
        ui.set_clip_rect(canvas_clip.intersect(drawn.outside.expand(marks_margin)));
        for (index, measure) in measurements(drawn).into_iter().enumerate() {
            let stroke = Stroke::new(stroke_width, measure.color);
            if (2..6).contains(&index) && measure.from != measure.to {
                ui.painter().line_segment(
                    [
                        measure.from.lerp(measure.to, 0.5),
                        at(measure.label.0, measure.label.1),
                    ],
                    stroke,
                );
            }
            arrow(ui, measure.from, measure.to, stroke);
        }
        ui.set_clip_rect(canvas_clip.intersect(rect));
        // Use the actual measured axes, so the link stays on their intersection.
        let axes = measurements(drawn);
        aspect_link(
            ui,
            egui::pos2(axes[1].from.x, axes[0].from.y),
            detail,
            value.locked,
            &mut result,
        );
        for (field, measure) in value.fields().into_iter().zip(measurements(drawn)) {
            dimension(
                ui,
                id,
                rect,
                measure.label,
                (field, false),
                detail,
                &mut result,
            );
        }
        if detail == Detail::ReadOnly {
            ui.painter().text(
                at(0.25, 0.17),
                Align2::CENTER_CENTER,
                "mm",
                egui::FontId::proportional(9.),
                crate::theme::MUTED,
            );
        }
    });
    result
}

fn dimension(
    ui: &mut Ui,
    monitor: &str,
    rect: Rect,
    at: (f32, f32),
    (field, percent): (Field, bool),
    detail: Detail,
    result: &mut Interaction,
) {
    let (name, current, min, edit) = field;
    let font_size = match detail {
        Detail::Full => 13.,
        Detail::Compact => 10.,
        Detail::ReadOnly => (rect.width() / 16.).clamp(8., 10.),
    };
    let unit = if percent { "%" } else { "mm" };
    let text = if detail == Detail::ReadOnly {
        if percent {
            format!("{current:.1} %")
        } else {
            format!("{current:.1}")
        }
    } else {
        format!("{current:.2} {unit}")
    };
    let font = egui::FontId::proportional(font_size);
    let text_size = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), crate::theme::TEXT)
        .size();
    let padding = if detail == Detail::Full {
        egui::vec2(10., 6.)
    } else {
        egui::vec2(3., 2.)
    };
    let height = (text_size.y + padding.y * 2.).max(if detail == Detail::Full { 28. } else { 16. });
    let width = (text_size.x + padding.x * 2.).min(rect.width());
    let center = egui::pos2(
        (rect.left() + at.0 * rect.width())
            .clamp(rect.left() + width / 2., rect.right() - width / 2.),
        (rect.top() + at.1 * rect.height()).clamp(
            rect.top() + height.min(rect.height()) / 2.,
            rect.bottom() - height.min(rect.height()) / 2.,
        ),
    );
    let target = Rect::from_center_size(center, egui::vec2(width, height));
    ui.push_id(name, |ui| {
        ui.scope_builder(UiBuilder::new().max_rect(target).layout(egui::Layout::top_down(egui::Align::Center)), |ui| {
            ui.spacing_mut().button_padding = padding;
            ui.spacing_mut().interact_size = egui::vec2(0., height);
            ui.spacing_mut().text_edit_width = width;
            ui.style_mut().override_font_id = Some(font.clone());
            let mut number = current;
            let response = if detail == Detail::ReadOnly {
                let (bounds, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
                ui.painter().rect_filled(bounds, 3., crate::theme::SURFACE);
                ui.painter().text(bounds.center(), Align2::CENTER_CENTER, &text, font, crate::theme::TEXT);
                response.widget_info(|| {
                    let mut info = egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), name);
                    info.value = Some(current);
                    info
                });
                response
            } else {
                let response = crate::properties::measurement(ui, &mut number, min, percent);
                response.widget_info(|| {
                    let mut info = egui::WidgetInfo::drag_value(ui.is_enabled(), number);
                    info.label = Some(name.to_owned()); info
                });
                response
            };
            crate::properties::measurement_highlight(ui, monitor, name, &response);
            let response = response.on_hover_text(if detail == Detail::ReadOnly {
                format!("{name}: {current:.2} {unit}\nEdit in the properties panel.")
            } else {
                format!("{name}\nClick to type; Enter to confirm, Escape to cancel. Drag to adjust. Wheel: Ctrl for precision, Shift for larger steps.")
            });
            result.engaged |= ui.is_enabled() && (response.has_focus() || response.clicked() || response.dragged() || response.is_pointer_button_down_on());
            if response.changed() && number != current { result.edits.push(edit(number)); }
        });
    });
}
