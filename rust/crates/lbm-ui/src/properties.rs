//! Selected-monitor properties use window-sized text, independently of the map zoom.
use crate::icons::{self, Icon};
use lbm_layout::model::DisplaySize;

pub fn panel(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    let beside = ui.available_width() >= 760.0;
    let max = if beside {
        430.0
    } else {
        (ui.available_height() * 0.6).max(100.0)
    };
    let panel = if beside {
        egui::Panel::right("monitor-properties-side")
            .default_size(320.0)
            .min_size(280.0)
            .max_size(max)
    } else {
        egui::Panel::bottom("monitor-properties-bottom")
            .default_size(260.0)
            .min_size(100.0)
            .max_size(max)
    };
    let frame = crate::theme::toolbar();
    let margins = frame.total_margin().sum();
    panel.resizable(true).frame(frame).show(ui, |ui| {
        // **Overflow past `max_size` is clipped here, never reported to the panel.** When
        // its content outgrows `max_size`, egui 0.36 trims the panel's rect back to
        // `max_size` from the *far* edge of the overflow (`panel.rs`, after `frame.show`),
        // so the space it reserves shrinks while its content stays where it started: the
        // central map then paints over the start of every line. A Windows interface path
        // (`\\?\DISPLAY#…#{GUID}`) as an output's connector was enough. Up to `max_size`
        // the content is still reported, so the panel keeps growing to fit it.
        let room = ui.available_rect_before_wrap();
        let mut reach = room;
        if beside {
            reach.max.x = room.min.x + max - margins.x;
        } else {
            reach.max.y = room.min.y + max - margins.y;
        }
        let mut inner = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("monitor-properties-content")
                .max_rect(room)
                .layout(*ui.layout()),
        );
        content(&mut inner);
        ui.advance_cursor_after_rect(inner.min_rect().intersect(reach));
    });
}

/// Screen selection and its adjacent map/single-screen toggle stay reachable in every mode.
pub fn monitor_selector(
    ui: &mut egui::Ui,
    selected: &mut Option<String>,
    monitors: &[(&str, &str)],
    single: &mut bool,
) {
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 40.0 - ui.spacing().item_spacing.x).max(40.0);
        let name = monitors
            .iter()
            .find(|(id, _)| Some(*id) == selected.as_deref())
            .map(|(_, name)| *name)
            .unwrap_or("Select a screen");
        let combo = egui::ComboBox::from_id_salt("properties-monitor")
            .width(width)
            .truncate()
            .selected_text(name)
            .show_ui(ui, |ui| {
                for (id, name) in monitors {
                    ui.selectable_value(selected, Some((*id).to_owned()), format!("{name} · {id}"));
                }
            });
        combo.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, ui.is_enabled(), "Screen")
        });
        if crate::toolbar::button(
            ui,
            crate::toolbar::Icon::List,
            "Single screen view",
            *single,
            !monitors.is_empty(),
        )
        .on_hover_text(if *single {
            "Show all screens"
        } else {
            "Focus on the selected screen"
        })
        .clicked()
        {
            *single = !*single;
            if *single
                && !monitors
                    .iter()
                    .any(|(id, _)| Some(*id) == selected.as_deref())
            {
                *selected = monitors.first().map(|(id, _)| (*id).to_owned());
            }
        }
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
    X(f64),
    Y(f64),
    ScaleX(f64),
    ScaleY(f64),
    Width(f64),
    Height(f64),
    Left(f64),
    Top(f64),
    Right(f64),
    Bottom(f64),
    OutsideWidth(f64),
    OutsideHeight(f64),
    Lock(bool),
}

pub struct Size {
    pub physical: DisplaySize,
    pub locked: bool,
    pub per_monitor_borders: bool,
    pub model: String,
}

