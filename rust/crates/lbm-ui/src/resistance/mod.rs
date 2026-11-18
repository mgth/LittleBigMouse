//! The resistance editor: values in the properties panel, intervals on the map.
mod applet;
mod bands;
pub mod geometry;
mod shape;
pub use bands::{band, owner, segment};
pub use geometry::SnapKind;
use geometry::{Span, MAX_SECTIONS};
use lbm_layout::{
    geo::Rect,
    model::{BorderResistance, BorderSection, ResistanceDefaults},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Side {
    #[default]
    Left,
    Top,
    Right,
    Bottom,
}
impl Side {
    pub const ALL: [Self; 4] = [Self::Left, Self::Top, Self::Right, Self::Bottom];
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Top => "Top",
            Self::Right => "Right",
            Self::Bottom => "Bottom",
        }
    }
    pub fn vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
    pub fn length(self, rect: Rect) -> f64 {
        if self.vertical() {
            rect.height()
        } else {
            rect.width()
        }
    }
    pub fn origin(self, rect: Rect) -> f64 {
        if self.vertical() {
            rect.top()
        } else {
            rect.left()
        }
    }
    pub fn sections(self, resistance: &BorderResistance) -> &[BorderSection] {
        match self {
            Self::Left => &resistance.left,
            Self::Top => &resistance.top,
            Self::Right => &resistance.right,
            Self::Bottom => &resistance.bottom,
        }
        .sections
        .as_slice()
    }
    pub fn sections_mut(self, resistance: &mut BorderResistance) -> &mut Vec<BorderSection> {
        &mut match self {
            Self::Left => &mut resistance.left,
            Self::Top => &mut resistance.top,
            Self::Right => &mut resistance.right,
            Self::Bottom => &mut resistance.bottom,
        }
        .sections
    }
}
#[derive(Clone, Debug)]
pub struct Monitor {
    pub id: String,
    pub bounds: Rect,
    pub excluded: bool,
    pub resistance: BorderResistance,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub monitor: String,
    pub side: Side,
    pub index: usize,
}
#[derive(Clone, Debug)]
pub enum Edit {
    Insert(BorderSection),
    Replace {
        index: usize,
        section: BorderSection,
    },
    Delete(usize),
    Mirror(usize),
}
#[derive(Clone, Debug)]
pub struct Action {
    pub monitor: String,
    pub side: Side,
    /// Prevent an index from editing a different section after a reload or deletion.
    pub expected: Vec<BorderSection>,
    pub edit: Edit,
}
impl Action {
    pub fn new(monitor: &Monitor, side: Side, edit: Edit) -> Self {
        Self {
            monitor: monitor.id.clone(),
            side,
            expected: side.sections(&monitor.resistance).to_vec(),
            edit,
        }
    }
}
#[derive(Default)]
pub struct Editor {
    pub defaults: ResistanceDefaults,
    pub selected: Option<Selection>,
    side: Side,
    gesture: Option<bands::Gesture>,
    applet: applet::State,
}
/// Ephemeral feedback in physical layout coordinates, shared by every view of the gesture.
#[derive(Clone, Debug, PartialEq)]
pub struct Guide {
    pub monitor: String,
    pub side: Side,
    pub mm: f64,
    pub layout_mm: f64,
    pub kind: SnapKind,
}
impl Editor {
    pub fn active(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn cancel(&mut self) {
        self.gesture = None;
    }
    pub fn clear(&mut self) {
        self.selected = None;
        self.applet = applet::State::default();
        self.cancel();
    }
    pub fn select_monitor(&mut self, id: &str) {
        if self.selected.as_ref().is_some_and(|s| s.monitor != id) {
            self.clear();
        }
    }
    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        monitors: &[Monitor],
        id: &str,
        travel: f64,
    ) -> Option<Action> {
        self.select_monitor(id);
        let monitor = monitors.iter().find(|m| m.id == id)?;
        if let Some(s) = &self.selected {
            self.side = s.side;
        }
        let previous = self.side;
        ui.horizontal(|ui| {
            for side in Side::ALL {
                ui.selectable_value(&mut self.side, side, side.label());
            }
        });
        if previous != self.side {
            self.clear();
        }
        let side = self.side;
        let length = side.length(monitor.bounds);
        if !length.is_finite() || length <= 0.0 {
            ui.label("No measured edge for this monitor.");
            return None;
        }
        let sections = side.sections(&monitor.resistance);
        if self
            .selected
            .as_ref()
            .is_some_and(|s| s.index >= sections.len())
        {
            self.selected = None;
        }
        ui.label(format!(
            "{} edge · {:.1} mm · {}/16 sections",
            side.label(),
            length,
            sections.len()
        ));
        ui.small("Drag on an empty band to draw. Drag a section or its end handles to move or resize. Double-click to fill the free space. Ctrl suspends snapping; Esc cancels the gesture.");
        ui.small("Snap guides: white = midpoint · cyan = screen edge · yellow = section boundary.");
        let mut action = None;
        for (i, s) in sections.iter().enumerate() {
            let selected = self.selected.as_ref().is_some_and(|s| s.index == i);
            if ui
                .selectable_label(
                    selected,
                    format!("Section {} · {:.1}–{:.1} mm", i + 1, s.from(), s.to()),
                )
                .clicked()
            {
                self.selected = Some(Selection {
                    monitor: id.into(),
                    side,
                    index: i,
                });
            }
        }
        let free = geometry::largest(
            sections,
            Span {
                from: 0.0,
                to: length,
            },
        );
        if ui
            .add_enabled(
                sections.len() < MAX_SECTIONS && free.is_some(),
                egui::Button::new("Add section"),
            )
            .clicked()
        {
            let section = free.unwrap().section(new_section());
            action = Some(Edit::Insert(section));
        }
        if let Some(selected) = &self.selected {
            let i = selected.index;
            if let Some(original) = sections.get(i) {
                ui.separator();
                // IDs include section identity so a text edit cannot leak into another selection.
                ui.push_id((id, side, i), |ui| {
                    let mut s = *original;
                    let mut from = s.from();
                    let mut to = s.to();
                    let mut span_changed = false;
                    let mut values_changed = false;
                    egui::Grid::new("section-values").num_columns(2).show(ui, |ui| {
                        span_changed |= number(ui, "From", &mut from, length, true);
                        span_changed |= number(ui, "To", &mut to, length, true);
                    });
                    values_changed |= applet::channel(ui, "Move", &mut s, self.defaults, false);
                    values_changed |= applet::channel(ui, "Drag", &mut s, self.defaults, true);
                    if span_changed {
                        if let Some(span) = geometry::clamp(
                            Span::new(from, to),
                            geometry::gap(sections, Some(i), (original.from() + original.to()) / 2.0, length),
                        ) {
                            s = span.section(s);
                        }
                    }
                    if (span_changed || values_changed) && s != *original {
                        action = Some(Edit::Replace { index: i, section: s });
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Expand").clicked() {
                            let span = geometry::gap(sections, Some(i), (s.from() + s.to()) / 2.0, length);
                            action = Some(Edit::Replace { index: i, section: span.section(s) });
                        }
                        if ui.add_enabled(
                            geometry::mirror(monitors, selected, travel).is_some(),
                            egui::Button::new("Mirror"),
                        )
                        .on_hover_text("Copy onto the nearest facing edge, retaining its largest free interval.")
                        .clicked() {
                            action = Some(Edit::Mirror(i));
                        }
                        if ui.button("Delete section").clicked() {
                            action = Some(Edit::Delete(i));
                        }
                    });
                });
            }
        }
        if geometry::fully_blocked(sections, length) {
            ui.colored_label(egui::Color32::YELLOW,"This edge blocks moves and drags over its whole length. Ctrl bypass is unavailable on Wayland.");
        }
        ui.small("Amber: moves blocked · Purple: drags blocked · Red: both blocked · Blue: resistance · Grey: free");
        action.map(|edit| Action::new(monitor, side, edit))
    }
}
fn number(ui: &mut egui::Ui, name: &str, value: &mut f64, max: f64, enabled: bool) -> bool {
    let label = ui.label(name);
    let changed = ui
        .add_enabled(
            enabled,
            egui::DragValue::new(value)
                .range(0.0..=max)
                .clamp_existing_to_range(false)
                .speed(0.1)
                .suffix(" mm"),
        )
        .labelled_by(label.id)
        .changed();
    ui.end_row();
    changed
}
pub fn color(section: &BorderSection) -> egui::Color32 {
    match (section.move_block(), section.drag_block()) {
        (true, true) => egui::Color32::from_rgb(192, 57, 43),
        (true, false) => egui::Color32::from_rgb(224, 142, 43),
        (false, true) => egui::Color32::from_rgb(155, 89, 182),
        _ if section.move_resistance() > 0.0 || section.drag() > 0.0 => {
            egui::Color32::from_rgb(43, 132, 192)
        }
        _ => egui::Color32::from_gray(112),
    }
}
pub fn description(s: &BorderSection) -> String {
    let movement = if s.move_block() {
        "blocked".into()
    } else {
        format!("{:.1} mm", s.move_resistance())
    };
    let drag = if s.drag_block() {
        "blocked".into()
    } else {
        format!("{:.1} mm", s.drag())
    };
    format!("Move: {movement}\nDrag: {drag}")
}

