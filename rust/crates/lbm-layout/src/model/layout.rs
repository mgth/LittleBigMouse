use crate::collation::invariant_compare;
use crate::geo::dotnet;
use crate::geo::{Rect, Thickness};

use super::ratio::inverse_of;
use super::{DisplaySize, LayoutOptions, Monitor, MonitorModel, PhysicalSource, Ratio};
use crate::solve::distance::{RectDistance, ThicknessDistance};

/// Where a layout comes from: C#'s `LayoutSource`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutSource {
    /// Built from the displays attached to this machine.
    #[default]
    System,
    /// Loaded from a virtual-layout file (debugging).
    VirtualFile,
    /// Imported from another machine's export.
    VirtualImport,
}

/// C#'s `DpiAwarenessKind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DpiAwareness {
    Invalid = -1,
    Unaware = 0,
    SystemAware = 1,
    #[default]
    PerMonitorAware = 2,
}

/// Values the C# layout does not compute on demand but *publishes*, from
/// DynamicData subscriptions, at precise moments. They are kept as fields and
/// recomputed at the same moments ([`Layout::parse_physical_monitors`],
/// [`Layout::parse_display_sources`]), because between two of those moments
/// C# reads the stale value — and the domain oracle recorded what C# read.
#[derive(Clone, Debug, PartialEq)]
struct Published {
    /// `MonitorsLayout.PhysicalBounds`: `default(Rect)` until the first parse.
    physical_bounds: Rect,
    /// `ILayoutOptions.MinimalMaxTravelDistance`.
    minimal_max_travel_distance: f64,
    /// Source id of `MonitorsLayout.PrimarySource`.
    primary_source: Option<String>,
    max_effective_dpi_x: f64,
    max_effective_dpi_y: f64,
    /// `ILayoutOptions.IsUnaryRatio`.
    is_unary_ratio: bool,
}

impl Default for Published {
    fn default() -> Self {
        Self {
            physical_bounds: Rect::default(),
            minimal_max_travel_distance: 0.0,
            primary_source: None,
            max_effective_dpi_x: 0.0,
            max_effective_dpi_y: 0.0,
            is_unary_ratio: false,
        }
    }
}

/// A monitor layout: C#'s `MonitorsLayout`.
///
/// Monitors and sources are kept in insertion order, which is the order C#'s
/// DynamicData caches enumerate them in. The order sources are *sorted* in
/// (by device id, for zones) is a separate view, `Layout::sorted_sources`.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub id: String,
    pub options: LayoutOptions,
    pub source_kind: LayoutSource,
    pub dpi_awareness: DpiAwareness,
    pub(crate) models: Vec<MonitorModel>,
    pub(crate) monitors: Vec<Monitor>,
    pub(crate) sources: Vec<PhysicalSource>,
    published: Published,
    pub(crate) saved: bool,
}

impl Layout {
    pub fn new(options: LayoutOptions) -> Self {
        Self {
            id: String::new(),
            options,
            source_kind: LayoutSource::System,
            dpi_awareness: DpiAwareness::PerMonitorAware,
            models: Vec::new(),
            monitors: Vec::new(),
            sources: Vec::new(),
            published: Published::default(),
            saved: false,
        }
    }

    /// `IsVirtual`: anything not built from this machine's displays.
    pub fn is_virtual(&self) -> bool {
        self.source_kind != LayoutSource::System
    }

    //==================//
    // Collections      //
    //==================//

    pub fn models(&self) -> &[MonitorModel] {
        &self.models
    }

    pub fn monitors(&self) -> &[Monitor] {
        &self.monitors
    }

    /// `PhysicalSources`, unsorted: the sources registered with the layout, in
    /// registration order.
    pub fn sources(&self) -> impl Iterator<Item = &PhysicalSource> {
        self.sources.iter().filter(|s| s.registered)
    }

    pub fn model(&self, pnp_code: &str) -> Option<&MonitorModel> {
        self.models.iter().find(|m| m.pnp_code == pnp_code)
    }

    pub fn monitor(&self, id: &str) -> Option<&Monitor> {
        self.monitors.iter().find(|m| m.id == id)
    }

    /// A source by id, registered or only attached to its monitor.
    pub fn source(&self, id: &str) -> Option<&PhysicalSource> {
        self.sources.iter().find(|s| s.source.id == id)
    }

