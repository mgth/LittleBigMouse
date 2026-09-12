//! The bridges between the model and the placement solvers: C#'s
//! `MonitorSnapshotExtensions`, `MonitorsLayout.Compact/ForceCompact`,
//! `MonitorsLocationsFromSystemExtensions` and
//! `MonitorsLocationsToSystemExtensions`. Snapshot the monitors, solve, write
//! the answers back through the layout's edits.

use std::collections::HashSet;

use crate::geo::{Point, Rect, Size};
use crate::solve::compaction::{self, CompactionMonitor, CompactionOptions};
use crate::solve::layout_geometry::MonitorSnapshot;
use crate::solve::pixel_location::{self, DEFAULT_TOLERANCE_MM};
use crate::solve::system_location;
use crate::solve::wayland_scale::adjusted_scale;

use super::{Layout, Monitor};

/// What "apply the layout to the system" asks for one source: C#'s
/// `SystemPlacement`.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemPlacement {
    pub pixel_bounds: Rect,
    /// The scale to switch to, when the Wayland adjustment proposes one.
    pub scale: Option<f64>,
}

impl Layout {
    /// `MonitorSnapshotExtensions.Snapshot`: the monitor's depth projection and
    /// its active source's pixel rectangle — resized to `pixel_size` when given.
    fn snapshot(
        &self,
        monitor: &Monitor,
        primary: bool,
        pixel_size: Option<Size>,
    ) -> Option<MonitorSnapshot> {
        let source = self.source(monitor.active_source.as_deref()?)?;
        let pixel = source.source.in_pixel.bounds();
        let projection = self.depth_projection(monitor)?;
        Some(MonitorSnapshot::new(
            monitor.id.clone(),
            projection.bounds(),
            projection.outside_bounds(),
            match pixel_size {
                Some(size) => Rect::from_location_size(pixel.location(), size),
                None => pixel,
            },
            primary,
        ))
    }

    /// `MonitorsLayout.Compact`: nothing when discontinuity is allowed.
    pub fn compact(&mut self) {
        if self.options.allow_discontinuity {
            return;
        }
        self.force_compact();
    }

    /// `MonitorsLayout.ForceCompact`: with a primary and at least two monitors,
    /// every monitor moved by what `CompactionSolver` answers for it.
    pub fn force_compact(&mut self) {
        let Some(primary) = self.primary_monitor().map(|m| m.id.clone()) else {
            return;
        };
        if self.monitors.len() < 2 {
            return;
        }
        let snapshot: Vec<CompactionMonitor> = self
            .monitors
            .iter()
            .filter_map(|m| {
                let dp = self.depth_projection(m)?;
                Some(CompactionMonitor::new(
                    m.id.clone(),
                    dp.bounds(),
                    dp.outside_bounds(),
                    m.id == primary,
                ))
            })
            .collect();
        let offsets = compaction::solve(
            &snapshot,
            CompactionOptions::new(
                self.options.allow_overlaps,
                self.options.allow_discontinuity,
                self.options.minimal_edge_overlap,
            ),
        );
        let ids: Vec<String> = self.monitors.iter().map(|m| m.id.clone()).collect();
        for id in ids {
            if let Some(offset) = offsets.get(&id).copied() {
                self.offset_location(&id, offset);
            }
        }
    }

    /// `SetLocationsFromSystemConfiguration`: millimetre positions from the
    /// system's pixel arrangement, for every monitor (`place_all`) or only the
    /// ones not placed yet; then a compaction, then the published values.
    /// Nothing happens without a primary, or with nothing to place.
    pub fn set_locations_from_system_configuration(&mut self, place_all: bool) {
        let (Some(_), Some(primary)) = (
            self.primary_source().map(|s| s.source.id.clone()),
            self.primary_monitor().map(|m| m.id.clone()),
        ) else {
            return;
        };
        // A monitor with no active source has no pixel rectangle to be placed from.
        let monitors: Vec<&Monitor> = self
            .monitors
            .iter()
            .filter(|m| {
                m.active_source
                    .as_deref()
                    .is_some_and(|id| self.source(id).is_some())
            })
            .collect();
        let to_place: HashSet<String> = monitors
            .iter()
            .filter(|m| place_all || !m.placed)
            .map(|m| m.id.clone())
            .collect();
        if to_place.is_empty() {
            return;
        }
        let snapshot: Vec<MonitorSnapshot> = monitors
            .iter()
            .filter_map(|m| self.snapshot(m, m.id == primary, None))
            .collect();
        let ids: Vec<String> = monitors.iter().map(|m| m.id.clone()).collect();
        let positions = system_location::solve(&snapshot, Some(&to_place));
        for id in ids {
            if let Some(position) = positions.get(&id).copied() {
                self.set_location(&id, position);
            }
        }
        self.force_compact();
        self.parse_physical_monitors();
    }

