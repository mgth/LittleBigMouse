//! The excluded processes: the applications the engine stands aside for.
//!
//! The list the user edits **is** the list the daemon filters on — one list, not a view
//! of one — so this is the section where being wrong costs the most. Two consequences run
//! through it:
//!
//! * **A window that has not read the list must not offer to edit it.** The list lives in
//!   its own file (`Excluded.txt`), not in the options, and everything else in this
//!   window worked without it: `SaveOptions` and `SaveLayout` both deliberately leave the
//!   field out (see `lbm_app::settings`). Offering an empty list to edit would let the
//!   user "remove" exclusions that are simply not loaded, and then save that.
//! * **The decisions are not here.** What counts as a duplicate is separator-insensitive
//!   (`\` == `/`, the daemon's own matching, `ExcludedProcessDefaults.ContainsEntry`) and
//!   that rule lives in `lbm-store` with the defaults it guards. This module reports what
//!   the user did and lets the caller — which has the store — decide what it means.

/// What the user did to the list. The caller applies it; nothing here changes anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Did {
    /// Add this pattern, typed or picked.
    Add(String),
    Remove(String),
    /// Top the list up with the built-in defaults.
    AddDefaults,
}

/// Whether the Exclude button means anything: `AddExcludedProcessCommand`'s guard —
/// something that is not whitespace, and not already in the list.
///
/// Exact comparison, deliberately: the separator-insensitive rule is for the *defaults*
/// top-up, which must not duplicate a Windows-style entry with its Linux twin. A user
/// typing a second spelling of the same path is entitled to have both.
pub fn can_add(list: &[String], pattern: &str) -> bool {
    !pattern.trim().is_empty() && !list.iter().any(|e| e == pattern)
}

/// Draws the section. `None` when the list could not be read, in which case it says so
/// instead of offering an empty list to edit.
pub fn panel(
    ui: &mut egui::Ui,
    list: Option<&[String]>,
    pattern: &mut String,
    seen: &[String],
) -> Option<Did> {
    let mut did = None;
    ui.horizontal(|ui| {
        crate::settings_icons::Icon::Excluded.show(ui);
        ui.label(
            egui::RichText::new("Application exclusions")
                .strong()
                .size(15.0),
        );
    });
    ui.add(egui::Label::new(egui::RichText::new("Pause cursor routing while these applications are focused. Changes are saved automatically.").small().color(crate::theme::MUTED)).wrap());
    ui.add_space(8.0);
    let Some(list) = list else {
        ui.label(
            egui::RichText::new(
                "Waiting for the agent to load your exclusions. Editing will be available once the list is loaded.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
        );
        return None;
    };

    if list.is_empty() {
        ui.label(
            egui::RichText::new("Nothing is excluded.")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    }
    for entry in list {
        crate::theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 36.0).max(60.0);
                ui.add_sized(
                    [width, 0.0],
                    egui::Label::new(entry).wrap().halign(egui::Align::Min),
                );
                let response = ui.button("×").on_hover_text("Remove exclusion");
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        format!("Remove {entry}"),
                    )
                });
                if response.clicked() {
                    did = Some(Did::Remove(entry.clone()));
                }
            });
        });
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let typed = ui.add(
            egui::TextEdit::singleline(pattern)
                .hint_text("game.exe or pattern")
                .desired_width((ui.available_width() - 88.0).max(80.0)),
        );
        let ready = can_add(list, pattern);
        // Enter is how a text field is submitted, and the button is how it is discovered.
        let entered = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui
            .add_enabled(ready, egui::Button::new("Exclude"))
            .clicked()
            || entered)
            && ready
        {
            did = Some(Did::Add(pattern.trim().to_owned()));
        }
    });

    if ui
        .button("Add defaults")
        .on_hover_text("Add the built-in exclusions that are not already in the list")
        .clicked()
    {
        did = Some(Did::AddDefaults);
    }

    if !seen.is_empty() {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Seen in the foreground — click one to use it as a pattern")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        for process in seen {
            // Already covered ones are shown as such rather than hidden: the list is how
            // you check that an exclusion took.
            let covered = list.iter().any(|e| process.contains(e.as_str()));
            if ui
                .selectable_label(covered, process)
                .on_hover_text(if covered {
                    "Already excluded"
                } else {
                    "Use as pattern"
                })
                .clicked()
            {
                pattern.clear();
                pattern.push_str(process);
            }
        }
    }
    did
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_is_addable_once_and_never_blank() {
        let list = vec!["game.exe".to_owned()];
        assert!(can_add(&list, "other.exe"));
        assert!(!can_add(&list, "game.exe"), "already there");
        assert!(!can_add(&list, ""), "nothing to add");
        assert!(!can_add(&list, "   "), "whitespace is nothing");
    }

    /// Exact, not separator-insensitive: that rule belongs to the defaults top-up, and a
    /// user who types a second spelling of a path is entitled to keep both.
    #[test]
    fn two_spellings_of_a_path_are_two_patterns() {
        let list = vec![r"\steamapps\".to_owned()];
        assert!(can_add(&list, "/steamapps/"));
    }
}