    pub(crate) fn monitor_index(&self, id: &str) -> Option<usize> {
        self.monitors.iter().position(|m| m.id == id)
    }

    pub(crate) fn source_index(&self, id: &str) -> Option<usize> {
        self.sources.iter().position(|s| s.source.id == id)
    }

    /// `GetOrAddPhysicalMonitorModel`: monitors sharing a PnP code share one
    /// model; `init` only runs for a code not seen yet.
    pub fn get_or_add_model(
        &mut self,
        pnp_code: &str,
        init: impl FnOnce(&str) -> MonitorModel,
    ) -> &MonitorModel {
        let index = match self.models.iter().position(|m| m.pnp_code == pnp_code) {
            Some(i) => i,
            None => {
                self.models.push(init(pnp_code));
                self.models.len() - 1
            }
        };
        &self.models[index]
    }

    /// `AddOrUpdatePhysicalMonitor`: a monitor with a known id replaces the old
    /// one in place. Like a change on C#'s monitor cache, it republishes the
    /// monitor-derived values.
    pub fn add_or_update_monitor(&mut self, monitor: Monitor) {
        match self.monitor_index(&monitor.id) {
            Some(i) => self.monitors[i] = monitor,
            None => self.monitors.push(monitor),
        }
        self.parse_physical_monitors();
    }

    /// Makes `source` available to its monitor without registering it with
    /// the layout (C#: `monitor.Sources.Add(physicalSource)`).
    pub fn attach_source(&mut self, mut source: PhysicalSource) {
        match self.source_index(&source.source.id) {
            Some(i) => {
                source.registered = self.sources[i].registered;
                source.observed_primary = self.sources[i].observed_primary.clone();
                self.sources[i] = source;
            }
            None => {
                source.observed_primary = self.published.primary_source.clone();
                self.sources.push(source);
            }
        }
    }

    /// [`Layout::attach_source`] for a source object built just now under an id
    /// the layout may already hold: C#'s Windows builder adds a clone's source
    /// that way, a new `PhysicalSource` under an existing key. Where
    /// `attach_source` keeps what the slot's previous object observed, the new
    /// object observes the primary published at its construction.
    pub(crate) fn attach_new_source(&mut self, mut source: PhysicalSource) {
        source.observed_primary = self.published.primary_source.clone();
        match self.source_index(&source.source.id) {
            Some(i) => {
                source.registered = self.sources[i].registered;
                self.sources[i] = source;
            }
            None => self.sources.push(source),
        }
    }

    /// `AddOrUpdatePhysicalSource`, keyed by the display source id: attaches
    /// and registers the source, then republishes the source-derived values.
    pub fn add_or_update_source(&mut self, mut source: PhysicalSource) {
        source.registered = true;
        match self.source_index(&source.source.id) {
            Some(i) => {
                source.observed_primary = self.sources[i].observed_primary.clone();
                self.sources[i] = source;
            }
            None => {
                source.observed_primary = self.published.primary_source.clone();
                self.sources.push(source);
            }
        }
        self.parse_display_sources();
    }

    /// `MonitorsLayout.PhysicalSources`: the sources sorted by device id with
    /// the invariant-culture comparer (stable, like DynamicData's insertion).
    pub fn sorted_sources(&self) -> Vec<&PhysicalSource> {
        let mut sorted: Vec<&PhysicalSource> = self.sources().collect();
        sorted.sort_by(|a, b| invariant_compare(&a.device_id, &b.device_id));
        sorted
    }

    /// `LayoutIdExtensions.ComputeId`: every monitor's id, suffixed `_<n>` when
    /// its active source is turned `n` quarter turns, sorted with the
    /// invariant-culture comparer (stable), joined with `+`. This is the key the
    /// layout is stored under.
    pub fn compute_id(&self) -> String {
        let mut ids: Vec<String> = self
            .monitors
            .iter()
            .map(|m| match self.orientation(m).unwrap_or(0) {
                0 => m.id.clone(),
                o => format!("{}_{o}", m.id),
            })
            .collect();
        ids.sort_by(|a, b| invariant_compare(a, b));
        ids.join("+")
    }

    //==================//
    // Geometry         //
    //==================//

