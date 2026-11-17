//! Selected-monitor properties use window-sized text, independently of the map zoom.
use lbm_layout::model::DisplaySize;

pub fn panel(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    let panel = if ui.available_width() >= 760.0 {
        egui::Panel::right("monitor-properties-side")
            .default_size(320.0)
            .min_size(280.0)
            .max_size(430.0)
    } else {
        egui::Panel::bottom("monitor-properties-bottom")
            .default_size(260.0)
            .min_size(100.0)
            .max_size((ui.available_height() * 0.6).max(100.0))
    };
    panel.resizable(true).show(ui, content);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edit {
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

pub fn size(ui: &mut egui::Ui, value: &Size) -> Vec<Edit> {
    let mut edits = Vec::new();
    ui.label("Dimensions in mm, in the screen's current orientation.");
    ui.label(format!("Panel size is shared by model {}.", value.model));
    let mut locked = value.locked;
    if ui
        .checkbox(&mut locked, "Keep aspect ratio")
        .on_hover_text("Editing aid for this session; the dimensions are saved.")
        .changed()
    {
        edits.push(Edit::Lock(locked));
    }
    let p = value.physical;
    egui::Grid::new("size-fields")
        .num_columns(2)
        .show(ui, |ui| {
            for (label, current, min, edit) in [
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
            ] {
                let label = ui.label(label);
                let mut number = current;
                if ui
                    .add(
                        egui::DragValue::new(&mut number)
                            .speed(0.1)
                            .range(min..=f64::MAX)
                            .clamp_existing_to_range(false)
                            .suffix(" mm"),
                    )
                    .labelled_by(label.id)
                    .changed()
                {
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
    ui.label(format!(
        "Diagonal: {:.2} in",
        p.width.hypot(p.height) / 25.4
    ));
    edits
}

/// Both the report and the layout are read-only here; long device paths wrap.
pub fn rows(ui: &mut egui::Ui, rows: &[(String, String)]) {
    for (name, value) in rows {
        ui.label(egui::RichText::new(name).weak());
        ui.add(egui::Label::new(value).wrap().selectable(true));
    }
}
