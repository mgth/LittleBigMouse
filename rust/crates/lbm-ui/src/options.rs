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
//!   distance. Saving one means sending a whole `LayoutDocument`, which is the same
//!   capability `Save` is still waiting on. They are **not drawn here**: a switch that
//!   cannot be kept is worse than a switch that is missing, because it looks like it
//!   worked.
//!
//! The split is not a simplification of the port. It is where the wire protocol already
//! cuts, and drawing the two halves together would hide that one of them has nowhere to go.
//!
//! What the Avalonia panel has and this does not, each for a reason worth stating:
//! the **rescue shortcut** (it needs a key-capture widget, which is its own piece of
//! work), the **excluded processes** (a list with add, remove, defaults and the seen
//! processes — `SaveOptions` carries it in a separate field, so it is a slice of its own),
//! and the **Ko-fi card**, which is a link and not a setting.

use lbm_layout::model::{LayoutOptions, PER_MODEL, PER_MONITOR};

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

/// The explanation under a setting.
///
/// `SettingRow`'s `Description` in the AXAML. Not decoration: several of these are
/// unguessable from their name — `BoundToAgent` above all — and the C# spells every one
/// of them out.
fn note(ui: &mut egui::Ui, description: &str) {
    if !description.is_empty() {
        ui.indent(description, |ui| {
            ui.label(
                egui::RichText::new(description)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    }
    ui.add_space(4.0);
}

/// A toggle row. `true` when the user moved it.
///
/// **The control carries the header, rather than a bare switch beside a label.** The
/// AXAML puts the name on the left and a `ToggleSwitch` on the right, and the first try
/// here did the same, handing the switch the label's id through `Response::labelled_by`.
/// It does not work: a plain `ui.label` puts its text in the accessibility node's
/// **value**, not its label, so the name resolves to nothing — the switch ends up with no
/// accessible name at all. A screen reader would announce "checkbox", and `egui_kittest`
/// could not find it either, which is how this was noticed. `ui.checkbox` gives the
/// control its own text, and accesskit takes a checkbox's name from its descendants.
fn toggle(ui: &mut egui::Ui, header: &str, description: &str, value: &mut bool) -> bool {
    let changed = ui.checkbox(value, header).changed();
    note(ui, description);
    changed
}

/// A row of one choice among a list, each with its own explanation.
fn choice(
    ui: &mut egui::Ui,
    header: &str,
    description: &str,
    items: &[(&str, &str)],
    value: &mut String,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_label(header)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for (id, what) in items {
                let picked = value == *id;
                if ui
                    .selectable_label(picked, *id)
                    .on_hover_text(*what)
                    .clicked()
                    && !picked
                {
                    *value = (*id).to_owned();
                    changed = true;
                }
            }
        });
    note(ui, description);
    changed
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(title).strong().size(15.0));
    ui.separator();
}

/// Draws the panel and edits `options` in place. `true` when something changed, which is
/// the caller's cue to save — the agent takes the whole set, so *what* changed is not its
/// business.
///
/// `elevated` says whether this build can offer the elevation switch at all: it is a
/// Windows notion (a process token, a scheduled task) and there is nothing behind it on
/// Linux, so the row is absent rather than present and dead.
pub fn panel(ui: &mut egui::Ui, options: &mut LayoutOptions, elevated: bool) -> bool {
    let mut changed = false;
    egui::ScrollArea::vertical().show(ui, |ui| {
        section(ui, "General");
        changed |= toggle(
            ui,
            "Check for updates automatically",
            "Look for new versions online in the background",
            &mut options.auto_update,
        );
        changed |= toggle(
            ui,
            "Load at startup",
            "Start LittleBigMouse when you sign in",
            &mut options.load_at_startup,
        );
        changed |= toggle(
            ui,
            "Start minimized to tray",
            "",
            &mut options.start_minimized,
        );
        changed |= toggle(
            ui,
            "Hide tray icon",
            "Reopen the window by launching LittleBigMouse again",
            &mut options.hide_tray_icon,
        );
        changed |= toggle(
            ui,
            "Stop the mouse engine with LittleBigMouse",
            "Off: the engine keeps routing the cursor if LittleBigMouse stops, and picks \
             up again when it comes back",
            &mut options.bound_to_agent,
        );
        if elevated {
            changed |= toggle(
                ui,
                "Start with elevated privileges",
                "Keeps working over elevated apps (admin accounts only)",
                &mut options.start_elevated,
            );
        }
        changed |= toggle(
            ui,
            "Enable VCP monitor control",
            "Brightness, contrast, inputs and smart-TV remotes over DDC/CI",
            &mut options.vcp_control,
        );
        // A sub-option of VCP: the C# hides it outright when VCP is off
        // (`IsVisible="{Binding Model.VcpControl}"`), because it only adds tools *inside*
        // the VCP panel.
        if options.vcp_control {
            ui.indent("vcp", |ui| {
                changed |= toggle(
                    ui,
                    "Enable experimental features",
                    "Argyll colour calibration and smart-TV remote test tools",
                    &mut options.experimental_features,
                );
            });
        }
        changed |= toggle(
            ui,
            "Activate debug tools",
            "Extra tools for troubleshooting, like viewing exported layouts",
            &mut options.debug_tools,
        );

        section(ui, "Daemon priority");
        changed |= choice(
            ui,
            "While active (hooked)",
            "Process priority of the mouse engine",
            &PRIORITIES,
            &mut options.priority,
        );
        changed |= choice(
            ui,
            "While inactive (unhooked)",
            "",
            &PRIORITIES,
            &mut options.priority_unhooked,
        );

        section(ui, "Layout");
        changed |= choice(
            ui,
            "Border values",
            "Where each monitor's bezel borders are stored",
            &BORDER_VALUES,
            &mut options.border_values,
        );
        changed |= toggle(
            ui,
            "Warn before monitor actions",
            "Ask before attaching, detaching or making a monitor primary",
            &mut options.show_monitor_action_warning,
        );

        // Said, rather than left as an absence the reader has to notice. Every setting
        // above is app-wide; the ones that are not are stored in the layout's own profile
        // and need a capability the frontend has not got yet.
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(
                "Overlaps, discontinuity, the crossing algorithm and the loops belong to \
                 this layout rather than to the app. Editing them needs the same layout \
                 document Save is waiting on, so they are not offered here yet.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
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