    /// The active source's orientation, when the monitor has one.
    pub(crate) fn orientation(&self, monitor: &Monitor) -> Option<i32> {
        let id = monitor.active_source.as_deref()?;
        Some(self.source(id)?.source.orientation)
    }

    fn model_of(&self, monitor: &Monitor) -> &MonitorModel {
        self.model(&monitor.model)
            .expect("a monitor always references a model of its layout")
    }

    /// `PhysicalMonitor.EffectivePhysicalSize`: the model's size, or the same
    /// size with this monitor's own borders when "Border values" is per
    /// monitor (`MonitorBorderPolicy`). While the monitor's borders are not
    /// customized they mirror the model's, so both read alike.
    pub fn effective_physical_size(&self, monitor: &Monitor) -> DisplaySize {
        let model = self.model_of(monitor).physical_size.as_display_size();
        if self.options.per_monitor_borders() {
            model.with_borders(self.monitor_borders(monitor))
        } else {
            model
        }
    }

    /// The borders a monitor's `DisplayBorders` hold. The layout keeps them
    /// mirroring the model's while the monitor does not own them (see
    /// `Layout::edit_model`).
    pub fn monitor_borders(&self, monitor: &Monitor) -> Thickness {
        monitor.borders
    }

    /// `PhysicalRotated`: the effective size turned by the active source's
    /// orientation. `None` without an active source: C#'s chain waits for one.
    pub fn physical_rotated(&self, monitor: &Monitor) -> Option<DisplaySize> {
        let orientation = self.orientation(monitor)?;
        Some(self.effective_physical_size(monitor).rotate(orientation))
    }

    /// `DepthProjection`: rotated, scaled by the depth ratio, at the monitor's
    /// location.
    pub fn depth_projection(&self, monitor: &Monitor) -> Option<DisplaySize> {
        Some(
            self.physical_rotated(monitor)?
                .scale(monitor.depth_ratio)
                .located(monitor.location),
        )
    }

    /// `DepthProjectionUnrotated`: the effective size scaled by the depth
    /// ratio, at the model size's own location.
    pub fn depth_projection_unrotated(&self, monitor: &Monitor) -> DisplaySize {
        self.effective_physical_size(monitor)
            .scale(monitor.depth_ratio)
    }

    /// `Diagonal`: `Math.Sqrt(w * w + h * h)` of the depth projection.
    pub fn diagonal(&self, monitor: &Monitor) -> Option<f64> {
        let dp = self.depth_projection(monitor)?;
        Some((dp.width * dp.width + dp.height * dp.height).sqrt())
    }

    //==================//
    // Source ratios    //
    //==================//

    fn monitor_of(&self, source: &PhysicalSource) -> Option<&Monitor> {
        self.monitor(&source.monitor)
    }

    /// `InDip`: the pixel rectangle, location included, times 96 / effective DPI.
    pub fn in_dip(&self, source: &PhysicalSource) -> DisplaySize {
        source
            .source
            .in_pixel
            .scale_dip(source.source.effective_dpi)
    }

    /// `RealPitch`: mm per pixel of the rotated panel, per axis.
    pub fn real_pitch(&self, source: &PhysicalSource) -> Option<Ratio> {
        let rotated = self.physical_rotated(self.monitor_of(source)?)?;
        let px = &source.source.in_pixel;
        Some(Ratio::new(
            rotated.width / px.width,
            rotated.height / px.height,
        ))
    }

    /// `Pitch`: the real pitch times the monitor's depth ratio.
    pub fn pitch(&self, source: &PhysicalSource) -> Option<Ratio> {
        let depth = self.monitor_of(source)?.depth_ratio;
        Some(self.real_pitch(source)?.multiply(depth))
    }

    /// `RealDpi`: `25.4 * (1 / realPitch)` — an inverse then a product, which
    /// is not always the same double as `25.4 / realPitch`.
    pub fn real_dpi(&self, source: &PhysicalSource) -> Option<Ratio> {
        Some(Ratio::uniform(25.4).multiply(self.real_pitch(source)?.inverse()))
    }

    /// `Dpi`: `25.4 * (1 / pitch)`.
    pub fn dpi(&self, source: &PhysicalSource) -> Option<Ratio> {
        Some(Ratio::uniform(25.4).multiply(self.pitch(source)?.inverse()))
    }

