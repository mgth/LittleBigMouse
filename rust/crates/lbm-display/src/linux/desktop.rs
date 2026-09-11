//! Which desktop shell runs — port of `LinuxDesktopEnvironment.cs`.

/// C# `LinuxDesktopEnvironment`: the freedesktop variables that tell which desktop
/// shell is running, reduced to the one signal the platform code needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopEnvironment {
    xdg_current_desktop: Option<String>,
    desktop_session: Option<String>,
}

impl DesktopEnvironment {
    /// From raw values of `XDG_CURRENT_DESKTOP` and `DESKTOP_SESSION`.
    pub fn new(xdg_current_desktop: Option<&str>, desktop_session: Option<&str>) -> Self {
        DesktopEnvironment {
            xdg_current_desktop: xdg_current_desktop.map(str::to_owned),
            desktop_session: desktop_session.map(str::to_owned),
        }
    }

    /// C# `LinuxDesktopEnvironment.Current`: the live process environment.
    pub fn current() -> Self {
        let var = |name| std::env::var(name).ok();
        DesktopEnvironment {
            xdg_current_desktop: var("XDG_CURRENT_DESKTOP"),
            desktop_session: var("DESKTOP_SESSION"),
        }
    }

    /// C# `IsKde`: true when the session is KDE Plasma. `XDG_CURRENT_DESKTOP` is a
    /// colon-separated priority list; any element naming KDE or Plasma
    /// (case-insensitively) counts. When it is unset or empty, `DESKTOP_SESSION` (the
    /// session file name, e.g. `plasmawayland`) decides.
    pub fn is_kde(&self) -> bool {
        match self.xdg_current_desktop.as_deref() {
            None | Some("") => names_kde(self.desktop_session.as_deref()),
            value => names_kde(value),
        }
    }
}

fn names_kde(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        v.split(':').filter(|t| !t.is_empty()).any(|token| {
            let token = token.to_lowercase();
            token.contains("kde") || token.contains("plasma")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::DesktopEnvironment;

    /// C# `LinuxDesktopEnvironmentTests`, the four of them.
    #[test]
    fn kde_is_recognised_as_in_csharp() {
        let is_kde = |xdg: Option<&str>, session: Option<&str>| {
            DesktopEnvironment::new(xdg, session).is_kde()
        };
        // XDG_CURRENT_DESKTOP, a priority list, any element, either spelling, any case.
        assert!(is_kde(Some("KDE"), None));
        assert!(is_kde(Some("ubuntu:kde"), None));
        assert!(is_kde(Some("Plasma"), None));
        assert!(!is_kde(Some("ubuntu:GNOME"), Some("plasma")));
        // Unset or empty: the session file name decides.
        assert!(is_kde(None, Some("plasmawayland")));
        assert!(is_kde(Some(""), Some("plasma")));
        assert!(!is_kde(None, Some("gnome")));
        assert!(!is_kde(None, None));
        assert!(!is_kde(Some("::"), None));
    }
}
