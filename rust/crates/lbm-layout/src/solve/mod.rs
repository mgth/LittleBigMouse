//! The pure placement solvers of `LittleBigMouse.DisplayLayout/Monitors/Extensions`:
//! plain records in, translations or positions out, no reactive model.
//!
//! - [`compaction`]: `CompactionSolver`, the geometry behind `ForceCompact`.
//! - [`system_location`]: `SystemLocationSolver`, system pixels to millimetres.
//! - [`pixel_location`]: `PixelLocationSolver`, millimetres to system pixels.
//! - [`layout_geometry`] and [`edge_projection`]: `LayoutGeometry.cs`, the rules
//!   both placement directions share, and the `MonitorSnapshot` they read.
//! - [`distance`]: the `Rect`/`Thickness` distance helpers of `MonitorExtensions`.
//! - [`wayland_scale`]: the scale quantisation of
//!   `MonitorsLocationsToSystemExtensions.ComputePixelLocationsFromPhysical`.
//!
//! The bridges that snapshot the model and write the answers back
//! (`MonitorSnapshotExtensions`, `MonitorsLocationsFromSystemExtensions`,
//! `MonitorsLocationsToSystemExtensions`, `MonitorsLayout.ForceCompact`) belong
//! to the model port, which calls these.

pub mod compaction;
pub mod distance;
pub mod edge_projection;
pub mod layout_geometry;
pub mod pixel_location;
pub mod system_location;
pub mod wayland_scale;

pub use compaction::{CompactionMonitor, CompactionOptions};
pub use layout_geometry::{Axis, AxisProfile, EdgeContact, Interval, MonitorSnapshot};

use std::cmp::Ordering;
use std::ops::Index;

/// The solvers' `Dictionary<string, T>`, keyed by monitor id.
///
/// Enumerates in insertion order, which is what a .NET `Dictionary` does as long
/// as nothing is removed (the solvers never remove). Writing a key already there
/// replaces its value where it stands, like the C# indexer, so the order is that
/// of first insertion. Lookups are linear: a layout has a handful of monitors.
#[derive(Clone, Debug, PartialEq)]
pub struct IdMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> IdMap<V> {
    /// `new Dictionary<string, T>()`.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `Count == 0`, what xUnit's `Assert.Empty` checks.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `TryGetValue`.
    pub fn get(&self, id: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == id).map(|(_, v)| v)
    }

    /// `ContainsKey`.
    pub fn contains_key(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// The indexer's setter: replaces the value of an existing key in place,
    /// appends a new one.
    pub fn insert(&mut self, id: &str, value: V) {
        match self.entries.iter_mut().find(|(k, _)| k == id) {
            Some((_, v)) => *v = value,
            None => self.entries.push((id.to_owned(), value)),
        }
    }

    /// The entries in enumeration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// `Values`, in enumeration order.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }
}

impl<V> Default for IdMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// The indexer's getter. Panics on a missing id, where C# throws
/// `KeyNotFoundException`.
impl<V> Index<&str> for IdMap<V> {
    type Output = V;
    fn index(&self, id: &str) -> &V {
        self.get(id)
            .unwrap_or_else(|| panic!("no monitor {id:?} in the solver result"))
    }
}

impl<V> IntoIterator for IdMap<V> {
    type Item = (String, V);
    type IntoIter = std::vec::IntoIter<(String, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

/// `StringComparer.Ordinal`, as byte order. The two agree everywhere except
/// between a character in U+E000..U+FFFF and one outside the BMP, which UTF-16
/// code units order the other way round; monitor ids are ASCII.
fn ordinal(a: &str, b: &str) -> Ordering {
    a.as_bytes().cmp(b.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_map_keeps_first_insertion_order_and_overwrites_in_place() {
        let mut m = IdMap::new();
        m.insert("B", 1);
        m.insert("A", 2);
        m.insert("B", 3);
        assert_eq!(m.len(), 2);
        assert_eq!(m["B"], 3);
        assert_eq!(m.get("C"), None);
        let keys: Vec<&str> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["B", "A"]);
    }

    #[test]
    fn ordinal_is_byte_order_not_culture() {
        // Culture-aware comparers put "a" before "B"; ordinal puts uppercase first.
        assert_eq!(ordinal("B", "a"), Ordering::Less);
        assert_eq!(ordinal("M10", "M2"), Ordering::Less);
    }
}
