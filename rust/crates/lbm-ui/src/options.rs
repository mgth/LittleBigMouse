//! The settings panel: `LayoutOptions.axaml`, as a function of the options.
//!
//! **Two families, and the agent's API draws the line between them.** What looks like one
//! panel in the Avalonia app is two kinds of setting stored in two places:
//!
//! * **App-wide** — `GlobalOptionsDto`, `options.json`, sent whole by the agent's
//!   `SaveOptions`. Auto-update, the tray, the daemon's priority, where bezel borders are
//!   kept. These are what this module edits, because they are the ones a frontend can
//!   read *and* write today.
//! * **Per-layout** — `LayoutOptionsDto`, inside the layout's own profile: allow
//!   overlaps, allow discontinuity, the crossing algorithm, the loops, the travel
//!   distance. These go with the layout, in the `LayoutDocument` the Save button sends.
//!
//! The split is not a simplification of the port: it is where the wire protocol already
//! cuts. It shows in the panel's **behaviour**, which is why [`Changed`] keeps the two
//! apart — an app-wide setting is written the moment it is moved, a per-layout one is an
//! edit like dragging a screen and waits for Save. The C# does the same, `SaveLive`
//! against the layout's own document.
//!
//! Excluded processes are loaded separately by the caller. The shared Ko-fi card
//! stays above the scrolling settings, as in Avalonia.

use crate::settings_icons::Icon;
use lbm_layout::model::{LayoutOptions, PER_MODEL, PER_MONITOR};

/// The crossing algorithms, wire spelling first — `LbmOptionsViewModel.AlgorithmList`.
pub const ALGORITHMS: [(&str, &str); 2] = [
    ("Strait", "Simple and highly CPU-efficient transition"),
    (
        "Cross",
        "Corner crossing: in a direction-friendly manner, allows traversal through corners",
    ),
];

/// Which half of the settings the user moved.
///
/// Two flags rather than one, because the two halves are kept in different places and
/// saved by different requests: an app-wide setting goes to the agent at once, a
/// per-layout one is an edit that waits for Save with the rest of the layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changed {
    /// Send `SaveOptions` now.
    pub app: bool,
    /// The layout is edited; Save will carry it.
    pub layout: bool,
}

impl Changed {
    pub fn any(self) -> bool {
        self.app || self.layout
    }
}

/// The daemon priorities, in the order the C# combo lists them, with the wire spelling
/// first — `LbmOptionsViewModel.PriorityList`.
pub const PRIORITIES: [(&str, &str); 5] = [
    ("Idle", "Only when nothing else wants the processor"),
    ("Below", "Below normal"),
    ("Normal", "Normal"),
    ("Above", "Above normal"),
    ("High", "High — use if the cursor stutters"),
];

/// Where a monitor's bezel borders are kept: `LbmOptionsViewModel.BorderValuesList`.
pub const BORDER_VALUES: [(&str, &str); 2] = [
    (
        PER_MODEL,
        "Shared by every monitor of the same model — measure once",
    ),
    (PER_MONITOR, "Each monitor keeps its own"),
];

/// Settings categories, kept independently of the monitor view and its mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    General,
    Mouse,
    Layout,
    Applications,
    Advanced,
    /// The multi-PC mode (docs/multi-pc.md). Not in [`Page::ALL`]: it is offered only
    /// while the agent announces `MultiPc`, so a window talking to an agent without it
    /// shows the drawer exactly as before — see [`Page::offered`].
    Machines,
}

impl Page {
    pub const ALL: [Self; 5] = [
        Self::General,
        Self::Mouse,
        Self::Layout,
        Self::Applications,
        Self::Advanced,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Mouse => "Mouse",
            Self::Layout => "Layout",
            Self::Applications => "Applications",
            Self::Advanced => "Advanced",
            Self::Machines => "Machines",
        }
    }

    /// The pages to offer: every one of [`Page::ALL`], and Machines after them when the
    /// agent can run the multi-PC mode.
    pub fn offered(machines: bool) -> &'static [Page] {
        const WITH_MACHINES: [Page; 6] = [
            Page::General,
            Page::Mouse,
            Page::Layout,
            Page::Applications,
            Page::Advanced,
            Page::Machines,
        ];
        if machines {
            &WITH_MACHINES
        } else {
            &Page::ALL
        }
    }
}

