//! The resistance editor: values in the properties panel, intervals on the map.
mod bands;
pub mod geometry;
pub use bands::{band, owner, segment};
use geometry::{Span, MAX_SECTIONS};
use lbm_layout::{
    geo::Rect,
    model::{BorderResistance, BorderSection},
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
    pub selected: Option<Selection>,
    side: Side,
    gesture: Option<bands::Gesture>,
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
            let section = free
                .unwrap()
                .section(BorderSection::new(0.0, 0.0, 0.0, true, 0.0, false));
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
                    let mut movement = s.move_resistance();
                    let mut drag = s.drag();
                    let mut move_block = s.move_block();
                    let mut drag_block = s.drag_block();
                    let mut span_changed = false;
                    let mut values_changed = false;
                    egui::Grid::new("section-values").num_columns(2).show(ui, |ui| {
                        span_changed |= number(ui, "From", &mut from, length, true);
                        span_changed |= number(ui, "To", &mut to, length, true);
                        values_changed |= number(ui, "Move resistance", &mut movement, f64::MAX, !move_block);
                        values_changed |= number(ui, "Drag resistance", &mut drag, f64::MAX, !drag_block);
                    });
                    values_changed |= ui.checkbox(&mut move_block, "Block moves").changed();
                    values_changed |= ui.checkbox(&mut drag_block, "Block drags").changed();
                    s.set_move_resistance(movement);
                    s.set_drag(drag);
                    s.set_move_block(move_block);
                    s.set_drag_block(drag_block);
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
