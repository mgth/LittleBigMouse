use crate::geo::dotnet::max;

use super::size::{dotnet_equals, set_if_changed};

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
}

impl BorderSection {
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
    pub fn set_move_resistance(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.move_resistance, max(0.0, value))
    }
    pub fn set_move_block(&mut self, value: bool) -> bool {
        let changed = self.move_block != value;
        self.move_block = value;
        changed
    }
    pub fn set_drag(&mut self, value: f64) -> bool {
        set_if_changed(&mut self.drag, max(0.0, value))
    }
    pub fn set_drag_block(&mut self, value: bool) -> bool {
        let changed = self.drag_block != value;
        self.drag_block = value;
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

/// One edge of a monitor: the sections drawn on it, in insertion order.
/// Wherever no section covers the edge, it offers no resistance.
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