pub fn navigation(ui: &mut egui::Ui, page: &mut Page) {
    navigation_among(ui, page, &Page::ALL);
}

/// The same, over the pages given. A page that is no longer offered falls back to the
/// first one, so a drawer left on Machines does not stay on a page that has gone.
pub fn navigation_among(ui: &mut egui::Ui, page: &mut Page, pages: &[Page]) {
    if !pages.contains(page) {
        if let Some(first) = pages.first() {
            *page = *first;
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        ui.spacing_mut().button_padding = egui::vec2(7.0, 6.0);
        for item in pages {
            ui.selectable_value(page, *item, item.label());
        }
    });
}

/// A bounded overlay, like the C# SplitView pane: the map keeps its dimensions and
/// the toolbar and Save controls remain available outside the pane's bounds.
/// Escape can be reserved for the shortcut recorder by the caller.
pub fn drawer(
    ctx: &egui::Context,
    bounds: egui::Rect,
    open: &mut bool,
    page: &mut Page,
    escape_closes: bool,
    content: impl FnMut(&mut egui::Ui, Page),
) {
    drawer_among(ctx, bounds, open, page, &Page::ALL, escape_closes, content);
}

/// [`drawer`] over the pages given ([`Page::offered`]).
pub fn drawer_among(
    ctx: &egui::Context,
    bounds: egui::Rect,
    open: &mut bool,
    page: &mut Page,
    pages: &[Page],
    escape_closes: bool,
    mut content: impl FnMut(&mut egui::Ui, Page),
) {
    if !*open {
        return;
    }
    if escape_closes && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        *open = false;
        return;
    }
    egui::Area::new(egui::Id::new("settings-drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(bounds.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_min_size(bounds.size());
            ui.set_max_size(bounds.size());
            let outside = ui.interact(bounds, ui.id().with("dismiss"), egui::Sense::click());
            ui.painter()
                .rect_filled(bounds, 0.0, egui::Color32::from_black_alpha(72));
            let pane = egui::Rect::from_min_size(
                bounds.min,
                egui::vec2(bounds.width().min(460.0), bounds.height()),
            );
            ui.painter().rect_filled(pane, 0.0, crate::theme::SURFACE);
            ui.painter().vline(
                pane.right(),
                pane.y_range(),
                egui::Stroke::new(1.0, crate::theme::BORDER),
            );
            // The panel itself consumes clicks; only the uncovered backdrop dismisses.
            ui.interact(pane, ui.id().with("pane"), egui::Sense::click());
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(pane.shrink(16.0))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Settings");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("Close")
                                .on_hover_text("Close settings · Esc")
                                .clicked()
                            {
                                *open = false;
                            }
                        });
                    });
                    ui.add_space(5.0);
                    navigation_among(ui, page, pages);
                    ui.add_space(5.0);
                    ui.separator();
                    let compact_key = ui.id().with(("support-compact", *page as u8));
                    let compact = ui.available_height() < 450.
                        || ctx.data(|d| d.get_temp::<bool>(compact_key).unwrap_or(false));
                    crate::support::card(ui, compact);
                    ui.add_space(8.);
                    let scroll = egui::ScrollArea::vertical()
                        .id_salt(("settings-content", *page as u8))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            content(ui, *page);
                        });
                    ctx.data_mut(|d| d.insert_temp(compact_key, scroll.state.offset.y > 1.));
                },
            );
            if outside.clicked()
                && outside
                    .interact_pointer_pos()
                    .is_some_and(|pos| !pane.contains(pos))
            {
                *open = false;
            }
        });
}

pub(crate) fn note(ui: &mut egui::Ui, description: &str) {
    if !description.is_empty() {
        ui.add(
            egui::Label::new(
                egui::RichText::new(description)
                    .size(11.5)
                    .color(crate::theme::MUTED),
            )
            .wrap(),
        );
    }
}

