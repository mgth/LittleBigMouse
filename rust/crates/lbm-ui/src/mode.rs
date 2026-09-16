//! What every monitor frame shows inside it — C#'s `MainViewModel.ContentViewMode`.
//!
//! One mode for the whole map, not one per screen: `MonitorFrameView.axaml:261` binds
//! every frame to the same `ContentViewMode`, so choosing "Location" turns all of them at
//! once.
//!
//! # Why this is a closed set, and why that is not a product decision
//!
//! In C# a mode is contributed by a **plugin**: `ViewMode` (`IView.cs:28`) is an empty
//! abstract class, each mode an empty subclass, and the type itself is the identity.
//! Plugins register one with `AddViewModeButton<T>(id, icon, tooltip)`, discovered by
//! scanning assemblies for `Bootloader` subclasses (`Program.cs:249-264`). Replacing that
//! with an `enum` looks like it decides which plugins survive the port.
//!
//! It does not, and the survey is what settles it: **there are exactly seven
//! registrations, and all seven are in this repository** — About, Info, Location, Border
//! resistance, Size, Vcp control, Wallpaper. Every one of them ships in the app's own
//! solution, is compiled into it, and is loaded by scanning the app's own DLLs. There is
//! no third-party plugin, no published extension point, nothing outside that the
//! mechanism serves. It is an internal indirection over a list this repository owns.
//!
//! So the enum is that list. What is genuinely a product decision — which of these
//! features get ported, and when — is a different question, and it is answered here by
//! [`Offered`] rather than by deleting a variant: a mode the port has not reached yet is
//! **shown and disabled, saying so**. A bar of what is missing is worth more than a bar
//! that pretends the missing things never existed.
//!
//! Two C# behaviours worth keeping, both easy to miss because neither is declared:
//!
//! * pressing the mode that is already on goes back to the default
//!   (`MainPluginsViewModelExtension.cs:22-26`) — the bar is a set of toggles, not a
//!   radio group, and this is what makes it behave like one;
//! * a mode that becomes unavailable while it is on falls back to the default
//!   (`:49-50`). VCP is the case: its button follows `ILayoutOptions.VcpControl`, which
//!   the user can turn off from the settings while looking at the VCP view.
//!
//! One C# behaviour **not** kept: the bar's order. It is `SortExpressionComparer` on the
//! plugin `Id` (`MainViewModel.cs:63`), so the order is alphabetical by an internal
//! string and a rename silently reorders the bar. [`Mode::ALL`] keeps the order the
//! shipped app happens to show, so a user's habits survive — but as a decision rather
//! than as a side effect.

/// What the frames show inside them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The name and the logo: the frame as a picture of a screen. No plugin owns it and
    /// no button selects it — it is what the bar goes back to.
    #[default]
    Default,
    About,
    Info,
    Location,
    Resistance,
    Size,
    Vcp,
    Wallpaper,
}

impl Mode {
    /// The seven that have a button, in the order the shipped app shows them.
    pub const ALL: [Mode; 7] = [
        Mode::About,
        Mode::Info,
        Mode::Location,
        Mode::Resistance,
        Mode::Size,
        Mode::Vcp,
        Mode::Wallpaper,
    ];

    /// The plugin id — the C#'s own string, kept because it is what the icons and the
    /// settings are named after.
    pub fn id(self) -> &'static str {
        match self {
            Mode::Default => "default",
            Mode::About => "about",
            Mode::Info => "info",
            Mode::Location => "location",
            Mode::Resistance => "resistance",
            Mode::Size => "size",
            Mode::Vcp => "vcp",
            Mode::Wallpaper => "wallpaper",
        }
    }

    /// What the button says. In C# the buttons are icon-only and this is the tooltip —
    /// the only human-readable name a mode has. Here it is on the button itself, until
    /// the icons are ported: a row of unlabelled glyphs is worse than a row of words.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Default => "Screens",
            Mode::About => "About",
            Mode::Info => "Info",
            Mode::Location => "Location",
            Mode::Resistance => "Border resistance",
            Mode::Size => "Size",
            Mode::Vcp => "Vcp control",
            Mode::Wallpaper => "Wallpaper",
        }
    }

    /// What it puts in the frames, for the tooltip.
    pub fn about(self) -> &'static str {
        match self {
            Mode::Default => "The screens as they look: the name and the maker's logo.",
            Mode::About => "This app's version, and what identifies each screen.",
            Mode::Info => "Everything the system reports about a screen, raw.",
            Mode::Location => "Where each screen is, how big it is, and at what pitch.",
            Mode::Resistance => {
                "The parts of each border the mouse may cross, and the parts it may not."
            }
            Mode::Size => "The physical size of a screen and of its bezel.",
            Mode::Vcp => "Brightness, contrast and colour, over DDC/CI.",
            Mode::Wallpaper => "The desktop picture, per screen or spanned across them.",
        }
    }
}

