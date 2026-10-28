//! Validated touchscreen policy. Parsed on layout load, never in a mouse callback.
use crate::zones::ZonesLayout;

#[derive(Clone, Debug)]
pub struct TouchPolicy {
    pub include_stylus: bool,
    pub delay_ms: u64,
    pub on_mouse_move: bool,
    pub all_displays: bool,
    pub bounds: Vec<[f64; 4]>,
    pub modifier: Modifier,
    keep_apps: Vec<String>,
    restore_apps: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    None,
    Ctrl,
    Alt,
    Shift,
    Win,
}

/// Keep an override or unselected display bypassed for the entire gesture,
/// including trailing promoted moves after finger-up.
#[derive(Clone, Copy, Default)]
pub struct TouchGesture {
    bypass: bool,
    pub overridden: bool,
    pub contact: bool,
}

impl TouchGesture {
    pub const fn new() -> Self {
        Self {
            bypass: false,
            overridden: false,
            contact: false,
        }
    }
    pub fn accepts(&mut self, selected: bool, overridden: bool, down: bool, up: bool) -> bool {
        if !self.contact && (down || !self.bypass) {
            self.bypass = !selected || overridden;
            self.overridden = selected && overridden;
        }
        if down {
            self.contact = true;
        }
        if up {
            self.contact = false;
        }
        !self.bypass
    }
}

fn apps(value: &str) -> Vec<String> {
    value
        .split([';', '\r', '\n'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn display_bounds(rect: &str) -> Option<[f64; 4]> {
    let mut fields = rect.split(',');
    let mut values = [0.0_f64; 4];
    for value in &mut values {
        *value = fields.next()?.parse().ok()?;
        if !value.is_finite() {
            return None;
        }
    }
    (fields.next().is_none() && values[2] > 0.0 && values[3] > 0.0).then_some(values)
}

impl TouchPolicy {
    pub fn from_layout(layout: &ZonesLayout) -> Self {
        Self {
            include_stylus: layout.stylus_mouse_independent,
            delay_ms: layout.focus_restore_delay.clamp(0, 5000) as u64,
            on_mouse_move: layout.focus_restore_on_mouse_move,
            all_displays: layout.touch_all_displays,
            bounds: layout
                .touch_display_bounds
                .split(';')
                .filter_map(display_bounds)
                .collect(),
            modifier: match layout.touch_override_modifier.to_ascii_lowercase().as_str() {
                "ctrl" => Modifier::Ctrl,
                "alt" => Modifier::Alt,
                "shift" => Modifier::Shift,
                "win" => Modifier::Win,
                _ => Modifier::None,
            },
            keep_apps: apps(&layout.focus_keep_apps),
            restore_apps: apps(&layout.focus_restore_apps),
        }
    }

    pub fn includes(&self, x: i32, y: i32) -> bool {
        self.all_displays
            || self.bounds.iter().any(|b| {
                x as f64 >= b[0]
                    && (x as f64) < b[0] + b[2]
                    && y as f64 >= b[1]
                    && (y as f64) < b[1] + b[3]
            })
    }

    /// Exact executable basenames, case-insensitive. Keep wins over restore.
    /// An unresolved process is safe only when there are no app rules to enforce.
    pub fn restores_app(&self, path: Option<&str>) -> bool {
        if self.keep_apps.is_empty() && self.restore_apps.is_empty() {
            return true;
        }
        let Some(path) = path else {
            return false;
        };
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
        !self
            .keep_apps
            .iter()
            .any(|app| app.eq_ignore_ascii_case(name))
            && (self.restore_apps.is_empty()
                || self
                    .restore_apps
                    .iter()
                    .any(|app| app.eq_ignore_ascii_case(name)))
    }
}

impl Default for TouchPolicy {
    fn default() -> Self {
        Self::from_layout(&ZonesLayout::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_bounds_never_become_a_selected_display() {
        for text in [
            "",
            "1,2,3",
            "1,2,3,4,5",
            "0,0,NaN,1",
            "0,0,1,inf",
            "0,0,0,1",
            "0,0,1,-1",
        ] {
            assert_eq!(display_bounds(text), None, "{text}");
        }
        assert_eq!(
            display_bounds("-1920,-100,1920,1080"),
            Some([-1920.0, -100.0, 1920.0, 1080.0])
        );
    }

    #[test]
    fn override_survives_modifier_release_and_trailing_moves() {
        let mut gesture = TouchGesture::new();
        assert!(!gesture.accepts(true, true, true, false));
        assert!(!gesture.accepts(true, false, false, false));
        assert!(!gesture.accepts(true, false, false, true));
        assert!(!gesture.accepts(true, false, false, false));
        assert!(gesture.overridden);
        assert!(gesture.accepts(true, false, true, false));
        assert!(!gesture.overridden);
        // Moving across a display boundary mid-drag does not change its policy.
        assert!(gesture.accepts(false, false, false, false));
        assert!(gesture.contact);
    }

    #[test]
    fn defaults_and_delay_limits() {
        let mut layout = ZonesLayout::default();
        let p = TouchPolicy::from_layout(&layout);
        assert_eq!(p.delay_ms, 120);
        assert!(!p.on_mouse_move);
        assert!(p.includes(-10000, 50));
        assert_eq!(p.modifier, Modifier::None);
        layout.focus_restore_delay = -1;
        assert_eq!(TouchPolicy::from_layout(&layout).delay_ms, 0);
        layout.focus_restore_delay = 99999;
        assert_eq!(TouchPolicy::from_layout(&layout).delay_ms, 5000);
    }

    #[test]
    fn selected_displays_and_empty_selection() {
        let mut l = ZonesLayout::default();
        l.touch_all_displays = false;
        assert!(!TouchPolicy::from_layout(&l).includes(0, 0));
        l.touch_display_bounds = "-1920,0,1920,1080;bad;0,0,NaN,100".into();
        let p = TouchPolicy::from_layout(&l);
        assert!(p.includes(-1920, 0));
        assert!(p.includes(-1, 1079));
        assert!(!p.includes(0, 0));
        assert!(!p.includes(-1, 1080));
    }

    #[test]
    fn app_rules_are_exact_and_keep_takes_precedence() {
        let mut l = ZonesLayout::default();
        l.focus_keep_apps = " chrome.exe ; Editor.exe".into();
        l.focus_restore_apps = "control.exe;editor.exe".into();
        let p = TouchPolicy::from_layout(&l);
        assert!(p.restores_app(Some(r"C:\Tools\CONTROL.EXE")));
        for path in [
            None,
            Some("editor.exe"),
            Some("chrome.exe"),
            Some("mycontrol.exe"),
            Some("other.exe"),
        ] {
            assert!(!p.restores_app(path));
        }
        l.focus_restore_apps.clear();
        assert!(TouchPolicy::from_layout(&l).restores_app(Some("other.exe")));
    }
}