    /// `AnchorOnPrimary`: everything translated so the primary's panel sits at
    /// (0, 0) mm — unless it already sits within a thousandth of a millimetre,
    /// in which case nothing is touched and the saved state stays as it was.
    pub fn anchor_on_primary(&mut self) {
        let Some(projection) = self
            .primary_monitor()
            .and_then(|m| self.depth_projection(m))
        else {
            return;
        };
        let (dx, dy) = (projection.x, projection.y);
        if dx.abs() < 0.001 && dy.abs() < 0.001 {
            return;
        }
        let ids: Vec<String> = self.monitors.iter().map(|m| m.id.clone()).collect();
        for id in ids {
            if let Some(location) = self.monitor(&id).map(|m| m.location) {
                self.set_location(&id, Point::new(location.x - dx, location.y - dy));
            }
        }
        self.parse_physical_monitors();
    }

    /// `ComputePixelLocationsFromPhysical`: the pixel rectangle each attached
    /// source should get so the system arrangement matches the millimetre
    /// layout — with, under `adjust_scale`, the Wayland scale that makes every
    /// monitor's logical pitch the primary's. Keyed by source id, in the
    /// solver's order. Empty without a primary showing a source.
    pub fn compute_pixel_locations_from_physical(
        &self,
        adjust_scale: bool,
    ) -> Vec<(String, SystemPlacement)> {
        let Some(primary) = self.primary_monitor() else {
            return Vec::new();
        };
        let Some(primary_source) = primary
            .active_source
            .as_deref()
            .and_then(|id| self.source(id))
        else {
            return Vec::new();
        };
        let monitors: Vec<&Monitor> = self
            .monitors
            .iter()
            .filter(|m| {
                m.active_source
                    .as_deref()
                    .and_then(|id| self.source(id))
                    .is_some_and(|s| s.source.attached_to_desktop)
            })
            .collect();
        if monitors.is_empty() {
            return Vec::new();
        }
        let Some(primary_projection) = self.depth_projection(primary) else {
            return Vec::new();
        };
        let target_pitch =
            primary_projection.bounds().width() / primary_source.source.in_pixel.width;

        let mut inputs = Vec::new();
        let mut sizes: Vec<(String, String, Size, Option<f64>)> = Vec::new();
        for monitor in monitors {
            let Some(source) = monitor
                .active_source
                .as_deref()
                .and_then(|id| self.source(id))
            else {
                continue;
            };
            let Some(projection) = self.depth_projection(monitor) else {
                continue;
            };
            let mut pixel_size = source.source.in_pixel.bounds().size();
            let mut new_scale = None;
            if adjust_scale {
                if let Some((wanted, size)) = adjusted_scale(
                    target_pitch,
                    source.source.effective_dpi.x,
                    source.source.in_pixel.width,
                    source.source.in_pixel.height,
                    projection.bounds().width(),
                ) {
                    new_scale = Some(wanted);
                    pixel_size = size;
                }
            }
            if let Some(snapshot) =
                self.snapshot(monitor, monitor.id == primary.id, Some(pixel_size))
            {
                inputs.push(snapshot);
            }
            match sizes.iter_mut().find(|(id, ..)| *id == monitor.id) {
                Some(entry) => {
                    *entry = (
                        monitor.id.clone(),
                        source.source.id.clone(),
                        pixel_size,
                        new_scale,
                    )
                }
                None => sizes.push((
                    monitor.id.clone(),
                    source.source.id.clone(),
                    pixel_size,
                    new_scale,
                )),
            }
        }

        let solved = pixel_location::solve(&inputs, DEFAULT_TOLERANCE_MM);
        let mut result: Vec<(String, SystemPlacement)> = Vec::new();
        for (id, position) in solved.iter() {
            let Some((_, source, pixel_size, scale)) = sizes.iter().find(|(m, ..)| m == id) else {
                continue;
            };
            let placement = SystemPlacement {
                pixel_bounds: Rect::from_location_size(*position, *pixel_size),
                scale: *scale,
            };
            match result.iter_mut().find(|(s, _)| s == source) {
                Some(entry) => entry.1 = placement,
                None => result.push((source.clone(), placement)),
            }
        }
        result
    }
}
