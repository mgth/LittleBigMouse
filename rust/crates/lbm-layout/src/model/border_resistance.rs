use crate::geo::dotnet::max;

use super::size::{dotnet_equals, set_if_changed};

/// How a section handles one kind of crossing, independently of the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResistanceMode {
    Block,
    Resist,
    Free,
    Custom,
}

impl ResistanceMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Block => "Block",
            Self::Resist => "Resist",
            Self::Free => "Free",
            Self::Custom => "Custom",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Block" => Some(Self::Block),
            "Resist" => Some(Self::Resist),
            "Free" => Some(Self::Free),
            "Custom" => Some(Self::Custom),
            _ => None,
        }
    }

    fn value(self, custom: f64, default: f64) -> f64 {
        match self {
            Self::Resist => default,
            Self::Free => 0.,
            Self::Block | Self::Custom => custom,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResistanceDefaults {
    pub movement: f64,
    pub drag: f64,
    pub move_without_section: bool,
    pub drag_without_section: bool,
}

impl Default for ResistanceDefaults {
    fn default() -> Self {
        Self {
            movement: 5.,
            drag: 15.,
            move_without_section: false,
            drag_without_section: false,
        }
    }
}

impl ResistanceDefaults {
    /// Only uncovered intervals inherit these values; even an explicit Free section wins.
    pub fn without_section(self) -> ResistanceValues {
        ResistanceValues {
            move_resistance: if self.move_without_section {
                self.movement
            } else {
                0.
            },
            drag: if self.drag_without_section {
                self.drag
            } else {
                0.
            },
            ..Default::default()
        }
    }
}

/// One stretch of a monitor edge with its own resistances: C#'s `BorderSection`.
///
/// `from` and `to` are millimetres from the edge's starting corner (the top for
/// the left and right edges, the left for the top and bottom ones). The numeric
/// setters floor at zero, `Math.Max(0, value)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BorderSection {
    from: f64,
    to: f64,
    move_resistance: f64,
    move_block: bool,
    drag: f64,
    drag_block: bool,
    move_mode: Option<ResistanceMode>,
    drag_mode: Option<ResistanceMode>,
}

impl BorderSection {
    pub fn move_mode(&self) -> ResistanceMode {
        self.move_mode
            .unwrap_or_else(|| legacy_mode(self.move_resistance, self.move_block))
    }
    pub fn drag_mode(&self) -> ResistanceMode {
        self.drag_mode
            .unwrap_or_else(|| legacy_mode(self.drag, self.drag_block))
    }
    pub fn set_move_mode(&mut self, mode: ResistanceMode) {
        self.move_mode = Some(mode);
        self.move_block = mode == ResistanceMode::Block;
    }
    pub fn set_drag_mode(&mut self, mode: ResistanceMode) {
        self.drag_mode = Some(mode);
        self.drag_block = mode == ResistanceMode::Block;
    }
    /// Resolve defaults at consumption time; preserve the editable custom values.
    pub fn resolved(&self, defaults: ResistanceDefaults) -> Self {
        Self::new(
            self.from,
            self.to,
            self.move_mode()
                .value(self.move_resistance, defaults.movement),
            self.move_block,
            self.drag_mode().value(self.drag, defaults.drag),
            self.drag_block,
        )
    }
    pub fn stored_move_mode(&self) -> Option<ResistanceMode> {
        self.move_mode
    }
    pub fn stored_drag_mode(&self) -> Option<ResistanceMode> {
        self.drag_mode
    }
    pub fn from(&self) -> f64 {
        self.from
    }
    pub fn to(&self) -> f64 {
        self.to
    }
    /// Resistance in mm opposing a plain cursor move (`Move` in C#).
    pub fn move_resistance(&self) -> f64 {
        self.move_resistance
    }
    pub fn move_block(&self) -> bool {
        self.move_block
    }
    /// Resistance in mm opposing a move made with a button held.
    pub fn drag(&self) -> f64 {
        self.drag
    }
    pub fn drag_block(&self) -> bool {
        self.drag_block
    }

    pub fn set_from(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.from, max(0.0, value))
    }
    pub fn set_to(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.to, max(0.0, value))
    }
    /// A new value is only used in `Custom`, so it takes an inheriting or free channel
    /// there. Editors write every field back: an unchanged one leaves the mode alone.
    pub fn set_move_resistance(&mut self, value: f64) -> bool {
        let changed = set_if_changed(&mut self.move_resistance, max(0.0, value));
        if changed {
            self.move_mode = mode_after_new_value(self.move_mode);
        }
        changed
    }
    /// A changed switch replaces the stored mode, which the older fields then imply.
    pub fn set_move_block(&mut self, value: bool) -> bool {
        let changed = self.move_block != value;
        if changed {
            self.move_mode = None;
            self.move_block = value;
        }
        changed
    }
    pub fn set_drag(&mut self, value: f64) -> bool {
        let changed = set_if_changed(&mut self.drag, max(0.0, value));
        if changed {
            self.drag_mode = mode_after_new_value(self.drag_mode);
        }
        changed
    }
    pub fn set_drag_block(&mut self, value: bool) -> bool {
        let changed = self.drag_block != value;
        if changed {
            self.drag_mode = None;
            self.drag_block = value;
        }
        changed
    }

    /// Builds a section through the setters, flooring like C# does.
    pub fn new(
        from: f64,
        to: f64,
        move_resistance: f64,
        move_block: bool,
        drag: f64,
        drag_block: bool,
    ) -> Self {
        let mut s = Self::default();
        s.set_from(from);
        s.set_to(to);
        s.set_move_resistance(move_resistance);
        s.set_move_block(move_block);
        s.set_drag(drag);
        s.set_drag_block(drag_block);
        s
    }
}

fn mode_after_new_value(mode: Option<ResistanceMode>) -> Option<ResistanceMode> {
    match mode {
        Some(ResistanceMode::Resist | ResistanceMode::Free) => Some(ResistanceMode::Custom),
        other => other,
    }
}

fn legacy_mode(value: f64, blocked: bool) -> ResistanceMode {
    if blocked {
        ResistanceMode::Block
    } else if value == 0. {
        ResistanceMode::Free
    } else {
        ResistanceMode::Custom
    }
}

/// One edge of a monitor: the sections drawn on it, in insertion order.
/// Uncovered intervals use the layout's optional defaults for unsectioned crossings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderSide {
    pub sections: Vec<BorderSection>,
}

/// The four edges of a monitor: C#'s `BorderResistance`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderResistance {
    pub left: BorderSide,
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
}

impl BorderResistance {
    pub fn resolved(&self, defaults: ResistanceDefaults) -> Self {
        let side = |s: &BorderSide| BorderSide {
            sections: s.sections.iter().map(|s| s.resolved(defaults)).collect(),
        };
        Self {
            left: side(&self.left),
            top: side(&self.top),
            right: side(&self.right),
            bottom: side(&self.bottom),
        }
    }
}

/// The resistances governing one stretch of edge once the link compiler has
/// resolved it: a section's, or none (`BorderResistanceValues` in C#).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResistanceValues {
    pub move_resistance: f64,
    pub move_block: bool,
    pub drag: f64,
    pub drag_block: bool,
}

impl ResistanceValues {
    /// `ZoneLink.HasSameResistanceAs`: `double.Equals` on the numbers (NaN
    /// equals NaN), `==` on the flags.
    pub fn same_as(&self, other: &ResistanceValues) -> bool {
        dotnet_equals(self.move_resistance, other.move_resistance)
            && self.move_block == other.move_block
            && dotnet_equals(self.drag, other.drag)
            && self.drag_block == other.drag_block
    }
}