/// A freshly drawn section follows both layout defaults.
fn new_section() -> BorderSection {
    let mut section = BorderSection::default();
    section.set_move_mode(lbm_layout::model::ResistanceMode::Resist);
    section.set_drag_mode(lbm_layout::model::ResistanceMode::Resist);
    section
}

/// Layout-wide defaults precede monitor selection in the properties panel.
pub fn defaults(ui: &mut egui::Ui, values: &mut ResistanceDefaults) -> bool {
    let mut changed = false;
    crate::theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.strong("Default resistance")
            .on_hover_text("Shared by sections set to Resist. Saved with this layout.");
        for (name, value, outside) in [
            ("Move", &mut values.movement, &mut values.move_without_section),
            ("Drag", &mut values.drag, &mut values.drag_without_section),
        ] {
            ui.push_id(name, |ui| {
                ui.horizontal(|ui| {
                    let label = ui.add_sized(egui::vec2(34., 20.), egui::Label::new(name));
                    changed |= ui.add(egui::DragValue::new(value)
                        .range(0.0..=f64::MAX).clamp_existing_to_range(false)
                        .speed(0.1).max_decimals(2).suffix(" mm"))
                        .labelled_by(label.id).changed();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("Outside sections").small())
                            .on_hover_text("Apply this default to crossings without a section. Explicit sections always keep their own mode.");
                        changed |= outside_switch(ui, outside, name);
                    });
                });
            });
        }
    });
    ui.add_space(8.);
    changed
}

fn outside_switch(ui: &mut egui::Ui, value: &mut bool, name: &str) -> bool {
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(30., 18.), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *value,
            format!("{name}: apply to crossings without a section"),
        )
    });
    let visuals = ui.style().interact_selectable(&response, *value);
    ui.painter().rect(
        rect,
        9.,
        if *value {
            crate::theme::ACCENT
        } else {
            crate::theme::RAISED
        },
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let x = if *value {
        rect.right() - 9.
    } else {
        rect.left() + 9.
    };
    ui.painter().circle_filled(
        egui::pos2(x, rect.center().y),
        6.,
        if *value {
            crate::theme::BACKGROUND
        } else {
            crate::theme::TEXT
        },
    );
    response.on_hover_text("Apply this default to crossings without a section. Explicit sections always keep their own mode.").changed()
}
