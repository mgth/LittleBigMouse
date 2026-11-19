//! Original Avalonia toolbar assets and geometry (PluginsControlsView / LbmStyles).
use egui::{vec2, Color32, Rect, Response, Stroke, Ui};

#[derive(Clone, Copy, Debug, Hash)]
pub(crate) enum Icon {
    About,
    Info,
    Options,
    List,
    Location,
    Resistance,
    Size,
    Vcp,
    Wallpaper,
}

impl Icon {
    fn svg(self) -> &'static [u8] {
        match self {
            Self::About => include_bytes!("../assets/about.svg"),
            Self::Info => include_bytes!("../../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon/MonitorInfo.svg"),
            Self::Options => include_bytes!("../../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon/Options.svg"),
            Self::List => include_bytes!("../../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon/MonitorList.svg"),
            Self::Location => include_bytes!("../../../../LittleBigMouse.Plugins/LittleBigMouse.Plugin.Layout.Avalonia/Assets/Icon/MonitorLocation.svg"),
            Self::Resistance => include_bytes!("../../../../LittleBigMouse.Plugins/LittleBigMouse.Plugin.Layout.Avalonia/Assets/Icon/MonitorBorderResistance.svg"),
            Self::Size => include_bytes!("../../../../LittleBigMouse.Plugins/LittleBigMouse.Plugin.Layout.Avalonia/Assets/Icon/MonitorSize.svg"),
            Self::Vcp => include_bytes!("../../../../LittleBigMouse.Plugins/LittleBigMouse.Plugin.Vcp.Avalonia/Assets/Icon/MonitorVCP.svg"),
            Self::Wallpaper => include_bytes!("../../../../LittleBigMouse.Plugins/LittleBigMouse.Plugin.Wallpaper.Avalonia/Assets/Icon/Wallpaper.svg"),
        }
    }
}

pub(crate) fn button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                label,
            )
        });
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact_selectable(&response, selected);
            let fill = if selected {
                ui.visuals().selection.bg_fill
            } else if response.is_pointer_button_down_on()
                || response.hovered()
                || response.has_focus()
            {
                visuals.weak_bg_fill
            } else {
                Color32::TRANSPARENT
            };
            let stroke = if selected || response.has_focus() {
                ui.visuals().selection.stroke
            } else {
                Stroke::NONE
            };
            ui.painter()
                .rect(rect, 8.0, fill, stroke, egui::StrokeKind::Inside);
            let foreground = if selected {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().text_color()
            };
            // The SVG's blue details stay coloured, just as in the C# IconView.
            let tint = if ui.is_enabled() {
                Color32::WHITE
            } else {
                Color32::from_white_alpha(64)
            };
            paint(
                ui,
                icon,
                Rect::from_center_size(rect.center(), vec2(22.0, 22.0)),
                foreground,
                tint,
            );
        }
        response
    })
    .inner
}

/// Rasterise at the native display density; retain only the current density
/// and theme per icon so moving between monitors cannot grow the cache.
pub(crate) fn paint(ui: &Ui, icon: Icon, rect: Rect, foreground: Color32, tint: Color32) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let size = (rect.width() * ui.ctx().pixels_per_point()).ceil().max(1.0) as u32;
    let key = egui::Id::new(("original-toolbar", icon, rect.width().to_bits()));
    let cached = ui
        .ctx()
        .data_mut(|d| d.get_temp::<(u32, Color32, egui::TextureHandle)>(key));
    let texture = match cached {
        Some((old_size, old_color, texture)) if old_size == size && old_color == foreground => {
            texture
        }
        _ => {
            let tree = lbm_icons::load(
                icon.svg(),
                lbm_icons::Rgba(foreground.r(), foreground.g(), foreground.b(), 255),
            )
            .expect("bundled toolbar SVG");
            let pixels = lbm_icons::render(&tree, size).expect("toolbar icon dimensions");
            let texture = ui.ctx().load_texture(
                format!("toolbar-{icon:?}-{size}"),
                egui::ColorImage::from_rgba_premultiplied([size as usize; 2], &pixels),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx()
                .data_mut(|d| d.insert_temp(key, (size, foreground, texture.clone())));
            texture
        }
    };
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        tint,
    );
}

pub(crate) fn separator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 40.0), egui::Sense::hover());
    let center = rect.center();
    ui.painter().line_segment(
        [center - vec2(0.0, 11.0), center + vec2(0.0, 11.0)],
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
}
