//! Validated touchscreen policy. Parsed on layout load, never in a mouse callback.
use crate::zones::ZonesLayout;

#[derive(Clone, Debug)]
pub struct TouchPolicy {
    pub all_displays: bool,
    pub bounds: Vec<[f64; 4]>,
    pub modifier: Modifier,
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
    fn by_default_every_display_counts_and_no_key_overrides() {
        let p = TouchPolicy::from_layout(&ZonesLayout::default());
        assert!(p.includes(-10000, 50));
        assert_eq!(p.modifier, Modifier::None);
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
}