/// Which modes this frontend can actually show, and why not when it cannot.
///
/// A mode is offered when there is a view behind it. The two the C# itself makes
/// conditional are here for the same reasons it has:
///
/// * **Vcp** follows `ILayoutOptions.VcpControl` — a setting, off by default, which the
///   user turns on when they want the sliders (`VcpPlugin.cs:46`);
/// * **Wallpaper** is a hard gate on `IWallpaperService.IsSupported` — KDE on Linux,
///   Windows elsewhere; on any other desktop the button is not created at all
///   (`WallpaperPlugin.cs:20`).
///
/// The rest are "not ported yet", which is a truth about this frontend and not about the
/// user's machine, so the two are worded differently: one says what the user can do about
/// it, the other says there is nothing to do yet.
#[derive(Clone, Copy, Debug, Default)]
pub struct Offered {
    /// The user turned VCP control on in the settings.
    pub vcp_enabled: bool,
    /// This desktop has a wallpaper service the agent can drive.
    pub wallpaper_supported: bool,
}

/// Why a mode cannot be chosen, or `None` when it can.
pub fn why_not(mode: Mode, offered: &Offered) -> Option<&'static str> {
    match mode {
        // What the window can draw today.
        Mode::Default | Mode::About | Mode::Location => None,
        Mode::Vcp if !offered.vcp_enabled => {
            Some("VCP control is off in the settings — turn it on to see this")
        }
        Mode::Wallpaper if !offered.wallpaper_supported => {
            Some("this desktop has no wallpaper service the agent can drive")
        }
        // Ported, but not yet. Named one by one rather than through a catch-all, so
        // that porting one is a line to delete here and the compiler finds the rest.
        Mode::Info => Some("not ported yet: the raw display report has no Rust reader"),
        Mode::Resistance => Some("not ported yet: the border section editor"),
        Mode::Size => Some("not ported yet: editing a screen's physical size"),
        Mode::Vcp => Some("not ported yet: DDC/CI and the calibration plugin"),
        Mode::Wallpaper => Some("not ported yet: the per-screen picture picker"),
    }
}

/// The mode bar. Gives back the mode to switch to, when the user chose one.
///
/// Pressing the mode already on goes back to [`Mode::Default`], as the C# does. A mode
/// that has become unavailable is dropped for the default too, without a press — the
/// caller gets that back as a change like any other, so the fallback happens once and in
/// one place.
pub fn bar(ui: &mut egui::Ui, current: Mode, offered: &Offered) -> Option<Mode> {
    // Before anything is drawn: a mode that stopped being available cannot stay on.
    if why_not(current, offered).is_some() {
        return Some(Mode::Default);
    }
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for mode in Mode::ALL {
            let unavailable = why_not(mode, offered);
            let on = current == mode;
            let button = ui.add_enabled(
                unavailable.is_none(),
                egui::Button::selectable(on, mode.label()),
            );
            let button = match unavailable {
                Some(why) => button.on_disabled_hover_text(why),
                None => button.on_hover_text(mode.about()),
            };
            if button.clicked() {
                // The toggle that behaves like a radio group: pressing the one that is
                // already on turns it off, which means back to the default.
                chosen = Some(if on { Mode::Default } else { mode });
            }
        }
    });
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_has_its_own_id_and_its_own_words() {
        let mut ids: Vec<&str> = Mode::ALL.iter().map(|m| m.id()).collect();
        ids.push(Mode::Default.id());
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "two modes share an id: {ids:?}");

        for mode in Mode::ALL {
            assert!(!mode.label().is_empty(), "{mode:?} has no label");
            assert!(
                !mode.about().is_empty(),
                "{mode:?} says nothing about itself"
            );
        }
    }

    /// The default is not in the bar: no plugin contributes it, and a button for "no
    /// mode" beside the modes would read as an eighth mode.
    #[test]
    fn the_default_has_no_button() {
        assert!(!Mode::ALL.contains(&Mode::Default));
        assert_eq!(Mode::ALL.len(), 7, "the seven the C# registers, no more");
    }

    #[test]
    fn what_is_offered_answers_for_itself() {
        let nothing = Offered::default();
        assert_eq!(why_not(Mode::Default, &nothing), None);
        assert_eq!(why_not(Mode::Location, &nothing), None);

        // The two the C# gates, gated for the C#'s own reasons and worded as a thing the
        // user can act on.
        assert!(why_not(Mode::Vcp, &nothing).unwrap().contains("settings"));
        let on = Offered {
            vcp_enabled: true,
            ..nothing
        };
        assert!(
            why_not(Mode::Vcp, &on).unwrap().contains("not ported"),
            "with the setting on, what is left to say is that the port has not reached it"
        );
    }
}