pub(crate) fn group(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(9.0);
    ui.label(egui::RichText::new(title).size(13.0).strong());
    crate::theme::card()
        .fill(crate::theme::BACKGROUND)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 9.0;
            content(ui);
        });
}

/// The label and description share a column, while every control shares the right edge.
fn row<R>(
    ui: &mut egui::Ui,
    icon: Icon,
    header: &str,
    description: &str,
    width: f32,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        icon.show(ui);
        let text_width = (ui.available_width() - width - 12.0).max(40.0);
        ui.allocate_ui_with_layout(
            egui::vec2(text_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.add(egui::Label::new(egui::RichText::new(header).strong()).wrap());
                note(ui, description);
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
            .inner
    })
    .inner
}

pub(crate) fn toggle(
    ui: &mut egui::Ui,
    icon: Icon,
    header: &str,
    description: &str,
    value: &mut bool,
) -> bool {
    row(ui, icon, header, description, 36.0, |ui| {
        let (rect, mut response) =
            ui.allocate_exact_size(egui::vec2(36.0, 20.0), egui::Sense::click());
        if response.clicked() {
            *value = !*value;
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, header)
        });
        let visuals = ui.style().interact_selectable(&response, *value);
        let fill = if *value {
            crate::theme::ACCENT
        } else {
            crate::theme::RAISED
        };
        ui.painter().rect(
            rect,
            10.0,
            fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let x = if *value {
            rect.right() - 10.0
        } else {
            rect.left() + 10.0
        };
        ui.painter().circle_filled(
            egui::pos2(x, rect.center().y),
            7.0,
            if *value {
                crate::theme::BACKGROUND
            } else {
                crate::theme::TEXT
            },
        );
        response.changed()
    })
}

fn caption(value: &str) -> &str {
    match value {
        "Strait" => "Straight",
        "Cross" => "Corners",
        PER_MODEL => "Per model",
        PER_MONITOR => "Per monitor",
        "Below" => "Below normal",
        "Above" => "Above normal",
        other => other,
    }
}

fn choice(
    ui: &mut egui::Ui,
    icon: Icon,
    header: &str,
    description: &str,
    items: &[(&str, &str)],
    value: &mut String,
) -> bool {
    row(ui, icon, header, description, 120.0, |ui| {
        let mut changed = false;
        let result = egui::ComboBox::from_id_salt(header)
            .width(120.0)
            .selected_text(caption(value))
            .show_ui(ui, |ui| {
                for (id, what) in items {
                    let picked = value == *id;
                    if ui
                        .selectable_label(picked, caption(id))
                        .on_hover_text(*what)
                        .clicked()
                        && !picked
                    {
                        *value = (*id).to_owned();
                        changed = true;
                    }
                }
            });
        result.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, ui.is_enabled(), header)
        });
        changed
    })
}

fn number(
    ui: &mut egui::Ui,
    icon: Icon,
    header: &str,
    description: &str,
    format: (std::ops::RangeInclusive<f64>, f64, &str),
    value: &mut f64,
) -> bool {
    let (range, step, unit) = format;
    row(ui, icon, header, description, 86.0, |ui| {
        let response = ui.add(
            egui::DragValue::new(value)
                .range(range)
                .speed(step)
                .suffix(format!(" {unit}")),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::DragValue, ui.is_enabled(), header)
        });
        response.changed()
    })
}