    /// `DipToPixelRatio` (`PhysicalSource.UpdateDipToPixelRatio`).
    ///
    /// C# combines the source's own values with the DPI of the primary source
    /// it observes through `Monitor.Layout.PrimarySource`. The layout publishes
    /// its primary inside `SuppressChangeNotifications()`, so a source only ever
    /// sees the primary published when it joined the layout (see
    /// `PhysicalSource::observed_primary`): `None` if there was none then, and
    /// until the source's real DPI exists (it needs the monitor's rotated size).
    /// Then, by DPI awareness: unaware, real over angular DPI rounded to a tenth;
    /// system aware, the observed primary's effective DPI over 96; per-monitor
    /// aware (the default) or invalid, this source's effective DPI over 96.
    pub fn dip_to_pixel_ratio(&self, source: &PhysicalSource) -> Option<Ratio> {
        let primary = self.source(source.observed_primary.as_deref()?)?;
        let real = self.real_dpi(source)?;
        let d = &source.source;
        Some(match self.dpi_awareness {
            DpiAwareness::Unaware => Ratio::new(
                dotnet::round(real.x / d.dpi_aware_angular_dpi.x * 10.0) / 10.0,
                dotnet::round(real.y / d.dpi_aware_angular_dpi.y * 10.0) / 10.0,
            ),
            DpiAwareness::SystemAware => Ratio::new(
                primary.source.effective_dpi.x / 96.0,
                primary.source.effective_dpi.y / 96.0,
            ),
            DpiAwareness::PerMonitorAware | DpiAwareness::Invalid => {
                Ratio::new(d.effective_dpi.x / 96.0, d.effective_dpi.y / 96.0)
            }
        })
    }

    /// `PixelToDipRatio`: the inverse of [`Layout::dip_to_pixel_ratio`], so
    /// [`Ratio::ZERO`].
    pub fn pixel_to_dip_ratio(&self, source: &PhysicalSource) -> Ratio {
        inverse_of(self.dip_to_pixel_ratio(source))
    }

    /// `PhysicalToPixelRatio`: `1 / pitch`.
    pub fn physical_to_pixel_ratio(&self, source: &PhysicalSource) -> Option<Ratio> {
        Some(self.pitch(source)?.inverse())
    }

    /// `MmToDipRatio`: `physicalToPixel * (1 / dipToPixel) * depthRatio`, left
    /// to right.
    pub fn mm_to_dip_ratio(&self, source: &PhysicalSource) -> Option<Ratio> {
        let depth = self.monitor_of(source)?.depth_ratio;
        Some(
            self.physical_to_pixel_ratio(source)?
                .multiply(inverse_of(self.dip_to_pixel_ratio(source)))
                .multiply(depth),
        )
    }

    //==================//
    // Published values //
    //==================//

    pub fn physical_bounds(&self) -> Rect {
        self.published.physical_bounds
    }

    /// `X0`: `-PhysicalBounds.Left`.
    pub fn x0(&self) -> f64 {
        -self.published.physical_bounds.left()
    }

    /// `Y0`: `-PhysicalBounds.Top`.
    pub fn y0(&self) -> f64 {
        -self.published.physical_bounds.top()
    }

    pub fn minimal_max_travel_distance(&self) -> f64 {
        self.published.minimal_max_travel_distance
    }

    pub fn is_unary_ratio(&self) -> bool {
        self.published.is_unary_ratio
    }

    pub fn max_effective_dpi(&self) -> (f64, f64) {
        (
            self.published.max_effective_dpi_x,
            self.published.max_effective_dpi_y,
        )
    }

    pub fn primary_source(&self) -> Option<&PhysicalSource> {
        self.source(self.published.primary_source.as_deref()?)
    }

    pub fn primary_monitor(&self) -> Option<&Monitor> {
        self.monitor_of(self.primary_source()?)
    }

    /// `ParsePhysicalMonitors`: the minimal travel distance (from the primary
    /// *as currently published*), then the union of every monitor's outside
    /// bounds with HLab.Geo's union (empty as soon as one operand is).
    pub fn parse_physical_monitors(&mut self) {
        self.published.minimal_max_travel_distance =
            self.compute_minimal_max_travel_distance().ceil();
        let mut bounds = self
            .monitors
            .iter()
            .filter_map(|m| self.depth_projection(m))
            .map(|dp| dp.outside_bounds());
        let Some(first) = bounds.next() else {
            return;
        };
        self.published.physical_bounds = bounds.fold(first, |acc, b| acc.union(&b));
    }