pub fn size(ui: &mut egui::Ui, monitor: &str, value: &Size) -> Vec<Edit> {
    let mut edits = Vec::new();
    ui.label("Dimensions in mm, in the screen's current orientation.");
    ui.label(format!("Panel size is shared by model {}.", value.model));
    let mut locked = value.locked;
    let lock = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            icons::glyph(ui, if locked { Icon::Lock } else { Icon::Unlock });
            ui.checkbox(&mut locked, "Keep aspect ratio")
        })
        .inner;
    if lock
        .on_hover_text("Editing aid for this session; the dimensions are saved.")
        .changed()
    {
        edits.push(Edit::Lock(locked));
    }
    let p = value.physical;
    egui::Grid::new("size-fields")
        .num_columns(2)
        .show(ui, |ui| {
            for ((name, current, min, edit), icon) in value.fields().into_iter().zip(SIZE_ICONS) {
                let label = icons::label(ui, icon, name);
                let mut number = current;
                let response = measurement(ui, &mut number, min, false).labelled_by(label.id);
                measurement_highlight(ui, monitor, name, &response);
                if response.changed() && number != current {
                    edits.push(edit(number));
                }
                ui.end_row();
            }
        });
    ui.label(if value.per_monitor_borders {
        "Bezels belong to this monitor."
    } else {
        "Bezels are shared by monitors of this model."
    });
    ui.small(
        "Outside width adjusts both side bezels equally; outside height adjusts the bottom bezel.",
    );
    icons::label(
        ui,
        Icon::Diagonal,
        format!("Diagonal: {:.2} in", p.width.hypot(p.height) / 25.4),
    );
    edits
}

/// All measurement controls register during one egui pass; paint after the map and panel
/// so hover works in both directions without a frame of lag or cross-monitor leaks.
#[derive(Clone, Default)]
struct MeasurementHighlights {
    pass: u64,
    hovered: Option<egui::Id>,
    focused: Option<egui::Id>,
    boxes: Vec<(egui::Id, egui::Rect, egui::Rect, egui::LayerId)>,
}

pub(crate) fn measurement_highlight(
    ui: &egui::Ui,
    monitor: &str,
    name: &str,
    response: &egui::Response,
) {
    let pass = ui.ctx().cumulative_pass_nr();
    let key = egui::Id::new(("measurement", monitor, name));
    let hovered = ui.is_enabled() && response.hovered();
    let focused = ui.is_enabled() && (response.has_focus() || response.dragged());
    ui.ctx().data_mut(|data| {
        let state = data.get_temp_mut_or_default::<MeasurementHighlights>(egui::Id::new(
            "measurement-highlights",
        ));
        if state.pass != pass {
            *state = MeasurementHighlights {
                pass,
                ..Default::default()
            };
        }
        if hovered {
            state.hovered = Some(key);
        }
        if focused {
            state.focused = Some(key);
        }
        state
            .boxes
            .push((key, response.rect, ui.clip_rect(), ui.layer_id()));
    });
}

