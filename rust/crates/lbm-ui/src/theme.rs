//! LBM's green selection and monitor hardware, with Studio's quiet dark surfaces.
use egui::{Color32, FontId, Margin, Stroke, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(14, 18, 23);
pub const SURFACE: Color32 = Color32::from_rgb(22, 28, 34);
pub const RAISED: Color32 = Color32::from_rgb(30, 38, 45);
pub const BORDER: Color32 = Color32::from_rgb(47, 58, 66);
pub const TEXT: Color32 = Color32::from_rgb(226, 235, 240);
pub const MUTED: Color32 = Color32::from_rgb(145, 164, 175);
pub const ACCENT: Color32 = Color32::from_rgb(113, 218, 169);
pub const SELECTED: Color32 = Color32::from_rgb(37, 70, 59);
pub const AMBER: Color32 = Color32::from_rgb(237, 186, 107);

pub fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = SURFACE;
    v.window_fill = SURFACE;
    v.extreme_bg_color = BACKGROUND;
    v.faint_bg_color = RAISED;
    v.override_text_color = Some(TEXT);
    v.weak_text_color = Some(MUTED);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = SELECTED;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = 10.into();
    for (widget, fill, edge) in [
        (&mut v.widgets.noninteractive, SURFACE, BORDER),
        (&mut v.widgets.inactive, RAISED, BORDER),
        (&mut v.widgets.hovered, Color32::from_rgb(43, 56, 62), MUTED),
        (&mut v.widgets.active, SELECTED, ACCENT),
        (&mut v.widgets.open, SELECTED, ACCENT),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, edge);
        widget.fg_stroke = Stroke::new(1.0, TEXT);
        widget.corner_radius = 6.into();
    }
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Heading, FontId::proportional(18.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.window_margin = Margin::same(16);
    style.spacing.indent = 18.0;
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    ctx.set_theme(egui::Theme::Dark);
}

/// Make an unframed button keep its size when the pointer reaches it.
///
/// egui lays a button out with the frame's stroke width in its margin only while the
/// frame is drawn, so a button without a frame at rest grows by two strokes on hover
/// and pushes its neighbours along. Here the frame is always there: at rest its fill
/// and its stroke are transparent but keep their width, and the padding gives that
/// width back so the button is the size it always was. Call it on the `Ui` the button
/// is added to, which must be a child (`add_enabled` closure) so the style stays local.
pub fn steady_at_rest<'a>(ui: &mut egui::Ui, button: egui::Button<'a>) -> egui::Button<'a> {
    // A disabled button cannot be hovered and is laid out as it always was.
    if !ui.is_enabled() {
        return button.frame_when_inactive(false);
    }
    let padding = &mut ui.spacing_mut().button_padding;
    *padding = (*padding - egui::vec2(1.0, 1.0)).max(egui::Vec2::ZERO);
    let idle = &mut ui.visuals_mut().widgets.inactive;
    idle.weak_bg_fill = Color32::TRANSPARENT;
    idle.bg_fill = Color32::TRANSPARENT;
    idle.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
    button.frame_when_inactive(true)
}

pub fn toolbar() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .inner_margin(Margin::symmetric(14, 9))
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(8)
        .inner_margin(12)
}

pub fn status(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(12)
        .inner_margin(Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.0, color);
                ui.label(egui::RichText::new(text).size(12.0).color(color));
            });
        })
        .response
}

/// Subtle lighting on an opaque surface; no textures or render backend required.
pub fn gradient(painter: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}
