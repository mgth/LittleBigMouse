//! The rescue shortcut: recording one, and saying whether it will work.
//!
//! The rescue frees the cursor when it is trapped somewhere the user cannot click — so a
//! rescue that silently does not exist is worse than none at all, because they only find
//! out at the moment they need it. That shapes the whole module: **everything this knows
//! about the shortcut's fate is shown**, and nothing is assumed.
//!
//! Two different facts, from two different places, and the panel must not blur them:
//!
//! * **Is it well formed?** Decided here, by `lbm_ipc::shortcut::Shortcut::parse` — the
//!   very parser the daemon registers through, so what this accepts is what will be
//!   registered. A bare key is refused: registered globally it would swallow that key for
//!   every application on the desktop.
//! * **Did the registration take?** Only the hook knows. It says so with a
//!   `ShortcutUnavailable` event, because something else owns the combination (Windows)
//!   or the desktop bound it to no key (Linux). The window cannot work that out and does
//!   not try — it repeats what the hook said.

use lbm_ipc::shortcut::Shortcut;

/// What the recorder is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Recording {
    /// Showing the shortcut.
    #[default]
    No,
    /// Waiting for a combination.
    Yes,
}

/// The shortcut as the user would write it, from a key and the modifiers held with it.
///
/// The order is the one the default is written in — `Ctrl+Alt+Shift+M` — so a recorded
/// shortcut and a stored one look the same, and the grammar is case-insensitive anyway.
pub fn spell(modifiers: egui::Modifiers, key: egui::Key) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    // `command` is Ctrl on Windows and Linux and already counted; on macOS it is the
    // other one. `mac_cmd` is the only unambiguous name for the Windows/Super key here.
    if modifiers.mac_cmd {
        parts.push("Win");
    }
    parts.push(key.name());
    parts.join("+")
}

/// Why this shortcut will not do, in the user's terms — or `None` when it is fine.
///
/// The parser answers yes or no; this says which of the two reasons it was, because
/// "invalid" is not something a person can act on.
pub fn why_not(text: &str) -> Option<&'static str> {
    if Shortcut::parse(text).is_some() {
        return None;
    }
    if text.trim().is_empty() {
        return Some("No rescue shortcut: the cursor cannot be freed by keyboard.");
    }
    // The grammar refuses exactly two things beyond an unknown key name: no modifier,
    // and no key.
    let has_modifier = text.split('+').any(|part| {
        matches!(
            part.trim().to_ascii_lowercase().as_str(),
            "ctrl" | "control" | "alt" | "shift" | "maj" | "win" | "super" | "meta" | "cmd"
        )
    });
    if !has_modifier {
        return Some(
            "Needs at least one of Ctrl, Alt, Shift or Win: a bare key registered \
             globally would be taken from every application.",
        );
    }
    Some("Needs exactly one key besides the modifiers.")
}

/// Draws the recorder. Returns the new shortcut when the user just recorded one.
///
/// `unavailable` is what the hook last said about a shortcut it could not arm — shown as
/// it is, and shown even when the text is well formed, because being well formed is not
/// the same as being registered.
pub fn recorder(
    ui: &mut egui::Ui,
    shortcut: &str,
    recording: &mut Recording,
    unavailable: Option<&str>,
) -> Option<String> {
    let mut recorded = None;
    ui.horizontal(|ui| {
        let label = match recording {
            Recording::Yes => "Press a combination…".to_owned(),
            Recording::No => shortcut.to_owned(),
        };
        if ui
            .add(egui::Button::selectable(
                *recording == Recording::Yes,
                label,
            ))
            .on_hover_text(
                "Frees the cursor when it gets trapped where you cannot click. Hold it \
                 for about a second.",
            )
            .clicked()
        {
            *recording = match recording {
                Recording::Yes => Recording::No,
                Recording::No => Recording::Yes,
            };
        }
        ui.label("Rescue shortcut");
    });

    if *recording == Recording::Yes {
        // Read before anything else can consume them. A modifier on its own is not a
        // shortcut and not an end to the recording either: the user is still holding
        // the combination down.
        let pressed = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*modifiers, *key)),
                _ => None,
            })
        });
        if let Some((modifiers, key)) = pressed {
            // Escape leaves the recorder without changing anything — the only way out
            // that does not commit, since every other key is a candidate.
            if key == egui::Key::Escape && modifiers.is_none() {
                *recording = Recording::No;
            } else {
                let spelled = spell(modifiers, key);
                if Shortcut::parse(&spelled).is_some() {
                    *recording = Recording::No;
                    recorded = Some(spelled);
                }
                // Anything else: keep waiting. A bare letter pressed on the way to
                // Ctrl+Alt+M must not be taken for the answer.
            }
        }
    }

    if let Some(why) = why_not(shortcut) {
        ui.colored_label(ui.visuals().error_fg_color, why);
    }
    if let Some(shortcut) = unavailable {
        // The hook's own words about the hook's own failure. Not folded into the
        // validation above: a well-formed shortcut that nothing armed is precisely the
        // case the user would otherwise never learn about.
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!(
                "{shortcut} could not be armed — something else owns it, or the desktop \
                 bound it to no key. The rescue is not available."
            ),
        );
    }
    recorded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_combination_is_spelled_the_way_the_grammar_reads_it() {
        let both = egui::Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
            ..Default::default()
        };
        let spelled = spell(both, egui::Key::M);
        assert_eq!(spelled, "Ctrl+Alt+Shift+M");
        assert_eq!(
            Shortcut::parse(&spelled),
            Shortcut::parse(lbm_ipc::shortcut::DEFAULT),
            "the recorder and the default must spell the same thing the same way"
        );
    }

    #[test]
    fn the_super_key_is_spelled_the_way_the_grammar_knows_it() {
        let win = egui::Modifiers {
            mac_cmd: true,
            ..Default::default()
        };
        let spelled = spell(win, egui::Key::F5);
        assert_eq!(spelled, "Win+F5");
        assert!(
            Shortcut::parse(&spelled).is_some(),
            "a combination this recorder can produce must be one the daemon registers"
        );
    }

    /// The refusal has to say which refusal it is: "invalid" is not actionable.
    #[test]
    fn each_way_of_being_wrong_says_which_one_it_is() {
        assert_eq!(why_not(lbm_ipc::shortcut::DEFAULT), None);
        assert!(why_not("").unwrap().contains("No rescue shortcut"));
        assert!(why_not("M").unwrap().contains("Ctrl, Alt, Shift or Win"));
        assert!(why_not("Ctrl+Alt").unwrap().contains("one key"));
        assert!(why_not("Ctrl+Nonsense").unwrap().contains("one key"));
    }

    /// Whatever the recorder can produce, the daemon can register. This is the property
    /// the shared grammar exists for, so it is asked directly.
    #[test]
    fn nothing_this_recorder_produces_is_a_shortcut_the_daemon_refuses() {
        let keys = [
            egui::Key::A,
            egui::Key::Z,
            egui::Key::Num0,
            egui::Key::Num9,
            egui::Key::F1,
            egui::Key::F12,
        ];
        let modifiers = [
            egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
            egui::Modifiers {
                alt: true,
                shift: true,
                ..Default::default()
            },
            egui::Modifiers {
                mac_cmd: true,
                ..Default::default()
            },
        ];
        for key in keys {
            for m in modifiers {
                let spelled = spell(m, key);
                assert!(
                    Shortcut::parse(&spelled).is_some(),
                    "{spelled} can be recorded and cannot be registered"
                );
                assert_eq!(why_not(&spelled), None, "{spelled} was called wrong");
            }
        }
    }
}