/// Call once after drawing both the properties panel and the monitor measurements.
pub fn paint_measurement_highlights(ctx: &egui::Context) {
    let Some(state) = ctx.data(|data| {
        data.get_temp::<MeasurementHighlights>(egui::Id::new("measurement-highlights"))
    }) else {
        return;
    };
    if state.pass != ctx.cumulative_pass_nr() {
        return;
    }
    let active = state.hovered.or(state.focused);
    for (key, rect, clip, layer) in state.boxes {
        if Some(key) == active {
            ctx.layer_painter(layer).with_clip_rect(clip).rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, crate::theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// Both the report and the layout are read-only here; long device paths wrap.
pub fn rows(ui: &mut egui::Ui, rows: &[(String, String)]) {
    for (name, value) in rows {
        ui.label(egui::RichText::new(name).weak());
        ui.add(egui::Label::new(value).wrap().selectable(true));
    }
}

/// Shared field definitions keep the panel and the on-screen controls in agreement.
pub type Field = (&'static str, f64, f64, fn(f64) -> Edit);

/// Pictograms of [`Size::fields`], in the same order.
const SIZE_ICONS: [Icon; 8] = [
    Icon::Width,
    Icon::Height,
    Icon::EdgeLeft,
    Icon::EdgeTop,
    Icon::EdgeRight,
    Icon::EdgeBottom,
    Icon::OuterWidth,
    Icon::OuterHeight,
];

/// Pictograms of [`Location::fields`], in the same order.
const LOCATION_ICONS: [Icon; 4] = [Icon::PositionX, Icon::PositionY, Icon::ScaleX, Icon::ScaleY];
impl Size {
    pub fn fields(&self) -> [Field; 8] {
        let p = self.physical;
        [
            ("Panel width", p.width, 0.1, Edit::Width as fn(f64) -> Edit),
            ("Panel height", p.height, 0.1, Edit::Height),
            ("Left bezel", p.left_border, 0.0, Edit::Left),
            ("Top bezel", p.top_border, 0.0, Edit::Top),
            ("Right bezel", p.right_border, 0.0, Edit::Right),
            ("Bottom bezel", p.bottom_border, 0.0, Edit::Bottom),
            (
                "Outside width",
                p.outside_width(),
                p.width + (p.left_border - p.right_border).abs(),
                Edit::OutsideWidth,
            ),
            (
                "Outside height",
                p.outside_height(),
                p.height + p.top_border,
                Edit::OutsideHeight,
            ),
        ]
    }
}

pub struct Location {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}
impl Location {
    pub fn fields(&self) -> [Field; 4] {
        [
            ("X position", self.x, -f64::MAX, Edit::X),
            ("Y position", self.y, -f64::MAX, Edit::Y),
            ("Horizontal scale", self.scale_x, 5., Edit::ScaleX),
            ("Vertical scale", self.scale_y, 5., Edit::ScaleY),
        ]
    }
}

pub fn location(ui: &mut egui::Ui, monitor: &str, value: &Location) -> Vec<Edit> {
    let mut edits = Vec::new();
    egui::Grid::new("location-fields")
        .num_columns(2)
        .show(ui, |ui| {
            for (index, ((name, current, min, edit), icon)) in
                value.fields().into_iter().zip(LOCATION_ICONS).enumerate()
            {
                let label = icons::label(ui, icon, name);
                let mut number = current;
                let scale = index >= 2;
                let response = measurement(ui, &mut number, min, scale).labelled_by(label.id);
                measurement_highlight(ui, monitor, name, &response);
                if response.changed() && number != current {
                    edits.push(edit(number));
                }
                ui.end_row();
            }
        });
    edits
}

pub(crate) fn parse_number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Defer typed edits until confirmation, including across egui's focus-loss frame.
pub(crate) fn measurement(
    ui: &mut egui::Ui,
    value: &mut f64,
    min: f64,
    percent: bool,
) -> egui::Response {
    let original = *value;
    let guard = ui.next_auto_id().with("cancelled-measurement");
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let mut cancelled = escape || ui.data(|d| d.get_temp::<bool>(guard).unwrap_or(false));
    let mut response = ui.add(
        egui::DragValue::new(value)
            .speed(0.1)
            .range(min..=if percent { 2000. } else { f64::MAX })
            .clamp_existing_to_range(false)
            .update_while_editing(false)
            .max_decimals(2)
            .custom_parser(parse_number)
            .suffix(if percent { " %" } else { " mm" }),
    );
    if !escape && (response.gained_focus() || response.clicked() || response.drag_started()) {
        cancelled = false;
    }
    if response.hovered() && ui.is_enabled() && !response.has_focus() && !escape {
        let wheel = ui.input_mut(|i| {
            let mut change = 0.;
            i.events.retain(|event| {
                if let egui::Event::MouseWheel {
                    delta, modifiers, ..
                } = event
                {
                    if delta.y != 0. {
                        let mut step = f64::from(delta.y.signum());
                        if modifiers.ctrl {
                            step /= 10.;
                        }
                        if modifiers.shift {
                            step *= 10.;
                        }
                        change += step;
                        return false;
                    }
                }
                true
            });
            i.smooth_scroll_delta = egui::Vec2::ZERO;
            change
        });
        if wheel != 0. {
            cancelled = false;
            *value = (*value + wheel).clamp(min, if percent { 2000. } else { f64::MAX });
            response.mark_changed();
        }
    }
    cancelled |= !ui.is_enabled();
    if cancelled {
        *value = original;
    }
    ui.data_mut(|d| d.insert_temp(guard, cancelled));
    response
}
