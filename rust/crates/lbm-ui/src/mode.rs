//! What every monitor frame shows inside it — C#'s `MainViewModel.ContentViewMode`.
//!
//! One mode for the whole map, not one per screen: `MonitorFrameView.axaml:261` binds
//! every frame to the same `ContentViewMode`, so choosing "Location" turns all of them at
//! once.
//!
//! The six monitor modes retain the original plugin order. About is a separate
//! application window, available at the right end of the toolbar.
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
    Info,
    Location,
    Resistance,
    Size,
    Vcp,
    Wallpaper,
}

impl Mode {
    /// The six that have a button, in the order the shipped app shows them.
    pub const ALL: [Mode; 6] = [
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
            Mode::Info => "info",
            Mode::Location => "location",
            Mode::Resistance => "resistance",
            Mode::Size => "size",
            Mode::Vcp => "vcp",
            Mode::Wallpaper => "wallpaper",
        }
    }

    /// The icon button's accessible name and tooltip title.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Default => "Screens",
            Mode::Info => "Info",
            Mode::Location => "Location",
            Mode::Resistance => "Border resistance",
            Mode::Size => "Size",
            Mode::Vcp => "Display Lab",
            Mode::Wallpaper => "Wallpaper",
        }
    }

    /// What it puts in the frames, for the tooltip.
    pub fn about(self) -> &'static str {
        match self {
            Mode::Default => "The screens as they look: the name and the maker's logo.",
            Mode::Info => "Detected display details and the current layout values.",
            Mode::Location => "Where each screen is, how big it is, and at what pitch.",
            Mode::Resistance => {
                "The parts of each border the mouse may cross, and the parts it may not."
            }
            Mode::Size => "The physical size of a screen and of its bezel.",
            Mode::Vcp => "Monitor controls and network remotes.",
            Mode::Wallpaper => "The desktop picture, per screen or spanned across them.",
        }
    }
}

/// Which modes this frontend can actually show, and why not when it cannot.
///
/// A mode is offered when there is a view behind it. The C# gates were:
///
/// * **Vcp** follows `ILayoutOptions.VcpControl` — a setting, off by default, which the
///   user turns on when they want the sliders (`VcpPlugin.cs:46`);
/// * **Wallpaper** is a hard gate on `IWallpaperService.IsSupported` — KDE on Linux,
///   Windows elsewhere; on any other desktop the button is not created at all
///   (`WallpaperPlugin.cs:20`).
///
/// The egui wallpaper editor also supports local previews on unsupported desktops.
/// Its availability follows the loaded agent/layout; native support gates Apply only.
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
        Mode::Default | Mode::Location | Mode::Size | Mode::Info => None,
        Mode::Vcp if !offered.vcp_enabled => {
            Some("Display Lab is off in the settings — turn it on to see this")
        }
        Mode::Wallpaper if !offered.wallpaper_supported => {
            Some("connect to the agent and load the layout to open the wallpaper editor")
        }
        // Ported, but not yet. Named one by one rather than through a catch-all, so
        // that porting one is a line to delete here and the compiler finds the rest.
        Mode::Resistance => None,
        Mode::Vcp => None,
        Mode::Wallpaper => None,
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
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        buttons(ui, current, offered)
    })
    .inner
}

/// Settings and monitor modes in the main toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarAction {
    Options,
    About,
    Mode(Mode),
}

/// Options, six mode toggles and a right-aligned About button; the view switch lives beside the monitor combo.
pub fn toolbar(
    ui: &mut egui::Ui,
    current: Mode,
    offered: &Offered,
    options: bool,
) -> Option<ToolbarAction> {
    use crate::toolbar::{self, Icon};
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let left_width = (ui.available_width() - 44.0).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(left_width, 40.0),
            egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
            |ui| {
                ui.set_min_width(left_width);
                if toolbar::button(ui, Icon::Options, "Settings", options, true)
                    .on_hover_text("Options")
                    .clicked()
                {
                    chosen = Some(ToolbarAction::Options);
                }
                toolbar::separator(ui);
                if let Some(mode) = buttons(ui, current, offered) {
                    chosen = Some(ToolbarAction::Mode(mode));
                }
            },
        );
        if toolbar::button(ui, Icon::About, "About", false, true)
            .on_hover_text("About Little Big Mouse")
            .clicked()
        {
            chosen = Some(ToolbarAction::About);
        }
    });
    chosen
}

fn icon(mode: Mode) -> crate::toolbar::Icon {
    use crate::toolbar::Icon;
    match mode {
        Mode::Default => Icon::About,
        Mode::Info => Icon::Info,
        Mode::Location => Icon::Location,
        Mode::Resistance => Icon::Resistance,
        Mode::Size => Icon::Size,
        Mode::Vcp => Icon::Vcp,
        Mode::Wallpaper => Icon::Wallpaper,
    }
}

/// The properties panel opens with the same picture as the mode's toolbar button.
pub fn heading(ui: &mut egui::Ui, mode: Mode) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
        let tint = if ui.is_enabled() {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_white_alpha(64)
        };
        crate::toolbar::paint(ui, icon(mode), rect, ui.visuals().text_color(), tint);
        ui.heading(mode.label());
    });
}

fn buttons(ui: &mut egui::Ui, current: Mode, offered: &Offered) -> Option<Mode> {
    let mut chosen = why_not(current, offered).map(|_| Mode::Default);
    for mode in Mode::ALL {
        let unavailable = why_not(mode, offered);
        let on = current == mode;
        let button =
            crate::toolbar::button(ui, icon(mode), mode.label(), on, unavailable.is_none());
        let tooltip = format!("{}\n{}", mode.label(), unavailable.unwrap_or(mode.about()));
        let button = button
            .on_hover_text(&tooltip)
            .on_disabled_hover_text(&tooltip);
        if button.clicked() {
            chosen = Some(if on { Mode::Default } else { mode });
        }
    }
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
        assert_eq!(Mode::ALL.len(), 6, "About belongs to its own window");
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
        assert_eq!(why_not(Mode::Vcp, &on), None);
    }
}
