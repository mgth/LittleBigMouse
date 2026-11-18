//! Direct framing in physical space. Pointer capture freezes the view transform.
use crate::{frame, map, theme};
use lbm_layout::geo::{Rect, Size};
use lbm_store::wallpaper_settings::SpanPlacement;

#[derive(Default)]
pub struct Editor {
    drag: Option<Drag>,
}
struct Drag {
    start: SpanPlacement,
    original: Option<SpanPlacement>,
    image: Rect,
    fit: map::Fit,
    handle: usize,
}

fn projected(rect: Rect, fit: &map::Fit) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(fit.x(rect.left()), fit.y(rect.top())),
        egui::pos2(fit.x(rect.right()), fit.y(rect.bottom())),
    )
}

const HANDLES: [(f64, f64); 8] = [
    (0., 0.),
    (0.5, 0.),
    (1., 0.),
    (1., 0.5),
    (1., 1.),
    (0.5, 1.),
    (0., 1.),
    (0., 0.5),
];
const LABELS: [&str; 8] = [
    "Resize panorama top left",
    "Resize panorama top",
    "Resize panorama top right",
    "Resize panorama right",
    "Resize panorama bottom right",
    "Resize panorama bottom",
    "Resize panorama bottom left",
    "Resize panorama left",
];

fn moved(
    start: SpanPlacement,
    image: Rect,
    handle: usize,
    delta: egui::Vec2,
    ratio: f64,
) -> SpanPlacement {
    let (dx, dy) = (f64::from(delta.x) / ratio, f64::from(delta.y) / ratio);
    if handle == 8 {
        return SpanPlacement {
            offset_x_mm: start.offset_x_mm + dx,
            offset_y_mm: start.offset_y_mm + dy,
            ..start
        };
    }
    let (hx, hy) = HANDLES[handle];
    let (vx, vy) = (
        (hx * 2. - 1.) * image.width(),
        (hy * 2. - 1.) * image.height(),
    );
    // Project onto the diagonal, keeping the opposite corner (or edge centre) fixed.
    let factor = (1. + (dx * vx + dy * vy) / (vx * vx + vy * vy))
        .clamp(0.01 / start.scale, 100. / start.scale);
    SpanPlacement {
        scale: start.scale * factor,
        offset_x_mm: start.offset_x_mm + (hx - 0.5) * image.width() * (factor - 1.),
        offset_y_mm: start.offset_y_mm + (hy - 0.5) * image.height() * (factor - 1.),
    }
}

impl Editor {
    /// Returns true only when the draft changes. The caller owns Save/Undo and IPC.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        monitors: &[map::MapMonitor<'_>],
        texture: &egui::TextureHandle,
        size: Size,
        placement: &mut Option<SpanPlacement>,
        selected: Option<&str>,
    ) -> bool {
        let before = *placement;
        let bounds = map::extent(monitors);
        let Ok(mut image) = lbm_wallpaper::render::panorama_rect(size, bounds, *placement) else {
            ui.label("The panorama needs a measured screen layout.");
            return false;
        };
        let available = ui.max_rect().shrink2(egui::vec2(12., 26.));
        // The whole source, including the portions outside the screens, remains visible.
        let fit = self
            .drag
            .as_ref()
            .map(|d| map::Fit {
                ratio: d.fit.ratio,
                origin: d.fit.origin,
                corner: d.fit.corner,
            })
            .unwrap_or_else(|| map::fit(bounds.union(&image), available));
        if fit.ratio <= 0. {
            return false;
        }
        let mut rect = projected(image, &fit);
        let mut responses = Vec::new();
        // Body first, handles last: the small controls win where they overlap.
        responses.push((
            8,
            ui.interact(rect, ui.id().with("panorama-move"), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::Grab)
                .on_hover_text("Drag to position the panorama"),
        ));
        for (i, (hx, hy)) in HANDLES.iter().enumerate() {
            let pos = rect.min + rect.size() * egui::vec2(*hx as f32, *hy as f32);
            let response = ui
                .interact(
                    egui::Rect::from_center_size(pos, egui::vec2(18., 18.)),
                    ui.id().with(("panorama-handle", i)),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(match i {
                    0 | 4 => egui::CursorIcon::ResizeNwSe,
                    2 | 6 => egui::CursorIcon::ResizeNeSw,
                    1 | 5 => egui::CursorIcon::ResizeVertical,
                    _ => egui::CursorIcon::ResizeHorizontal,
                });
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::DragValue, ui.is_enabled(), LABELS[i])
            });
            responses.push((i, response));
        }
        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape)) || !ui.is_enabled();
        if cancel {
            if let Some(drag) = self.drag.take() {
                *placement = drag.original;
            }
        } else {
            for (handle, response) in &responses {
                if response.drag_started() {
                    self.drag = Some(Drag {
                        start: placement.unwrap_or_default(),
                        original: *placement,
                        image,
                        fit: map::Fit {
                            ratio: fit.ratio,
                            origin: fit.origin,
                            corner: fit.corner,
                        },
                        handle: *handle,
                    });
                }
                if let Some(drag) = &self.drag {
                    if drag.handle == *handle {
                        if let Some(delta) = response.total_drag_delta() {
                            *placement = Some(moved(
                                drag.start,
                                drag.image,
                                *handle,
                                delta,
                                drag.fit.ratio,
                            ));
                        }
                        if response.drag_stopped() {
                            self.drag = None;
                        }
                    }
                }
            }
        }
        if *placement != before {
            image = lbm_wallpaper::render::panorama_rect(size, bounds, *placement).unwrap();
            rect = projected(image, &fit);
        }
        let painter = ui.painter();
        painter.rect_filled(rect, 0., egui::Color32::BLACK);
        painter.image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::from_gray(110),
        );
        for m in monitors {
            frame::monitor_with_panorama(
                ui,
                &fit.place(m),
                &frame::Look {
                    name: m.name,
                    selected: selected == Some(m.id),
                    logo: m.logo,
                    wallpaper: None,
                    details: m.details,
                    probe: None,
                },
                Some((texture, rect)),
            );
        }
        let painter = ui.painter();
        painter.rect_stroke(
            rect,
            0.,
            egui::Stroke::new(1., theme::ACCENT),
            egui::StrokeKind::Outside,
        );
        for (hx, hy) in HANDLES {
            let pos = rect.min + rect.size() * egui::vec2(hx as f32, hy as f32);
            let handle = egui::Rect::from_center_size(pos, egui::vec2(8., 8.));
            painter.rect_filled(handle.expand(2.), 3., theme::BACKGROUND);
            painter.rect_filled(handle, 2., theme::ACCENT);
        }
        let hint = if ui.max_rect().width() < 260. {
            "Drag · Resize · Esc"
        } else if ui.max_rect().width() < 500. {
            "Drag image · Resize with handles"
        } else {
            "Drag image to move · Handles keep proportions · Esc to cancel"
        };
        painter.text(
            egui::pos2(ui.max_rect().center().x, ui.max_rect().bottom() - 8.),
            egui::Align2::CENTER_BOTTOM,
            hint,
            egui::FontId::proportional(12.),
            theme::MUTED,
        );
        *placement != before
    }
}