    /// `ParseDisplaySources`, over the sources in insertion order: the last
    /// primary one wins, the effective DPIs fold with `Math.Max` from 0, and
    /// the ratio is unary while every source's pixel-to-DIP ratio is.
    pub fn parse_display_sources(&mut self) {
        let mut max_x = 0.0;
        let mut max_y = 0.0;
        let mut is_unary = true;
        let mut primary = None;
        let mut unsaved = false;
        for source in self.sources.iter().filter(|s| s.registered) {
            if source.source.primary {
                primary = Some(source.source.id.clone());
            }
            max_x = dotnet::max(max_x, source.source.effective_dpi.x);
            max_y = dotnet::max(max_y, source.source.effective_dpi.y);
            if is_unary && !self.pixel_to_dip_ratio(source).is_unary() {
                is_unary = false;
            }
            unsaved |= !source.saved;
        }
        if unsaved {
            self.saved = false;
        }
        self.published.primary_source = primary;
        self.published.max_effective_dpi_x = max_x;
        self.published.max_effective_dpi_y = max_y;
        self.published.is_unary_ratio = is_unary;
    }

    /// `TravelDistanceHelper.GetMinimalMaxTravelDistance`: from the primary,
    /// every monitor reached through its nearest already-reached neighbour; the
    /// longest hop of that tree. 0 without a primary.
    fn compute_minimal_max_travel_distance(&self) -> f64 {
        let Some(primary) = self.primary_monitor() else {
            return 0.0;
        };
        let Some(primary_bounds) = self.depth_projection(primary).map(|dp| dp.bounds()) else {
            return 0.0;
        };
        let bounds: Vec<Rect> = self
            .monitors
            .iter()
            .map(|m| {
                self.depth_projection(m)
                    .map_or(Rect::EMPTY, |dp| dp.bounds())
            })
            .collect();
        let primary_index = self
            .monitors
            .iter()
            .position(|m| m.id == primary.id)
            .expect("the primary monitor is one of the layout's");
        minimal_max_travel_distance(primary_index, primary_bounds, &bounds)
    }

    //==================//
    // Saved            //
    //==================//

    /// `MonitorsLayout.Saved`.
    pub fn saved(&self) -> bool {
        self.saved
    }
}

/// One edge of the travel tree: `MonitorDistance` in C#.
#[derive(Clone, Copy, Debug)]
struct Hop {
    source: usize,
    target: usize,
    distance: f64,
}

/// The travel-tree walk of `GetMinimalMaxTravelDistance`, over monitor indices
/// (C# compares the `MonitorDistance` objects by reference; indices here).
fn minimal_max_travel_distance(primary: usize, primary_bounds: Rect, bounds: &[Rect]) -> f64 {
    let mut hops: Vec<Hop> = bounds
        .iter()
        .enumerate()
        .map(|(i, b)| Hop {
            source: primary,
            target: i,
            distance: primary_bounds.distance(b).distance_hv(),
        })
        .collect();

    let mut progress = true;
    while progress {
        // `OrderBy(d => d.Distance)`: stable, `double.CompareTo` order.
        hops.sort_by(|a, b| dotnet::compare(a.distance, b.distance));
        let last = hops.len() - 1;
        progress = false;
        // `distances.Except([last])`: every other hop, in the sorted order.
        for other in 0..last {
            // Is the last hop's target already on this hop's chain to the primary?
            let mut d = hops[other];
            while d.source != primary {
                if d.target == hops[last].target {
                    break;
                }
                d = *hops
                    .iter()
                    .find(|e| e.target == d.source)
                    .expect("every chain leads back to the primary");
            }
            if d.target == hops[last].target {
                continue;
            }
            let candidate = bounds[hops[last].target]
                .distance(&bounds[hops[other].target])
                .distance_hv();
            if candidate >= hops[last].distance {
                continue;
            }
            hops[last].source = hops[other].target;
            hops[last].distance = candidate;
            progress = true;
        }
    }
    dotnet::enumerable_max(hops.iter().map(|h| h.distance)).unwrap_or(0.0)
}
