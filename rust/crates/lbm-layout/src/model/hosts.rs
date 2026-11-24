//! What the layout knows of other machines in the multi-PC mode (docs/multi-pc.md,
//! "Model"): which machines its sources belong to, which source a panel two machines
//! know is showing, and where each remote desktop sits in the engine's pixel space.
//!
//! A layout without remote sources goes through none of this: every function here is
//! a no-op on it, or answers as if the machine were alone.

use crate::geo::{Point, Rect, Vector};

use super::{Layout, Monitor, PhysicalSource};

/// Pixels left empty between two desktops in the engine's pixel space. Zones are linked
/// in millimetres, so the gap carries no meaning for travel; it is there so that no
/// pixel of one machine's desktop touches another's, which keeps "which machine is
/// this point on" a question with one answer even at an edge.
pub const HOST_GAP_PX: i32 = 1000;

/// Where a machine's desktop sits in the engine's pixel space: its own desktop
/// coordinates plus this offset. This machine's offset is always zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOffset {
    /// The machine id.
    pub host: String,
    pub x: i32,
    pub y: i32,
}

impl HostOffset {
    pub fn vector(&self) -> Vector {
        Vector::new(f64::from(self.x), f64::from(self.y))
    }
}

impl Layout {
    /// The source a monitor is showing, when it has one the layout holds.
    pub fn active_source_of(&self, monitor: &Monitor) -> Option<&PhysicalSource> {
        self.source(monitor.active_source.as_deref()?)
    }

    /// Whether the monitor is showing one of this machine's sources: only those take
    /// part in what is read from, or written to, the local desktop.
    pub fn shows_local_source(&self, monitor: &Monitor) -> bool {
        self.active_source_of(monitor)
            .is_some_and(|s| s.source.is_local())
    }

    /// The remote machines the layout holds sources of, by machine id, sorted (byte
    /// order: ids are lowercase hex, and the order must not depend on a locale).
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self
            .sources()
            .filter_map(|s| s.source.host.clone())
            .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }

    /// Sets the VCP 0x60 value that selects a source on its monitor, learnt by reading
    /// it while that source is shown (docs/multi-pc.md, "Input switching"). A new value
    /// is something to save.
    pub fn set_source_input(&mut self, source_id: &str, input_source: Option<u16>) {
        if let Some(i) = self.source_index(source_id) {
            if self.sources[i].source.input_source != input_source {
                self.sources[i].source.input_source = input_source;
                self.saved = false;
            }
        }
    }

    /// The rule of a panel both machines know (docs/multi-pc.md): attached on one
    /// machine only, that machine's source is shown; attached on both, the current
    /// choice stands — the stored `ActiveSource` once a store has been applied — and
    /// without one, this machine's source. A panel nobody shows keeps its last choice.
    ///
    /// Only monitors whose sources belong to more than one machine are touched: a local
    /// monitor with two sources of its own (two inputs Windows could not tell apart)
    /// keeps what the store says, as it always has.
    pub fn resolve_active_sources(&mut self) {
        let mut choices: Vec<(String, String)> = Vec::new();
        for monitor in &self.monitors {
            let sources: Vec<&PhysicalSource> = monitor
                .sources
                .iter()
                .filter_map(|id| self.source(id))
                .collect();
            let mut hosts: Vec<Option<&str>> =
                sources.iter().map(|s| s.source.host.as_deref()).collect();
            hosts.sort();
            hosts.dedup();
            if hosts.len() < 2 {
                continue;
            }
            let attached: Vec<&&PhysicalSource> = sources
                .iter()
                .filter(|s| s.source.attached_to_desktop)
                .collect();
            let current = monitor.active_source.as_deref();
            if attached
                .iter()
                .any(|s| Some(s.source.id.as_str()) == current)
            {
                continue;
            }
            let Some(choice) = attached
                .iter()
                .find(|s| s.source.is_local())
                .or_else(|| attached.first())
            else {
                continue;
            };
            choices.push((monitor.id.clone(), choice.source.id.clone()));
        }
        for (monitor, source) in choices {
            self.set_active_source(&monitor, Some(source));
        }
    }

    /// Where each remote desktop goes in the engine's pixel space (docs/multi-pc.md,
    /// "Engine pixel space"): this machine's desktop stays where it is, and each remote
    /// one, in machine-id order, is translated to start [`HOST_GAP_PX`] to the right of
    /// everything before it, tops unchanged.
    ///
    /// A desktop's extent is every attached source of that machine, shown or not, so
    /// that switching a shared panel does not move another machine's desktop under the
    /// pointer. A machine with nothing attached has no zone and gets no offset.
    pub fn host_pixel_offsets(&self) -> Vec<HostOffset> {
        let extent = |host: Option<&str>| -> Option<Rect> {
            self.sources()
                .filter(|s| s.source.attached_to_desktop && s.source.host.as_deref() == host)
                .map(|s| s.source.in_pixel.bounds())
                .reduce(|a, b| {
                    // Min/max, not `Rect::union`: see `desktop_pixel_bounds`.
                    let left = a.left().min(b.left());
                    let top = a.top().min(b.top());
                    Rect::new(
                        left,
                        top,
                        a.right().max(b.right()) - left,
                        a.bottom().max(b.bottom()) - top,
                    )
                })
        };
        let mut right = extent(None).map_or(0, |r| r.right().ceil() as i32);
        let mut offsets = Vec::new();
        for host in self.hosts() {
            let Some(bounds) = extent(Some(&host)) else {
                continue;
            };
            let x = right + HOST_GAP_PX - bounds.left().floor() as i32;
            right = (bounds.right() + f64::from(x)).ceil() as i32;
            offsets.push(HostOffset { host, x, y: 0 });
        }
        offsets
    }

    /// Puts the monitors showing a remote source that were never placed to the right
    /// of the others, top-aligned, in the layout's order: a remote desktop has no pixel
    /// relation to this one to derive a placement from, and the physical size is all
    /// there is. `place_all` re-places those already placed as well, as it does for the
    /// local ones.
    pub(crate) fn place_remote_monitors(&mut self, place_all: bool) {
        let to_place: Vec<String> = self
            .monitors
            .iter()
            .filter(|m| {
                (place_all || !m.placed)
                    && self
                        .active_source_of(m)
                        .is_some_and(|s| !s.source.is_local())
            })
            .map(|m| m.id.clone())
            .collect();
        if to_place.is_empty() {
            return;
        }
        let others: Vec<Rect> = self
            .monitors
            .iter()
            .filter(|m| !to_place.contains(&m.id))
            .filter_map(|m| self.depth_projection(m))
            .map(|dp| dp.outside_bounds())
            .collect();
        let mut right = others
            .iter()
            .map(Rect::right)
            .reduce(f64::max)
            .unwrap_or(0.0);
        let top = others.iter().map(Rect::top).reduce(f64::min).unwrap_or(0.0);
        for id in to_place {
            let Some(monitor) = self.monitor(&id) else {
                continue;
            };
            let Some(outside) = self.depth_projection(monitor).map(|dp| dp.outside_bounds()) else {
                continue;
            };
            let location = monitor.location();
            let (dx, dy) = (right - outside.left(), top - outside.top());
            self.set_location(&id, Point::new(location.x + dx, location.y + dy));
            right = outside.right() + dx;
        }
    }
}