/// Edits one category; the caller owns the single scroll area, including exclusions.
/// Global settings still save immediately, layout settings use the main Save button.
pub fn panel(
    ui: &mut egui::Ui,
    options: &mut LayoutOptions,
    elevated: bool,
    rescue: &mut crate::shortcut::Recording,
    unavailable: Option<&str>,
    page: Page,
) -> Changed {
    let mut what = Changed::default();
    match page {
        Page::General => {
            note(ui, "Application preferences · saved automatically");
            group(ui, "Startup", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Startup,
                    "Load at startup",
                    "Start LittleBigMouse when you sign in",
                    &mut options.load_at_startup,
                );
                ui.separator();
                what.app |= toggle(
                    ui,
                    Icon::Startup,
                    "Start minimized to tray",
                    "Keep the main window closed at startup",
                    &mut options.start_minimized,
                );
                if elevated {
                    ui.separator();
                    what.app |= toggle(
                        ui,
                        Icon::Shield,
                        "Start with elevated privileges",
                        "Keep working over administrator apps",
                        &mut options.start_elevated,
                    );
                }
            });
            group(ui, "Appearance", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Virtual,
                    "Hide tray icon",
                    "Launch LittleBigMouse again to reopen its window",
                    &mut options.hide_tray_icon,
                );
            });
            group(ui, "Updates", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Update,
                    "Check for updates automatically",
                    "Look for new versions in the background",
                    &mut options.auto_update,
                );
            });
        }
        Page::Mouse => {
            note(
                ui,
                "Cursor behaviour for this layout · use Save to keep changes",
            );
            group(ui, "Crossing between screens", |ui| {
                what.layout |= choice(
                    ui,
                    Icon::Algorithm,
                    "Crossing algorithm",
                    "Straight transitions or crossing through corners",
                    &ALGORITHMS,
                    &mut options.algorithm,
                );
                ui.separator();
                what.layout |= number(
                    ui,
                    Icon::Ruler,
                    "Max travel distance",
                    "Maximum jump between screens",
                    (0.0..=10_000.0, 10.0, "px"),
                    &mut options.max_travel_distance,
                );
                ui.separator();
                what.layout |= toggle(
                    ui,
                    Icon::LoopH,
                    "Horizontal loop",
                    "Re-enter from the opposite horizontal edge",
                    &mut options.loop_x,
                );
                ui.separator();
                what.layout |= toggle(
                    ui,
                    Icon::LoopV,
                    "Vertical loop",
                    "Re-enter from the opposite vertical edge",
                    &mut options.loop_y,
                );
            });
            group(ui, "Pointer", |ui| {
                what.layout |= toggle(
                    ui,
                    Icon::Mouse,
                    "Adjust pointer speed",
                    "Keep speed consistent across screen densities",
                    &mut options.adjust_speed,
                );
                ui.separator();
                what.layout |= toggle(
                    ui,
                    Icon::Pointer,
                    "Adjust pointer size",
                    "Keep size consistent across screen densities",
                    &mut options.adjust_pointer,
                );
            });
            group(ui, "Games", |ui| {
                what.layout |= toggle(
                    ui,
                    Icon::Gamepad,
                    "Pause in games",
                    "Pause crossing while a game captures the mouse",
                    &mut options.freelook_enabled,
                );
                if options.freelook_enabled {
                    ui.separator();
                    what.layout |= number(
                        ui,
                        Icon::Gamepad,
                        "Detection interval",
                        "0 checks on every mouse event",
                        (0.0..=1000.0, 10.0, "ms"),
                        &mut options.freelook_check_interval,
                    );
                }
            });
            group(ui, "Rescue shortcut", |ui| {
                ui.horizontal(|ui| {
                    Icon::Shield.show(ui);
                    note(
                        ui,
                        "Hold this shortcut to free a trapped cursor. Saved automatically.",
                    );
                });
                if let Some(recorded) =
                    crate::shortcut::recorder(ui, &options.rescue_shortcut, rescue, unavailable)
                {
                    options.rescue_shortcut = recorded;
                    what.app = true;
                }
            });
        }
        Page::Layout => {
            note(
                ui,
                "Screen arrangement for this layout · use Save to keep changes",
            );
            group(ui, "Screen arrangement", |ui| {
                what.layout |= toggle(
                    ui,
                    Icon::Overlap,
                    "Allow overlaps",
                    "Screens may overlap in the layout",
                    &mut options.allow_overlaps,
                );
                ui.separator();
                what.layout |= toggle(
                    ui,
                    Icon::Gap,
                    "Allow discontinuity",
                    "Allow gaps between monitors",
                    &mut options.allow_discontinuity,
                );
                ui.separator();
                what.layout |= number(
                    ui,
                    Icon::Ruler,
                    "Minimal edge overlap",
                    "Shared edge needed to join screens; 0 allows simple contact",
                    (0.0..=1000.0, 5.0, "mm"),
                    &mut options.minimal_edge_overlap,
                );
            });
            group(ui, "Monitor measurements", |ui| {
                what.app |= choice(
                    ui,
                    Icon::Ruler,
                    "Border values",
                    "Share bezel measurements by model, or measure each monitor separately",
                    &BORDER_VALUES,
                    &mut options.border_values,
                );
                note(ui, "Measurement storage is saved automatically.");
            });
            group(ui, "Monitor actions", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Warning,
                    "Warn before monitor actions",
                    "Confirm before attaching, detaching or changing the primary monitor",
                    &mut options.show_monitor_action_warning,
                );
                note(ui, "This preference is saved automatically.");
            });
        }
        Page::Advanced => {
            note(ui, "Engine and optional tools · saved automatically");
            group(ui, "Mouse engine", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Priority,
                    "Stop the mouse engine with LittleBigMouse",
                    "When off, cursor routing continues after closing the app",
                    &mut options.bound_to_agent,
                );
                ui.separator();
                what.app |= choice(
                    ui,
                    Icon::Priority,
                    "While active (hooked)",
                    "Engine process priority",
                    &PRIORITIES,
                    &mut options.priority,
                );
                ui.separator();
                what.app |= choice(
                    ui,
                    Icon::Priority,
                    "While inactive (unhooked)",
                    "Priority while cursor routing is stopped",
                    &PRIORITIES,
                    &mut options.priority_unhooked,
                );
            });
            group(ui, "Display Lab", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Vcp,
                    "Enable Display Lab",
                    "Monitor controls, network remotes and test patterns",
                    &mut options.vcp_control,
                );
                if options.vcp_control {
                    ui.separator();
                    what.app |= toggle(
                        ui,
                        Icon::Experimental,
                        "Enable experimental features",
                        "Colour calibration and smart-TV test tools",
                        &mut options.experimental_features,
                    );
                }
            });
            group(ui, "Diagnostics", |ui| {
                what.app |= toggle(
                    ui,
                    Icon::Debug,
                    "Activate debug tools",
                    "Extra troubleshooting and layout inspection tools",
                    &mut options.debug_tools,
                );
            });
        }
        Page::Applications => {} // The separately loaded excluded list is owned by the caller.
        Page::Machines => {}     // Drawn from the agent's multi-PC state: see `crate::machines`.
    }
    what
}

