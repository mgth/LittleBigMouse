/// "Border values": bezel borders are shared by every monitor of a model.
pub const PER_MODEL: &str = "PerModel";
/// "Border values": each physical monitor keeps its own bezel borders.
pub const PER_MONITOR: &str = "PerMonitor";

/// The options a layout carries: C#'s `ILayoutOptions`, with the defaults of
/// its only implementation, `LbmOptions`.
///
/// Two members of the C# interface are not stored here because the layout
/// publishes them: `MinimalMaxTravelDistance` and `IsUnaryRatio` (see
/// `Layout`). `LoopAllowed` is always true in C# and is not ported.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutOptions {
    pub enabled: bool,
    pub allow_overlaps: bool,
    pub allow_discontinuity: bool,
    /// Wire value, spelled as the daemon matches it: "Strait" or "Cross".
    pub algorithm: String,
    pub minimal_edge_overlap: f64,
    pub max_travel_distance: f64,
    pub freelook_check_interval: f64,
    pub freelook_enabled: bool,
    pub loop_x: bool,
    pub loop_y: bool,
    pub adjust_pointer: bool,
    pub adjust_speed: bool,
    /// [`PER_MODEL`] or [`PER_MONITOR`].
    pub border_values: String,
    pub rescue_shortcut: String,
    /// The hook belongs to the agent that drives it: when their connection ends it
    /// lets go of the mice and leaves. Off by default — a hook outliving its agent is
    /// what keeps the cursor routing across a crash (D5), and this is the option that
    /// trades that away for no resident process.
    pub bound_to_agent: bool,
    pub priority: String,
    pub priority_unhooked: String,
    pub auto_update: bool,
    pub home_cinema: bool,
    pub pinned: bool,
    pub start_minimized: bool,
    pub start_elevated: bool,
    pub hide_tray_icon: bool,
    pub debug_tools: bool,
    pub experimental_features: bool,
    pub vcp_control: bool,
    pub show_monitor_action_warning: bool,
    /// Read from the platform (a scheduled task on Windows), never stored.
    pub load_at_startup: bool,
    /// Read from the process token on Windows, never stored.
    pub elevated: bool,
    pub excluded_list: Vec<String>,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_overlaps: false,
            allow_discontinuity: false,
            algorithm: "Strait".to_owned(),
            minimal_edge_overlap: 20.0,
            max_travel_distance: 200.0,
            freelook_check_interval: 100.0,
            freelook_enabled: true,
            loop_x: false,
            loop_y: false,
            adjust_pointer: false,
            adjust_speed: false,
            border_values: PER_MODEL.to_owned(),
            rescue_shortcut: "Ctrl+Alt+Shift+M".to_owned(),
            bound_to_agent: false,
            priority: "Normal".to_owned(),
            priority_unhooked: "Below".to_owned(),
            auto_update: false,
            home_cinema: false,
            pinned: false,
            start_minimized: false,
            start_elevated: false,
            hide_tray_icon: false,
            debug_tools: false,
            experimental_features: false,
            vcp_control: false,
            show_monitor_action_warning: true,
            load_at_startup: false,
            elevated: false,
            excluded_list: Vec::new(),
        }
    }
}

impl LayoutOptions {
    pub fn per_monitor_borders(&self) -> bool {
        self.border_values == PER_MONITOR
    }
}
