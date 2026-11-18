//! C# DefaultMonitorView and MonitorLocationView, adapted to the slate theme.
use crate::theme;
use egui::{Align2, Color32, FontId, Rect, Stroke};

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub number: String,
    pub pnp: String,
    pub name: String,
    pub connector: String,
    pub serial: String,
    pub resolution: String,
    pub density: String,
    pub diagonal: String,
    pub refresh: String,
    pub primary: bool,
    pub excluded: bool,
}

#[derive(Clone, Copy)]
pub enum Presentation {
    Info,
    Location,
}

/// Paint only: monitor selection, dragging and measurement controls retain input.
pub fn draw(ui: &egui::Ui, content: Rect, value: &Info, presentation: Presentation) {
    if content.width() < 60. || content.height() < 40. {
        return;
    }
    let location = matches!(presentation, Presentation::Location);
    let painter = ui
        .painter()
        .with_clip_rect(ui.clip_rect().intersect(content));
    let inset = (content.width().min(content.height()) * 0.06).clamp(6., 18.);
    let rect = content.shrink(inset);
    // A translucent veil preserves the picture and reflection, while text stays legible.
    theme::gradient(
        &painter,
        content,
        Color32::from_rgba_unmultiplied(12, 22, 30, if location { 115 } else { 180 }),
        Color32::from_rgba_unmultiplied(9, 17, 23, if location { 155 } else { 225 }),
    );
    let font = (content.height() * 0.045).clamp(10., 14.);
    let full = rect.width() >= 340. && rect.height() >= 210.;
    let compact = rect.width() >= 180. && rect.height() >= 95.;
    let font = if full { font } else { font.min(11.) };
    let text = |at: egui::Pos2, align: Align2, text: &str, size: f32, color, width: f32| {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text.into(),
            FontId::proportional(size),
            color,
        );
        job.wrap.max_width = width.max(1.);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = painter.layout_job(job);
        let bounds = align.anchor_size(at, galley.size());
        painter.galley(bounds.min, galley, color);
    };
    let accent = if value.primary {
        theme::ACCENT
    } else {
        Color32::from_rgb(141, 169, 229)
    };
    // Location reserves the four edge centres for its existing position/scale editors.
    if !location || (content.width() >= 216. && content.height() >= 120.) {
        let radius = (content.height().min(content.width()) * if location { 0.13 } else { 0.17 })
            .clamp(16., 58.);
        let center = rect.center();
        for spread in (1..=6).rev() {
            painter.circle_stroke(
                center,
                radius + spread as f32 * 2.,
                Stroke::new(2., accent.gamma_multiply(0.012 * (7 - spread) as f32)),
            );
        }
        painter.circle_filled(
            center,
            radius,
            Color32::from_rgba_unmultiplied(19, 31, 41, 225),
        );
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(if location { 2. } else { 1.5 }, accent.gamma_multiply(0.75)),
        );
        painter.circle_stroke(
            center,
            radius - 4.,
            Stroke::new(1., accent.gamma_multiply(0.12)),
        );
        text(
            center,
            Align2::CENTER_CENTER,
            &value.number,
            radius * 0.95,
            theme::TEXT,
            radius * 1.55,
        );
        if full {
            text(
                center + egui::vec2(0., radius + 14.),
                Align2::CENTER_CENTER,
                if value.primary {
                    "PRIMARY DISPLAY"
                } else {
                    "DISPLAY"
                },
                9.,
                accent,
                rect.width() * 0.5,
            );
        }
    }
    if compact {
        let column = rect.width() * if location { 0.27 } else { 0.43 };
        text(
            rect.left_top(),
            Align2::LEFT_TOP,
            &value.pnp,
            font,
            theme::TEXT,
            column,
        );
        if !location {
            text(
                rect.left_top() + egui::vec2(0., font + 7.),
                Align2::LEFT_TOP,
                &value.connector,
                font - 1.,
                theme::MUTED,
                column,
            );
        }
        let resolution = if full {
            value.resolution.clone()
        } else {
            value.resolution.replace(" px", "").replace(" × ", "×")
        };
        let metrics = if rect.height() >= 210. && !location {
            vec![
                resolution.as_str(),
                value.density.as_str(),
                value.diagonal.as_str(),
                value.refresh.as_str(),
            ]
        } else {
            vec![resolution.as_str(), value.density.as_str()]
        };
        for (i, line) in metrics.iter().filter(|s| !s.is_empty()).enumerate() {
            text(
                rect.right_top() + egui::vec2(0., i as f32 * (font + 6.)),
                Align2::RIGHT_TOP,
                line,
                font,
                if i == 0 { theme::TEXT } else { theme::MUTED },
                column,
            );
        }
        if !location {
            text(
                rect.right_bottom(),
                Align2::RIGHT_BOTTOM,
                &value.name,
                font,
                theme::TEXT,
                rect.width() * 0.72,
            );
            if full && !value.serial.is_empty() {
                text(
                    rect.right_bottom() - egui::vec2(0., font + 6.),
                    Align2::RIGHT_BOTTOM,
                    &value.serial,
                    font - 1.,
                    theme::MUTED,
                    rect.width() * 0.65,
                );
            }
            if full && (value.primary || value.excluded) {
                let label = if value.excluded {
                    "Mouse excluded"
                } else {
                    "Primary"
                };
                text(
                    rect.left_bottom(),
                    Align2::LEFT_BOTTOM,
                    label,
                    10.,
                    accent,
                    rect.width() * 0.25,
                );
            }
        }
    } else if !location && rect.height() >= 75. {
        text(
            rect.center_bottom(),
            Align2::CENTER_BOTTOM,
            &value.resolution,
            9.,
            theme::MUTED,
            rect.width(),
        );
    }
}