/// The keys that can exempt one touch gesture, wire spelling first.
pub const TOUCH_MODIFIERS: [(&str, &str); 5] = [
    ("None", "No key"),
    ("Ctrl", "Hold Ctrl"),
    ("Alt", "Hold Alt"),
    ("Shift", "Hold Shift"),
    ("Win", "Hold the Windows key"),
];

/// `ids` (`;`-separated) with `id` added or removed, sorted as the C# writes them.
pub fn with_touch_display(ids: &str, id: &str, selected: bool) -> String {
    let mut all: Vec<&str> = ids
        .split(';')
        .filter(|known| !known.is_empty() && *known != id)
        .collect();
    if selected {
        all.push(id);
    }
    all.sort_unstable();
    all.join(";")
}

/// A line of executable names, committed when the field is left: every keystroke would
/// otherwise be a save and a reload of the running engine.
fn app_list(ui: &mut egui::Ui, label: &str, hint: &str, value: &mut String) -> bool {
    ui.label(label);
    let id = ui.id().with(label);
    let mut text = ui
        .data(|data| data.get_temp::<String>(id))
        .unwrap_or_else(|| value.clone());
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label)
    });
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return false;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let changed = response.lost_focus() && text != *value;
    if changed {
        *value = text;
    }
    changed
}

/// The touchscreen settings (Windows only: the caller decides). `displays` is what can
/// be selected, as `(source id, label)`. Returns whether an app-wide setting changed.
pub fn touch_panel(
    ui: &mut egui::Ui,
    options: &mut LayoutOptions,
    displays: &[(String, String)],
) -> bool {
    let mut changed = false;
    group(ui, "Touchscreen", |ui| {
        changed |= toggle(
            ui,
            Icon::Mouse,
            "Keep mouse position after touch",
            "After using the touchscreen, move the mouse to resume from its previous position",
            &mut options.touch_mouse_independent,
        );
        if !options.touch_mouse_independent {
            return;
        }
        ui.separator();
        changed |= toggle(
            ui,
            Icon::Mouse,
            "Include stylus",
            "Keep the mouse position after pen use and apply the same focus settings",
            &mut options.stylus_mouse_independent,
        );
        ui.separator();
        changed |= toggle(
            ui,
            Icon::Mouse,
            "Restore keyboard focus after touch",
            "Return typing to the previous app after touch",
            &mut options.restore_keyboard_focus,
        );
        if options.restore_keyboard_focus {
            ui.separator();
            changed |= toggle(
                ui,
                Icon::Mouse,
                "Restore when the mouse moves",
                "Keep focus on the touched app until you move the physical mouse",
                &mut options.focus_restore_on_mouse_move,
            );
            ui.separator();
            let mut delay = f64::from(options.focus_restore_delay);
            let moved = ui
                .add_enabled_ui(!options.focus_restore_on_mouse_move, |ui| {
                    number(
                        ui,
                        Icon::Mouse,
                        "Focus delay",
                        "Wait after lifting your finger",
                        (0.0..=5000.0, 20.0, "ms"),
                        &mut delay,
                    )
                })
                .inner;
            if moved {
                options.focus_restore_delay = delay.round() as i32;
                changed = true;
            }
            ui.separator();
            note(
                ui,
                "Executable names separated by semicolons. These rules only affect focus.",
            );
            changed |= app_list(
                ui,
                "Keep focus in these touched apps",
                "chrome.exe; notepad.exe",
                &mut options.focus_keep_apps,
            );
            changed |= app_list(
                ui,
                "Restore focus only after touching these apps",
                "Empty = all apps except the list above",
                &mut options.focus_restore_apps,
            );
        }
        ui.separator();
        changed |= toggle(
            ui,
            Icon::Mouse,
            "Use on all displays",
            "Turn off to choose which displays these settings apply to",
            &mut options.touch_all_displays,
        );
        if !options.touch_all_displays {
            note(ui, "With no display selected, these settings do nothing.");
            for (id, label) in displays {
                let mut selected = options
                    .touch_display_ids
                    .split(';')
                    .any(|known| known == id);
                if ui.checkbox(&mut selected, label).changed() {
                    options.touch_display_ids =
                        with_touch_display(&options.touch_display_ids, id, selected);
                    changed = true;
                }
            }
        }
        ui.separator();
        changed |= choice(
            ui,
            Icon::Mouse,
            "Temporary override",
            "Hold this key before tapping to use the touched app normally for that gesture",
            &TOUCH_MODIFIERS,
            &mut options.touch_override_modifier,
        );
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every value the C# offers has to be one this panel can round-trip: an option set
    /// to something the list does not contain would be silently unselectable.
    #[test]
    fn the_stored_defaults_are_values_the_panel_can_show() {
        let o = LayoutOptions::default();
        assert!(
            PRIORITIES.iter().any(|(id, _)| *id == o.priority),
            "the default priority {:?} is not in the list",
            o.priority
        );
        assert!(
            PRIORITIES.iter().any(|(id, _)| *id == o.priority_unhooked),
            "the default unhooked priority {:?} is not in the list",
            o.priority_unhooked
        );
        assert!(
            BORDER_VALUES.iter().any(|(id, _)| *id == o.border_values),
            "the default border values {:?} are not in the list",
            o.border_values
        );
    }

    /// The border-values list must be exactly the two the model knows, spelled the way
    /// the wire spells them — `PerModel`/`PerMonitor` reach the store as written.
    #[test]
    fn the_border_values_are_the_models_own_two() {
        let ids: Vec<&str> = BORDER_VALUES.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, vec![PER_MODEL, PER_MONITOR]);
    }

    // What the panel *does* — that a toggle reaches the options, that an untouched pass
    // reports no change — cannot be asked of these constants. It is in
    // `tests/options.rs`, through the accessibility tree, and the agreement with
    // `GlobalOptionsDto` is in `lbm-app`, which is where the panel and the DTO meet.
}
