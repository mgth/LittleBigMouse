//! The frontend's window: what `lbm-ui` draws, on a screen someone can look at.
//!
//! Everything up to now has been a library with no window — driven and asserted on
//! through the accessibility tree, which is how it can be tested at all on a Wayland
//! machine. That is worth keeping, so `lbm-ui` stays window-free and everything that
//! needs winit and a GL context lives here.
//!
//! The window starts a sibling agent when the normal session endpoint is absent,
//! then reconnects in the background. An explicit LBM_AGENT_ENDPOINT is connect-only.
//! All writes and mouse-engine ownership stay with the resident agent.
//!
//! The map reads options and the saved profile, so editing begins with the physical
//! arrangement the user saved. SaveLayout and SaveOptions go through the agent, the
//! only writer. Exclusions use the read-only reader, never the loader that seeds a
//! missing Excluded.txt. Display enumeration uses lbm-display and lbm-layout directly;
//! the frontend does not link or supervise the hook.

use std::collections::HashMap;
use std::path::PathBuf;

use lbm_app::client::{self, Message};
use lbm_icons::{Catalogue, Rgba};
use lbm_layout::geo::{Rect, Vector};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_ui::map::MapMonitor;
use lbm_ui::{drag, frame, list, map};

type DisplayReports = HashMap<String, Vec<(String, String)>>;

/// One monitor, owned, because `MapMonitor` borrows.
struct Screen {
    id: String,
    /// The zone report names display sources, not physical monitor ids.
    source_id: String,
    name: String,
    logo: Option<PathBuf>,
    mm_outside: Rect,
    mm_content: Rect,
    /// The source's rectangle in cursor pixels — `ActiveSource.Source.InPixel.Bounds`,
    /// which is what the thumbnail is cut to.
    pixels: Rect,
    /// What the chosen view mode writes inside this screen. Built with the screens, so
    /// the rows and the rectangles are read off one layout and cannot drift apart.
    details: Vec<(&'static str, String)>,
}

/// Background results. Screens arrive before the wallpaper decoding completes.
enum Found {
    /// The detected layout; the window derives its frames in the current view mode.
    Displays {
        generation: u64,
        layout: Box<Layout>,
        reports: DisplayReports,
        /// The model sizes before the stored profile was applied.
        detected: lbm_app::properties::Detected,
        /// `None` when `Excluded.txt` does not exist yet.
        excluded: Option<Vec<String>>,
    },
    /// Something the agent said.
    Session(lbm_app::session::Event),
    DetectionFailed {
        generation: u64,
    },
    WallpaperFile {
        choice: Box<lbm_app::wallpaper_editor::FileChoice>,
        result: lbm_app::image_picker::Selection,
    },
    WallpaperPreview {
        generation: u64,
        document: Box<lbm_store::LayoutDocument>,
        result: Result<WallpaperPreview, String>,
    },
}

struct WallpaperPreview {
    screens: Vec<lbm_wallpaper::render::Painted>,
    panorama: Option<(String, lbm_layout::geo::Size, image::RgbaImage)>,
}

/// A screen in hand: which one, and how far the pointer has taken it.
///
/// `by` is in points and covers the whole gesture, press to now — the map turns it into
/// millimetres, because the conversion is the map's ratio and only the map knows it.
struct Drag {
    id: String,
    by: egui::Vec2,
}

/// What the pointer did to the map, in a form that no longer borrows the screens.
///
/// The views hand back a `&str` into `self.screens`, and every one of these answers ends
/// in changing `self` — so the borrow has to be given up in between.
enum Did {
    Clicked(String),
    Dragged { id: String, by: egui::Vec2 },
    Dropped { id: String },
}

/// Which view the window is showing — `MainViewModel.ViewList`, a toggle of its own and
/// not one of the plugin-contributed view modes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Map,
    List,
}

struct App {
    /// Not yet arrived. Finding the displays runs `kscreen-doctor` — a **subprocess** —
    /// and the architecture note this crate was written against says effects never run
    /// on the UI thread. It takes 77 ms here, which is not a stall, but the thread is
    /// what the agent connection will need and it is better exercised now than invented
    /// later.
    arriving: Option<std::sync::mpsc::Receiver<Found>>,
    found: std::sync::mpsc::Sender<Found>,
    _session: Option<lbm_app::session::Session>,
    instance: Option<lbm_app::instance::Inbox>,
    replacement_draft: Option<serde_json::Value>,
    refresh: lbm_app::refresh::Refresh,
    detecting: Option<u64>,
    retry_display_at: std::time::Instant,
    display_error: Option<String>,
    /// Everything the agent has said, and the requests still out.
    agent: lbm_app::conversation::Conversation,
    probe: lbm_app::probe::Probe,
    show_probe: bool,
    outputs: lbm_app::displays::Displays,
    vcp: lbm_app::vcp::Controller,
    remote: lbm_app::remote::Controller,
    test_patterns: lbm_app::patterns::Controller,
    pattern_strip: lbm_ui::patterns::Strip,
    /// The layout the screens were read off, kept so that the window can edit it.
    ///
    /// **In memory only, and this window writes no file.** A drag changes this copy and
    /// nothing else; what makes an edit last is `SaveLayout`, which hands it to the agent
    /// — the only writer. Whether there is anything to hand over is [`App::saved`].
    layout: Option<Box<Layout>>,
    /// The document this window loaded, or last saved — what Save and Undo compare
    /// against. See [`lbm_app::saved`] for why it is not the store.
    saved: Option<lbm_app::saved::Reference>,
    screens: Vec<Screen>,
    reports: DisplayReports,
    /// The model sizes as detected, which the size editor must not contradict while the
    /// agent predates `IntrinsicSize`.
    detected: lbm_app::properties::Detected,
    /// The connected agent announced `IntrinsicSize`: a size may cross the detected shape.
    agent_keeps_intrinsic_size: bool,
    property_error: Option<String>,
    resistance: lbm_ui::resistance::Editor,
    screen_overlay: lbm_overlay::Overlay,
    screen_input: lbm_app::screen_input::Controller,
    screen_overlay_mode: lbm_overlay::Mode,
    screen_overlay_error: Option<String>,
    next_screen_overlay: std::time::Instant,
    selected: Option<String>,
    /// The screen the pointer is holding, while it holds it.
    drag: Option<Drag>,
    /// The processes the engine stands aside for, once they have been read.
    ///
    /// `None` is **not** an empty list: it is "not read", and it is what keeps the window
    /// from offering exclusions it does not have to be removed, and from sending an empty
    /// list over the user's real one. See [`read_excluded`].
    excluded: Option<Vec<String>>,
    /// What the user is typing into the exclusion box.
    pattern: String,
    /// Whether the apply asks for the scales too. Remembered between openings of the
    /// menu, not across runs — it is a choice about this apply, not a setting.
    placement: lbm_ui::placement::Panel,
    topology_pending: Option<u64>,
    /// Whether the rescue recorder is waiting for a combination.
    recording: lbm_ui::shortcut::Recording,
    /// The document the agent was last given, as it went on the wire, and when.
    ///
    /// `None` means "unknown", which makes the next tick send whatever the layout is —
    /// `LiveLayoutUpdater.Forget`. The comparison is the gate that keeps a live preview
    /// from costing anything while nothing moves: an edit the agent cannot see (a value
    /// set back to itself) produces no request, and the agent is never made to swap a
    /// layout for an identical one.
    ///
    /// The C# has a cheaper gate in front of this one — a revision counter, so a still
    /// layout is one integer read. There is no such counter here and none is invented:
    /// building the document is a walk over owned data rather than a reactive graph, and
    /// at five times a second it does not show.
    previewed: Option<(String, std::time::Instant)>,
    view: View,
    settings_open: bool,
    pending_options: lbm_app::settings::PendingOptions,
    about_open: bool,
    settings_page: lbm_ui::options::Page,
    /// What every frame shows inside it — the mode bar's choice. See `lbm_ui::mode`.
    mode: lbm_ui::mode::Mode,
    icons: Catalogue,
    /// Uploaded logos, by icon path. Loaded once, on the frame that first needs one.
    textures: HashMap<String, egui::TextureHandle>,
    desktop_wallpaper: lbm_app::desktop_wallpaper::Watcher,
    desktop_textures: HashMap<String, egui::TextureHandle>,
    /// Uploaded editor previews, by screen id.
    wallpapers: HashMap<String, egui::TextureHandle>,
    wallpaper: lbm_app::wallpaper_editor::Editor,
    wallpaper_preview_requested: bool,
    panorama: Option<(String, lbm_layout::geo::Size, egui::TextureHandle)>,
    panorama_editor: lbm_ui::panorama::Editor,
    wallpaper_picking: bool,
    /// Where Browse was pressed (view, mode, screen), while its chooser is open. Checked
    /// on every pass: any departure retires the choice, even if the user comes back.
    wallpaper_choice_context: Option<(View, bool, lbm_ui::mode::Mode, Option<String>)>,
    picker_parent: lbm_app::image_picker::Parent,
    wallpaper_preview_busy: Option<u64>,
    /// What the bottom bar shows, from what the agent says.
    state: lbm_ui::State,
    /// What to ask the agent, when there is one to ask.
    requests: Option<lbm_app::session::Requests>,
    /// The id the next request will carry. The window's own counter — see [`Asks`].
    next_id: u64,
    /// Why there is nothing to ask, when there is not: a short reason for the bar and
    /// the endpoint it was looking for, which belongs in a tooltip and not across the
    /// top of the window.
    without_agent: Option<(String, String)>,
}

/// Where the icons are.
///
/// `$LBM_ICONS` first, then a directory beside the executable, then — for a build run
/// out of the source tree — the Avalonia project's assets. **Where these files should
/// live once the frontend is packaged is not settled**: they are inside the C# UI
/// project, which phase 7 deletes.
fn icons_root() -> Option<PathBuf> {
    if let Some(from_env) = std::env::var_os("LBM_ICONS") {
        let path = PathBuf::from(from_env);
        if path.is_dir() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(beside) = exe.parent().map(|d| d.join("icons")) {
            if beside.is_dir() {
                return Some(beside);
            }
        }
    }
    let in_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets");
    in_tree.is_dir().then_some(in_tree)
}

/// The displays now, as the agent's `Discovery` finds them — the same two calls, without
/// the agent.
///
/// The layout comes back whole rather than reduced to its screens, because the window
/// keeps it: a drag writes a position into it and [`screens_of`] reads the result.
fn detect() -> Option<(Box<Layout>, DisplayReports, lbm_app::properties::Detected)> {
    let mut layout = Layout::new(LayoutOptions::default());
    let mut reports = HashMap::new();

    #[cfg(windows)]
    let built = {
        match lbm_display::windows::discover() {
            Ok(tree) => {
                for monitor in tree.monitors() {
                    reports.insert(
                        monitor.id.clone(),
                        lbm_app::properties::report(&lbm_display::windows::display_json(
                            &tree, monitor,
                        )),
                    );
                }
                lbm_layout::windows::populate(
                    &mut layout,
                    lbm_display::windows::thread_dpi_awareness(),
                    &tree.layout_input(),
                    |_| Ok::<(), std::convert::Infallible>(()),
                )
                .is_ok()
            }
            Err(error) => {
                eprintln!("[lbm-app] display discovery failed: {error}");
                false
            }
        }
    };

    #[cfg(not(windows))]
    let built = {
        let backend = lbm_display::linux::Backend::detect();
        let outputs = backend
            .map(|backend| {
                backend.query().unwrap_or_else(|error| {
                    eprintln!("[lbm-app] display discovery failed: {error}");
                    Vec::new()
                })
            })
            .unwrap_or_default();
        for output in &outputs {
            let mut report = lbm_display::linux::display_json(output);
            report["Discovery"] = serde_json::json!(backend.map(|b| b.name()));
            reports.insert(
                output.connector_name.clone(),
                lbm_app::properties::report(&report),
            );
        }
        // No loader: nothing is read from the store and nothing is written to it. The
        // window shows the screens as they are, not as a saved profile arranges them.
        lbm_layout::linux::populate(&mut layout, &outputs, |_| {
            Ok::<(), std::convert::Infallible>(())
        })
        .is_ok()
    };

    if !built {
        return None;
    }
    let detected = lbm_app::properties::Detected::of(&layout);
    read_stored(&mut layout);
    let detected = detected.with_loaded(&layout);
    Some((Box::new(layout), reports, detected))
}

/// The stored profile onto the layout in memory: the app-wide options, the layout's own
/// options, and every monitor's saved place and model.
///
/// **Read, never written.** The frontend reads the profile files the agent owns;
/// writing goes back through the agent, which stays the only writer.
///
/// **Not `LayoutPersistence::load`, deliberately.** That is the same read plus the
/// excluded list — and `ExcludedListPersistence::load` **writes `Excluded.txt`** when it
/// does not exist yet, seeding the defaults. Correct for the agent, and a promise broken
/// for a window that creates nothing. The list is not read here at all, which is why
/// [`lbm_app::settings::save_layout`] has to strip it back out of the document.
///
/// **This changes what the map shows**: the screens as the stored profile arranges them,
/// not as the system reports them. That is the arrangement the engine runs, so it is the
/// one an edit should start from — saving a detected arrangement over a user's saved one
/// would lose it.
///
/// A missing or unreadable store leaves the detected arrangement and the defaults, which
/// is what the agent falls back to as well (`read_store`: a store that cannot be read
/// must not keep the app from starting, #589).
fn read_stored(layout: &mut Layout) {
    use lbm_store::layout_dto_mapper as mapper;
    use lbm_store::LayoutStore;

    let store = lbm_store::JsonLayoutStore::new(lbm_store::lbm_paths::config_dir());
    let Ok(data) = store.read(&layout.id, &[]) else {
        eprintln!("[lbm-app] the store could not be read: the detected screens are shown");
        return;
    };
    layout.edit_options(|o| mapper::apply_global_options(o, data.global_options.as_ref()));
    mapper::apply_layout(layout, data.layout.as_ref(), &data.models);
    // The store's own load semantics, kept so this window's layout says what the agent's
    // says about itself. The bar no longer reads this flag — it compares documents
    // (`lbm_app::saved`) — but a layout that lied about having been loaded would be a
    // trap for whatever reads it next.
    layout.mark_saved();
    layout.parse_physical_monitors();
}

/// What the chosen mode writes inside one screen's frame.
///
/// Empty for every mode this window cannot draw — `lbm_ui::mode::why_not` is what says
/// which those are, and the bar will not let one be chosen, so reaching a missing arm
/// here means the two have drifted apart. They are kept in one place by the compiler
/// instead: both match on the same closed enum.
fn details_of(
    layout: &Layout,
    monitor: &lbm_layout::model::Monitor,
    mode: lbm_ui::mode::Mode,
) -> Vec<(&'static str, String)> {
    use lbm_ui::mode::Mode;
    let Some(projection) = layout.depth_projection(monitor) else {
        return Vec::new();
    };
    let model = layout.model(&monitor.model);
    match mode {
        // The frame is the picture; nothing is written over it.
        Mode::Default => Vec::new(),
        // C# `MonitorLocationView`: where it is, how big, and at what pitch.
        Mode::Location => {
            let mm = projection.bounds();
            let source = monitor
                .active_source
                .as_deref()
                .and_then(|id| layout.source(id));
            let pixels = source.map(|s| s.source.in_pixel.bounds());
            let mut rows = vec![
                ("at", format!("{:.0}, {:.0} mm", mm.x(), mm.y())),
                ("size", format!("{:.0} × {:.0} mm", mm.width(), mm.height())),
            ];
            if let Some(p) = pixels {
                rows.push(("pixels", format!("{:.0} × {:.0}", p.width(), p.height())));
                // The pitch the layout actually works in, rather than the one the
                // system advertises: pixels over the millimetres of the lit part.
                if mm.width() > 0.0 && mm.height() > 0.0 {
                    rows.push((
                        "dpi",
                        format!(
                            "{:.0} × {:.0}",
                            p.width() / mm.width() * 25.4,
                            p.height() / mm.height() * 25.4
                        ),
                    ));
                }
            }
            if let Some(model) = model {
                rows.push(("pnp", model.pnp_code.clone()));
            }
            rows
        }
        // These modes use the selected-monitor properties panel instead of map rows.
        Mode::Info | Mode::Size | Mode::Resistance => Vec::new(),
        // Wallpaper has its own image editor, with no text inside monitor frames.
        Mode::Vcp | Mode::Wallpaper => Vec::new(),
    }
}

/// The screens as the views want them, read off the layout.
///
/// Called again after every edit, so what is drawn is what the layout says and never a
/// separate copy of it kept in step by hand.
fn screens_of(layout: &Layout, mode: lbm_ui::mode::Mode) -> Vec<Screen> {
    layout
        .monitors()
        .iter()
        .filter_map(|m| {
            // No active source, no place on the map — the rule the C# presenter applies.
            let projection = layout.depth_projection(m)?;
            let model = layout.model(&m.model);
            Some(Screen {
                id: m.id.clone(),
                source_id: m.active_source.clone().unwrap_or_default(),
                name: model
                    .and_then(|model| model.pnp_device_name.clone())
                    .unwrap_or_else(|| m.id.clone()),
                logo: model
                    .and_then(|model| model.logo.clone())
                    .map(PathBuf::from),
                mm_outside: projection.outside_bounds(),
                mm_content: projection.bounds(),
                pixels: m
                    .active_source
                    .as_deref()
                    .and_then(|id| layout.source(id))
                    .map(|source| source.source.in_pixel.bounds())
                    .unwrap_or_default(),
                details: details_of(layout, m, mode),
            })
        })
        .collect()
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        Self::with_config(ctx, lbm_app::session::Config::for_session())
    }

    fn with_config(ctx: egui::Context, config: Option<lbm_app::session::Config>) -> Self {
        lbm_ui::theme::install(&ctx);
        let icons = icons_root()
            .map(|root| lbm_icons::catalogue(&root).0)
            .unwrap_or_default();
        if icons.is_empty() {
            eprintln!("[lbm-app] no icons found: logos will not be drawn (set LBM_ICONS)");
        }
        let (send, arriving) = std::sync::mpsc::channel();
        let waker = ctx.clone();
        let events = send.clone();
        let without_agent = Some((
            if config.is_some() {
                "connecting to agent"
            } else {
                "this session has no agent endpoint"
            }
            .into(),
            config
                .as_ref()
                .map(|c| c.endpoint.display().to_string())
                .unwrap_or_default(),
        ));
        let session = config.map(|config| {
            lbm_app::session::spawn(config, move |event| {
                let sent = events.send(Found::Session(event)).is_ok();
                waker.request_repaint();
                sent
            })
        });
        let mut refresh = lbm_app::refresh::Refresh::default();
        refresh.invalidate();

        App {
            arriving: Some(arriving),
            found: send,
            _session: session,
            refresh,
            detecting: None,
            retry_display_at: std::time::Instant::now(),
            display_error: None,
            layout: None,
            saved: None,
            screens: Vec::new(),
            reports: HashMap::new(),
            detected: Default::default(),
            agent_keeps_intrinsic_size: false,
            property_error: None,
            resistance: lbm_ui::resistance::Editor::default(),
            screen_overlay: Default::default(),
            screen_input: Default::default(),
            screen_overlay_mode: Default::default(),
            screen_overlay_error: None,
            next_screen_overlay: std::time::Instant::now(),
            selected: None,
            drag: None,
            instance: None,
            replacement_draft: None,
            agent: lbm_app::conversation::Conversation::default(),
            probe: lbm_app::probe::Probe::default(),
            show_probe: true,
            outputs: Default::default(),
            vcp: Default::default(),
            remote: Default::default(),
            test_patterns: Default::default(),
            pattern_strip: Default::default(),
            excluded: None,
            pattern: String::new(),
            placement: Default::default(),
            topology_pending: None,
            recording: lbm_ui::shortcut::Recording::default(),
            previewed: None,
            view: View::Map,
            settings_open: false,
            pending_options: Default::default(),
            about_open: false,
            settings_page: lbm_ui::options::Page::default(),
            mode: lbm_ui::mode::Mode::default(),
            icons,
            textures: HashMap::new(),
            desktop_wallpaper: Default::default(),
            desktop_textures: HashMap::new(),
            wallpapers: HashMap::new(),
            wallpaper: Default::default(),
            wallpaper_preview_requested: false,
            panorama: None,
            panorama_editor: Default::default(),
            wallpaper_picking: false,
            wallpaper_choice_context: None,
            picker_parent: Default::default(),
            wallpaper_preview_busy: None,
            state: lbm_ui::State::default(),
            requests: None,
            next_id: 3,
            without_agent,
        }
    }

    fn dirty(&self) -> bool {
        match (&self.layout, &self.saved) {
            (Some(layout), Some(reference)) => !reference.holds(layout),
            _ => false,
        }
    }

    fn session_event(&mut self, event: lbm_app::session::Event) {
        use lbm_app::session::Event;
        match event {
            Event::Connected {
                requests,
                snapshot,
                capabilities,
            } => {
                self.agent_keeps_intrinsic_size = capabilities.iter().any(|c| c == "IntrinsicSize");
                self.pending_options.disconnected();
                self.placement = Default::default();
                self.topology_pending = None;
                self.outputs = Default::default();
                self.vcp = Default::default();
                self.remote = Default::default();
                self.test_patterns.stop();
                self.probe = lbm_app::probe::Probe::default();
                self.requests = Some(requests);
                self.next_id = 3;
                self.state = lbm_ui::State::default();
                self.previewed = None;
                self.agent = lbm_app::conversation::Conversation::after([(2, "Subscribe")]);
                self.refresh.observe(
                    lbm_app::refresh::Stamp::from_snapshot(&snapshot),
                    true,
                    self.dirty(),
                );
                self.agent_said(Message::Answer {
                    id: 2,
                    result: Ok(snapshot),
                });
                self.send("SeenProcesses", serde_json::json!({}));
                self.send_pending_options();
            }
            Event::Message(message) => self.agent_said(message),
            Event::Disconnected(why) => {
                self.pending_options.disconnected();
                self.wallpaper.disconnect();
                self.wallpaper_preview_busy = None;
                self.screen_overlay_mode = lbm_overlay::Mode::Off;
                self.screen_overlay.clear();
                self.placement = Default::default();
                self.topology_pending = None;
                self.outputs = Default::default();
                self.vcp = Default::default();
                self.remote = Default::default();
                self.test_patterns.stop();
                self.probe = lbm_app::probe::Probe::default();
                self.requests = None;
                self.agent_keeps_intrinsic_size = false;
                self.state = lbm_ui::State::default();
                self.previewed = None;
                // Forget pending acknowledgements, never the user's document.
                self.agent = lbm_app::conversation::Conversation::default();
                self.without_agent = Some((
                    why,
                    client::default_endpoint()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                ));
            }
        }
    }

    fn read_displays(&mut self, ctx: &egui::Context, discard_edits: bool) {
        if self.detecting.is_some() || std::time::Instant::now() < self.retry_display_at {
            return;
        }
        let Some(generation) = self.refresh.begin(self.dirty() && !discard_edits) else {
            return;
        };
        self.detecting = Some(generation);
        let found = self.found.clone();
        let waker = ctx.clone();
        std::thread::spawn(move || {
            let Some((layout, reports, detected)) = detect() else {
                let _ = found.send(Found::DetectionFailed { generation });
                waker.request_repaint();
                return;
            };
            if found
                .send(Found::Displays {
                    generation,
                    layout,
                    reports,
                    detected,
                    excluded: lbm_app::settings::read_excluded(
                        lbm_store::layout_persistence::default_excluded_list_file(),
                    ),
                })
                .is_err()
            {
                return;
            }
            waker.request_repaint();
        });
    }

    /// Asks the agent for what the user pressed.
    ///
    /// **Undo never leaves this process.** Throwing an edit away is re-reading the store,
    /// which this window does for itself; asking the agent to do it would be asking it to
    /// reload a layout it never edited. Save is the only one that travels, because the
    /// agent is the only writer.
    fn ask(&mut self, press: lbm_ui::Press) {
        let request = match press {
            lbm_ui::Press::Start => Some(("Start", serde_json::json!({}))),
            lbm_ui::Press::Stop => Some(("Stop", serde_json::json!({}))),
            // The one press that earns a new reference, and only if the agent takes it.
            lbm_ui::Press::Save => {
                if self.refused_by_old_agent() {
                    return;
                }
                let Some(layout) = self.layout.as_deref() else {
                    return;
                };
                let (method, extra) = lbm_app::settings::save_layout(layout);
                let earned = lbm_app::saved::Reference::of(layout);
                let id = self.send(method, extra);
                // What this save earns if the agent takes it — the document **as sent**.
                self.agent.earning(id, earned);
                return;
            }
            lbm_ui::Press::Undo => {
                self.undo();
                None
            }
            // Turning the switch on sends the layout at once rather than waiting up to
            // `PREVIEW_INTERVAL`: the user pressed something and expects the cursor to
            // follow. `previewed` is cleared so the gate cannot suppress it — the agent
            // is holding the last applied layout, not ours
            // (`LiveLayoutUpdater.Forget`).
            lbm_ui::Press::Live => {
                self.previewed = None;
                None
            }
        };
        let Some((method, extra)) = request else {
            return;
        };
        self.send(method, extra);
    }

    /// Asks the agent to move the real screens to match this layout.
    ///
    /// **Through the agent, though the plan lets the frontend touch topology directly.**
    /// Applying means closing the engine's 1px gaps first, and the gap guard is the
    /// agent's (D7) — which a frontend must not link, because linking the agent is
    /// linking the code that can start a hook. Two processes writing the topology would
    /// fight over it; one of them already knows whether the engine is running.
    fn apply_topology(&mut self) {
        if self.refused_by_old_agent() {
            return;
        }
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let (method, mut extra) =
            lbm_app::settings::apply_topology(layout, self.placement.adjust_scale);
        extra["LayoutRevision"] =
            serde_json::json!(self.agent.layout.as_ref().and_then(|s| s.revision));
        let earned = lbm_app::saved::Reference::of(layout);
        let id = self.send(method, extra);
        self.agent.earning(id, earned);
        self.topology_pending = Some(id);
    }

    /// The command lines an apply ran, or would run.
    ///
    /// A window of its own rather than a line in the bar: this is a list to read, it can
    /// be a dozen lines long, and the thing it is about — the user's screens moving — is
    /// the one action here that cannot be undone.
    fn topology_report(&mut self, ctx: &egui::Context) {
        let Some(ran) = self.agent.topology.clone() else {
            return;
        };
        let title = if ran.dry {
            "What applying would run"
        } else {
            "What applying ran"
        };
        let mut open = true;
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                if ran.commands.is_empty() {
                    ui.label("Nothing: the screens are already where this layout puts them.");
                    return;
                }
                if ran.dry {
                    ui.label(
                        egui::RichText::new(
                            if cfg!(windows) {
                                "No screens have moved and nothing has been saved. The proposed \
                                 positions have been checked. Applying checks the displays again, saves \
                                 this layout, then moves the screens without changing their scales."
                            } else {
                                "Nothing has been run and nothing has been saved. The positions \
                                 below are a prediction: the real pass re-reads the compositor \
                                 after the scales are applied, and re-asserts whatever drifts — \
                                 neither can be known without changing something."
                            },
                        )
                        .small()
                        .color(ui.visuals().weak_text_color()),
                    );
                    ui.separator();
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (n, line) in ran.commands.iter().enumerate() {
                        ui.label(egui::RichText::new(format!("{}. {line}", n + 1)).monospace());
                    }
                });
            });
        if !open {
            self.agent.topology = None;
        }
    }

    /// Asks what an apply *would* run, and changes nothing.
    ///
    /// Worth its own entry rather than a flag, because this is the one action in the
    /// window that cannot be undone: a user is entitled to read the commands before the
    /// screens move, and the first person ever to press Apply for real deserves it most.
    fn dry_run_topology(&mut self) {
        // A dry run still has the agent read the document, and on Linux apply it to the
        // model it holds: an old agent would simulate (and keep) a swapped size.
        if self.refused_by_old_agent() {
            return;
        }
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        self.agent.topology = None;
        let (method, mut extra) =
            lbm_app::settings::dry_run_topology(layout, self.placement.adjust_scale);
        extra["LayoutRevision"] =
            serde_json::json!(self.agent.layout.as_ref().and_then(|s| s.revision));
        self.topology_pending = Some(self.send(method, extra));
    }

    /// Stops previewing: the agent goes back to the layout it had.
    fn end_preview(&mut self) {
        self.previewed = None;
        let (method, extra) = lbm_app::settings::end_preview();
        self.send(method, extra);
    }

    /// One tick of the live preview, if it is on and anything the agent can see has moved.
    ///
    /// Called every frame; the interval is a **rate limit**, not a clock — egui redraws
    /// for its own reasons and this must not turn a repaint into a layout swap.
    fn preview_tick(&mut self, ctx: &egui::Context) {
        if !self.state.live || self.topology_pending.is_some() {
            return;
        }
        // Without this the window only redraws when something happens, so an edit made
        // by a key repeat or by the agent would wait for the next stray repaint.
        ctx.request_repaint_after(lbm_app::settings::PREVIEW_INTERVAL);

        if self.refused_by_old_agent() {
            return;
        }
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        if let Some((_, when)) = &self.previewed {
            if when.elapsed() < lbm_app::settings::PREVIEW_INTERVAL {
                return;
            }
        }
        let (method, extra) = lbm_app::settings::preview(layout);
        let document = extra.to_string();
        if self
            .previewed
            .as_ref()
            .is_some_and(|(sent, _)| *sent == document)
        {
            // Something moved, but nothing the agent can see. Take the time so the
            // document is not rebuilt until the interval is up again.
            self.previewed = Some((document, std::time::Instant::now()));
            return;
        }
        self.send(method, extra);
        self.previewed = Some((document, std::time::Instant::now()));
    }

    /// Whether the layout's document must not go to the connected agent: one that
    /// predates `IntrinsicSize` would transpose a size crossing the detected shape, and
    /// save or run that, whatever edit sent it. Says why, and keeps the reason up.
    fn refused_by_old_agent(&mut self) -> bool {
        if self.requests.is_none() || self.agent_keeps_intrinsic_size {
            return false;
        }
        let Some(layout) = self.layout.as_deref() else {
            return false;
        };
        if lbm_app::properties::swapped_by_old_agent(layout, &self.detected).is_none() {
            return false;
        }
        self.property_error = Some(lbm_app::properties::OLD_AGENT_WOULD_SWAP.into());
        true
    }

    /// Puts a request on the writer thread, and says so if there is nobody to take it.
    fn send(&mut self, method: &'static str, extra: serde_json::Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        // Recorded **before** the request leaves, on this thread: an answer that arrives
        // the instant after cannot find the pairing missing.
        self.agent.asking(id, method);
        if let Some(requests) = &self.requests {
            if requests.send((id, method, extra)).is_err() {
                self.requests = None;
                self.without_agent =
                    Some(("the agent stopped listening".to_owned(), String::new()));
            }
        }
        id
    }

    fn restore_replacement_draft(&mut self, layout: &mut Layout) {
        let Some(draft) = self.replacement_draft.take() else {
            return;
        };
        if draft["LayoutId"].as_str() != Some(layout.id.as_str()) {
            return;
        }
        if let Ok(document) =
            serde_json::from_value::<lbm_store::LayoutDocument>(draft["Document"].clone())
        {
            document.apply(layout);
            self.selected = draft["Selected"].as_str().map(str::to_owned);
            self.mode = lbm_ui::mode::Mode::ALL
                .into_iter()
                .find(|m| Some(m.id()) == draft["Mode"].as_str())
                .unwrap_or_default();
        }
    }

    /// Requests from other launches. Run from `logic`, which eframe also calls while the
    /// window is minimized: activating it must not wait for it to be visible again.
    fn instance_requests(&mut self, ctx: &egui::Context) {
        if let Some(instance) = &self.instance {
            for event in instance.pending() {
                use lbm_app::instance::Event;
                match event {
                    Event::Activate(reply) => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        #[cfg(target_os = "linux")]
                        std::thread::spawn(|| {
                            if let Err(error) = lbm_app::patterns::editor::activate() {
                                eprintln!("[lbm-app] KDE activation: {error}");
                            }
                        });
                        let _ = reply.send(serde_json::Value::Null);
                    }
                    Event::Replace(reply) => {
                        // Carry the local layout draft without saving it over the user's
                        // profile. The replacement takes its saved reference from disk.
                        let draft = self.layout.as_ref().map(|layout| {
                            serde_json::json!({
                                "LayoutId": layout.id,
                                "Document": lbm_app::settings::document(layout),
                                "Selected": self.selected,
                                "Mode": self.mode.id(),
                            })
                        });
                        let _ = reply.send(draft.unwrap_or(serde_json::Value::Null));
                    }
                    Event::Close => {
                        self.test_patterns.stop();
                        self.screen_overlay.clear();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    }
                }
            }
        }
    }

    /// Throws the edits away: the stored profile again, over the layout as detected.
    ///
    /// Start with the last loaded/accepted state, including first-run defaults, then
    /// overlay the current store. Reading an absent file alone would keep the edit.
    fn undo(&mut self) {
        let Some(layout) = self.layout.as_mut() else {
            return;
        };
        if let Some(reference) = &self.saved {
            reference.restore(layout);
        }
        read_stored(layout);
        self.pending_options.apply(&mut layout.options);
        self.saved = Some(lbm_app::saved::Reference::of(layout));
        self.screens = screens_of(layout, self.mode);
        self.drag = None;
        self.property_error = None;
        self.resistance.clear();
    }

    /// Applies what the user did to the excluded list. `true` when it changed, which is
    /// the cue to save.
    ///
    /// The duplicate rules are the store's, not this window's: a typed pattern is
    /// compared exactly (a user is entitled to two spellings of a path), while the
    /// defaults top-up is **separator-insensitive** — `ExcludedProcessDefaults.ContainsEntry`,
    /// the daemon's own matching — so a list seeded with the Windows-style entries is not
    /// doubled with their Linux twins.
    fn excluded_edit(&mut self, did: lbm_ui::excluded::Did) -> bool {
        use lbm_store::excluded_process_defaults as defaults;
        let Some(list) = self.excluded.as_mut() else {
            return false;
        };
        match did {
            lbm_ui::excluded::Did::Add(pattern) => {
                if !lbm_ui::excluded::can_add(list, &pattern) {
                    return false;
                }
                list.push(pattern);
                self.pattern.clear();
                true
            }
            lbm_ui::excluded::Did::Remove(pattern) => {
                let before = list.len();
                list.retain(|e| *e != pattern);
                before != list.len()
            }
            lbm_ui::excluded::Did::AddDefaults => {
                let mut added = false;
                for entry in defaults::ALL {
                    if !defaults::contains_entry(list.iter(), entry) {
                        list.push((*entry).to_owned());
                        added = true;
                    }
                }
                added
            }
        }
    }

    /// Sends the app-wide options to the agent, which is the only thing that writes them.
    ///
    /// The request itself is [`lbm_app::settings::save_options`], where a test can check
    /// the very frame this sends.
    fn save_options(&mut self, before: &LayoutOptions, excluded_changed: bool) {
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        // A new shortcut makes the hook's old verdict meaningless: it was about the one
        // before. Cleared here rather than in the panel, because this is the moment the
        // change actually leaves — the hook will say again if the new one is no better.
        self.agent.shortcut_changed();
        let (method, extra) = lbm_app::settings::save_options(
            before,
            &layout.options,
            if excluded_changed {
                self.excluded.as_deref()
            } else {
                None
            },
        );
        debug_assert_eq!(method, "SaveOptions");
        self.pending_options.add(extra);
        self.send_pending_options();
    }

    fn send_pending_options(&mut self) {
        if self.requests.is_none() {
            return;
        }
        if let Some((method, extra)) = self.pending_options.request() {
            let id = self.send(method, extra);
            self.pending_options.sent(id);
        }
    }

    /// What the agent said, in the bar's terms.
    ///
    /// **`saved` is still not taken from the agent, and that is still deliberate** —
    /// for a different reason now. The agent's `Snapshot.saved` is about *its* layout;
    /// this window edits its own copy, and what Save and Undo are for is the difference
    /// between that copy and the store. So the flag comes from the layout here, in
    /// [`App::ui`], and the agent's is ignored rather than fought with.
    /// What the agent said, and what the window must do about it.
    ///
    /// The deciding is [`lbm_app::conversation`]'s, which has no window and no socket and
    /// can therefore be tested; what is left here is the two things that need both.
    fn agent_said(&mut self, message: Message) {
        if let Message::Answer { id, result } = &message {
            self.pending_options.answered(*id);
            if self.topology_pending == Some(*id) {
                self.topology_pending = None;
            }
            if self.remote.answered(*id, result) {
                return;
            }
            if self.vcp.answered(*id, result) {
                return;
            }
            if self.wallpaper.answered(*id, result) {
                self.wallpaper_preview_requested = self.wallpaper.error.is_none();
                return;
            }
            if let Some(changed) = self.outputs.answered(*id, result) {
                if changed {
                    self.refresh.invalidate();
                }
                return;
            }
            if self.probe.answered(*id, result) {
                return;
            }
        }
        for effect in self.agent.said(message, &mut self.state) {
            match effect {
                lbm_ui::Effect::Ask(press) => self.ask(press),
                lbm_ui::Effect::EndPreview => self.end_preview(),
            }
        }
        if let Some(stamp) = self.agent.layout.clone() {
            if let (Some(layout), Some(revision)) = (&stamp.layout_id, stamp.revision) {
                self.wallpaper.observes(&lbm_app::wallpaper_editor::Key {
                    layout: layout.clone(),
                    revision,
                });
            }
            if self.refresh.observe(stamp, false, self.dirty()) {
                self.outputs.invalidate();
                self.vcp.select(None);
                self.remote.select(None);
                self.state.live = false;
                self.previewed = None;
                self.drag = None;
            }
        }
        // A save that landed: what it sent is now what is on disk.
        if let Some(earned) = self.agent.earned() {
            self.saved = Some(earned);
        }
        if self.agent.heard {
            self.without_agent = None;
        }
    }

    /// The logo for one screen, uploaded once and kept.
    ///
    /// Rendered at a fixed size rather than at the band's: a texture re-uploaded every
    /// time the window is resized would be a decode and an upload per frame during a
    /// drag. The frame scales it down, which is what `Stretch="Uniform"` does anyway.
    fn logo(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if let Some(ready) = self.textures.get(path) {
            return Some(ready.clone());
        }
        let file = lbm_icons::resolve(&self.icons, path)?;
        let source = std::fs::read(file).ok()?;
        let colour = frame::logo_colour(&ctx.style_of(ctx.theme()).visuals);
        let tree = lbm_icons::load(
            &source,
            Rgba(colour.r(), colour.g(), colour.b(), colour.a()),
        )
        .ok()?;
        let logo = lbm_icons::render_logo(&tree, 512)?;
        let image = egui::ColorImage::from_rgba_premultiplied(logo.size, &logo.pixels);
        let handle = ctx.load_texture(path, image, egui::TextureOptions::LINEAR);
        self.textures.insert(path.to_owned(), handle.clone());
        Some(handle)
    }

    /// The screen in hand, if there is one: which of `monitors` it is, how far it has
    /// really gone in millimetres, and what it lines up with on the way.
    ///
    /// One function for both because the two must not disagree: the lines say why the
    /// screen is where it is, so they have to come from the same call that put it there.
    ///
    /// `anchors` is the Ctrl key, inverted — see [`drag::snap`]. It is read on every
    /// frame of the gesture rather than at the press, because that is when the user
    /// reaches for it: they drag, watch it jump flush, and then hold Ctrl to say no.
    fn held(
        &self,
        monitors: &[MapMonitor<'_>],
        fit: &map::Fit,
        anchors: bool,
    ) -> Option<(usize, (f64, f64), drag::Snap)> {
        if self.mode != lbm_ui::mode::Mode::Location {
            return None;
        }
        let drag = self.drag.as_ref()?;
        let i = monitors.iter().position(|m| m.id == drag.id)?;
        // Points back to millimetres, `FrameMover.cs:172`. The ratio should not be zero
        // here — a screen was drawn, so it was drawn at some scale — but dividing by it
        // is not a thing to do on a should.
        if fit.ratio <= 0.0 || !fit.ratio.is_finite() {
            return None;
        }
        let free = (drag.by.x as f64 / fit.ratio, drag.by.y as f64 / fit.ratio);
        let others: Vec<drag::Screen> = monitors
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, m)| drag::Screen::from(m))
            .collect();
        let snap = drag::snap(drag::Screen::from(&monitors[i]), free, &others, anchors);
        Some((i, (free.0 + snap.offset.0, free.1 + snap.offset.1), snap))
    }

    /// What a click, a move or a drop means.
    ///
    /// A **drop is the only thing here that touches the layout**, which is the C#'s shape
    /// too: the mover carries its own position while the gesture lasts and writes it into
    /// the model in `EndMove`. Nothing leaves this process either way — the edit lives in
    /// the window's copy of the layout until there is a Save to send it anywhere.
    fn acted_on_the_map(
        &mut self,
        did: Option<Did>,
        total: Option<(f64, f64)>,
        ctx: &egui::Context,
    ) {
        // A mode change cancels a move, including a late release or lost-pointer drop.
        if self.mode != lbm_ui::mode::Mode::Location {
            self.drag = None;
            if !matches!(did, Some(Did::Clicked(_))) {
                return;
            }
        }
        match did {
            Some(Did::Clicked(id)) => {
                if self.selected.as_ref() != Some(&id) {
                    self.property_error = None;
                }
                self.resistance.select_monitor(&id);
                self.selected = Some(id);
                self.drag = None;
            }
            Some(Did::Dragged { id, by }) => self.drag = Some(Drag { id, by }),
            Some(Did::Dropped { id }) => self.drop_it(&id, total),
            // A drag that stops being reported without a drop — the window losing the
            // pointer mid-gesture — is a drop, not a screen left hanging.
            // `MonitorLocationView.axaml.cs:177-182` ends the move the same way, on the
            // button no longer being down rather than on a release it may never see.
            None => {
                if self.drag.is_some() && !ctx.input(|i| i.pointer.any_down()) {
                    let id = self.drag.as_ref().map(|d| d.id.clone()).unwrap_or_default();
                    self.drop_it(&id, total);
                }
            }
        }
    }

    /// Writes the drop into the layout and reads the screens back out of it.
    fn drop_it(&mut self, id: &str, total: Option<(f64, f64)>) {
        self.drag = None;
        let (Some(layout), Some(by)) = (self.layout.as_mut(), total) else {
            return;
        };
        drag::drop_screen(layout, id, by);
        // Not "move that one screen": compaction can shift any of them, so what is drawn
        // next is read off the layout whole.
        self.screens = screens_of(layout, self.mode);
    }

    /// Which modes this window can offer right now.
    ///
    /// `vcp_enabled` is the user's setting, read off the layout the window holds — the
    /// same `VcpControl` the C# button follows. The wallpaper editor also offers local
    /// previews on unsupported desktops; only Apply needs the agent’s native service.
    fn offered(&self) -> lbm_ui::mode::Offered {
        lbm_ui::mode::Offered {
            vcp_enabled: self
                .layout
                .as_deref()
                .is_some_and(|l| l.options.vcp_control),
            wallpaper_supported: self.wallpaper.key.is_some(),
        }
    }

    /// The screens as the views want them, logos resolved.
    fn monitors<'a>(
        &'a self,
        logos: &'a HashMap<String, egui::TextureHandle>,
    ) -> Vec<MapMonitor<'a>> {
        self.screens
            .iter()
            .map(|s| MapMonitor {
                id: &s.id,
                name: &s.name,
                mm_outside: s.mm_outside,
                mm_content: s.mm_content,
                wallpaper: (self.mode == lbm_ui::mode::Mode::Wallpaper)
                    .then(|| self.wallpapers.get(&s.id))
                    .flatten()
                    .or_else(|| self.desktop_textures.get(&s.id)),
                logo: s
                    .logo
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .and_then(|p| logos.get(p)),
                details: if self.mode == lbm_ui::mode::Mode::Location {
                    &[]
                } else {
                    &s.details
                },
                probe: if matches!(
                    self.mode,
                    lbm_ui::mode::Mode::Resistance
                        | lbm_ui::mode::Mode::Info
                        | lbm_ui::mode::Mode::Location
                ) {
                    None
                } else {
                    self.probe.overlay(&s.source_id, s.pixels)
                },
            })
            .collect()
    }

    fn probe_tick(&mut self, ctx: &egui::Context) {
        let now = std::time::Instant::now();
        let key = if self.show_probe
            && self.mode != lbm_ui::mode::Mode::Resistance
            && matches!(self.view, View::Map | View::List)
            && self.requests.is_some()
            && !self.refresh.pending()
            && self.drag.is_none()
        {
            self.layout
                .as_ref()
                .zip(self.agent.layout.as_ref().and_then(|s| s.revision))
                .map(|(layout, revision)| lbm_app::probe::Key::of(layout, revision))
        } else {
            None
        };
        self.probe.want(key, now);
        if let Some(extra) = self.probe.ready(now) {
            if let Some(requests) = &self.requests {
                let id = self.next_id;
                self.next_id += 1;
                // This answer belongs to the report, never to a pending Save/Start.
                self.probe.sent(id);
                if requests.send((id, "ProbeLayout", extra)).is_err() {
                    self.probe
                        .answered(id, &Err("the agent stopped listening".into()));
                }
            }
        }
        if self.probe.waiting() {
            ctx.request_repaint_after(lbm_app::probe::DEBOUNCE);
        }
    }

    fn output_blocked(&self) -> Option<&'static str> {
        if self.requests.is_none() {
            Some("Connect to the agent to change displays.")
        } else if self.refresh.pending() {
            Some("Wait for the screen configuration to reload.")
        } else if self.dirty() {
            Some("Save or undo your edits before changing system displays.")
        } else if self.state.is_virtual || self.layout.as_ref().is_some_and(|l| l.is_virtual()) {
            Some("A foreign layout cannot change system displays.")
        } else if self.agent.enabled != Some(false)
            || self.agent.previewing
            || self.state.live
            || self.agent.suspended
            || matches!(
                self.state.engine,
                lbm_ui::Engine::Running | lbm_ui::Engine::Paused
            )
        {
            Some("Stop the engine and end the preview before changing displays.")
        } else if self.layout.is_none()
            || self
                .agent
                .layout
                .as_ref()
                .and_then(|s| s.revision)
                .is_none()
        {
            Some("A current layout and an up-to-date agent are required.")
        } else if self.state.waiting {
            Some("Wait for the current operation to finish.")
        } else {
            None
        }
    }

    fn wallpaper_key(&self) -> Option<lbm_app::wallpaper_editor::Key> {
        let layout = self.layout.as_ref()?;
        let stamp = self.agent.layout.as_ref()?;
        if stamp.layout_id.as_deref() != Some(&layout.id) || self.refresh.pending() {
            return None;
        }
        Some(lbm_app::wallpaper_editor::Key {
            layout: layout.id.clone(),
            revision: stamp.revision?,
        })
    }
    fn wallpaper_request(&mut self, read: bool) {
        let Some(requests) = &self.requests else {
            return;
        };
        if self.wallpaper.busy() {
            return;
        }
        let id = self.next_id;
        let (method, extra) = if read {
            let Some(key) = self.wallpaper_key() else {
                return;
            };
            ("WallpaperSettings", self.wallpaper.read(id, key))
        } else {
            let Some(extra) = self.wallpaper.apply(id) else {
                return;
            };
            ("ApplyWallpaper", extra)
        };
        self.next_id += 1;
        if requests.send((id, method, extra)).is_err() {
            self.wallpaper
                .answered(id, &Err("The agent stopped listening.".into()));
        }
    }
    fn sync_wallpaper(&mut self, ctx: &egui::Context) {
        if let Some(layout) = &self.layout {
            if let Some(pictures) = self.desktop_wallpaper.poll(layout) {
                self.desktop_textures.clear();
                for (id, pixels) in pictures {
                    let size = [pixels.width() as usize, pixels.height() as usize];
                    self.desktop_textures.insert(
                        id.clone(),
                        ctx.load_texture(
                            format!("desktop-wallpaper:{id}"),
                            egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
                            egui::TextureOptions::LINEAR,
                        ),
                    );
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if let Some(key) = self.wallpaper_key() {
            self.wallpaper.observes(&key);
            if self.requests.is_some()
                && !self.wallpaper.busy()
                && (self.wallpaper.key.is_none()
                    || (self.wallpaper.stale && !self.wallpaper.dirty()))
            {
                self.wallpaper_request(true);
            }
        } else if self.wallpaper.key.is_some() && !self.wallpaper.stale {
            self.wallpaper.disconnect();
        }
        if self.wallpaper_preview_requested && self.wallpaper_preview_busy.is_none() {
            self.wallpaper_preview_requested = false;
            self.preview_wallpaper(ctx);
        }
    }
    fn preview_wallpaper(&mut self, ctx: &egui::Context) {
        if self.wallpaper_preview_busy.is_some() {
            return;
        }
        let (Some(layout), Some(settings)) = (self.layout.clone(), self.wallpaper.draft.clone())
        else {
            return;
        };
        self.wallpaper.preview_generation += 1;
        let generation = self.wallpaper.preview_generation;
        self.wallpaper_preview_busy = Some(generation);
        self.wallpaper.error = None;
        let found = self.found.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let document = Box::new(lbm_store::LayoutDocument::of(&layout));
            let result = (|| {
                if settings.mode == lbm_store::wallpaper_settings::WallpaperMode::Span {
                    let path = settings
                        .span_image_path
                        .as_deref()
                        .filter(|p| !p.is_empty())
                        .ok_or("Choose a panorama image.")?;
                    let pixels = lbm_wallpaper::render::decode(std::path::Path::new(path))?;
                    let size =
                        lbm_layout::geo::Size::new(pixels.width().into(), pixels.height().into());
                    let pixels = image::DynamicImage::ImageRgba8(pixels)
                        .thumbnail(2048, 2048)
                        .into_rgba8();
                    Ok(WallpaperPreview {
                        screens: Vec::new(),
                        panorama: Some((path.to_owned(), size, pixels)),
                    })
                } else {
                    let screens = if settings.per_screen.is_empty() {
                        Vec::new()
                    } else {
                        lbm_wallpaper::render::paint(&layout, &settings, true)?
                    };
                    Ok(WallpaperPreview {
                        screens,
                        panorama: None,
                    })
                }
            })();
            let _ = found.send(Found::WallpaperPreview {
                generation,
                document,
                result,
            });
            ctx.request_repaint();
        });
    }
    fn browse_wallpaper(&mut self, ctx: &egui::Context) {
        if self.wallpaper_picking {
            return;
        }
        let Some(choice) = self.begin_file_choice() else {
            return;
        };
        let found = self.found.clone();
        let ctx = ctx.clone();
        let parent = self.picker_parent.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("lbm-image-picker".into())
            .spawn(move || {
                // The worker owns its request; an unwind does not resume a mutated
                // editor. Native parent resources are released by their guards.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    lbm_app::image_picker::choose(parent, choice.current_path())
                }))
                .unwrap_or_else(|_| {
                    Err("The image chooser stopped unexpectedly. You can paste a path or drop an image.".into())
                });
                let _ = found.send(Found::WallpaperFile {
                    choice: Box::new(choice),
                    result,
                });
                ctx.request_repaint();
            })
        {
            self.wallpaper_picking = false;
            self.wallpaper_choice_context = None;
            self.wallpaper.error = Some(format!("Could not open the image chooser: {e}"));
        }
    }
    fn wallpaper_choice_here(&self) -> (View, bool, lbm_ui::mode::Mode, Option<String>) {
        // The settings drawer covers the editor: opening it leaves the choice's context.
        (
            self.view,
            self.settings_open,
            self.mode,
            self.selected.clone(),
        )
    }
    fn begin_file_choice(&mut self) -> Option<lbm_app::wallpaper_editor::FileChoice> {
        let choice = self.wallpaper.choose_file(self.selected.as_deref())?;
        self.wallpaper_picking = true;
        self.wallpaper_choice_context = Some(self.wallpaper_choice_here());
        Some(choice)
    }
    /// The portal dialog is not modal: the user can leave Wallpaper or pick another
    /// screen and come back before answering. Run on every pass, before answers land.
    fn retire_departed_file_choice(&mut self) {
        if self
            .wallpaper_choice_context
            .as_ref()
            .is_some_and(|at| *at != self.wallpaper_choice_here())
        {
            self.wallpaper_choice_context = None;
            self.wallpaper.abandon_file_choice();
        }
    }
    fn wallpaper_file_chosen(
        &mut self,
        choice: lbm_app::wallpaper_editor::FileChoice,
        result: lbm_app::image_picker::Selection,
    ) {
        self.retire_departed_file_choice();
        self.wallpaper_picking = false;
        self.wallpaper_choice_context = None;
        if !self.settings_open
            && matches!(self.view, View::Map | View::List)
            && self.mode == lbm_ui::mode::Mode::Wallpaper
            && self
                .wallpaper
                .file_chosen(choice, self.selected.as_deref(), result)
        {
            self.wallpaper_preview_requested = true;
        }
    }

    fn wallpaper_panel(&mut self, ui: &mut egui::Ui) {
        if !matches!(self.view, View::Map | View::List)
            || self.mode != lbm_ui::mode::Mode::Wallpaper
        {
            return;
        }
        let stable =
            self.wallpaper_key().is_some() && self.requests.is_some() && !self.wallpaper.stale;
        let can_apply = stable
            && self.wallpaper.supported
            && !self.wallpaper.busy()
            && !self.dirty()
            && !self.state.live
            && !self.agent.previewing
            && !self.agent.suspended
            && !(self.outputs.changing() || self.topology_pending.is_some());
        lbm_ui::properties::panel(ui, |ui| {
            if self.settings_open {
                ui.disable();
            }
            egui::ScrollArea::vertical().show(ui,|ui|{
            lbm_ui::mode::heading(ui, lbm_ui::mode::Mode::Wallpaper);
            self.screen_selector(ui);
            if !stable {
                ui.label("The connection or displays changed. Your wallpaper edits are kept; reload before applying.");
            }
            if !self.wallpaper.supported {
                ui.label("Desktop application is unavailable. You can still preview locally.");
            }
            if self.dirty() || self.state.live || self.agent.previewing {
                ui.label("Save the physical layout and end Live before applying wallpaper.");
            }
            if self.wallpaper.busy() {
                ui.label("Waiting for the agent…");
            }
            if self.wallpaper_picking {ui.label("Choose an image in the file chooser, or cancel there.");}
            if self.wallpaper_preview_busy.is_some() {
                ui.label("Preparing preview…");
            }
            if let Some(e) = &self.wallpaper.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            if let Some(status) = &self.wallpaper.status {
                ui.label(status);
            }
            if ui
                .add_enabled(
                    !self.wallpaper.busy()
                        && self.requests.is_some()
                        && self.wallpaper_key().is_some(),
                    egui::Button::new("Reload saved wallpaper (discard edits)"),
                )
                .clicked()
            {
                self.wallpaper_request(true);
            }
            let mut action = None;
            let editable = !self.wallpaper.busy();
            if let Some(settings) = &mut self.wallpaper.draft {
                let before = settings.clone();
                ui.add_enabled_ui(editable, |ui| {
                    action = lbm_ui::wallpaper_editor::panel(
                        ui,
                        settings,
                        self.selected.as_deref(),
                        can_apply,
                        !self.wallpaper_picking,
                    );
                });
                if *settings != before {
                    self.wallpaper.preview_generation += 1;
                    if settings.span_image_path != before.span_image_path || settings.mode != before.mode {
                        self.panorama_editor = Default::default();
                        self.wallpaper_preview_requested = true;
                    }
                    self.wallpaper.status =
                        Some("Wallpaper edits are not applied to the desktop.".into());
                }
            }
            match action {
                Some(lbm_ui::wallpaper_editor::Action::Browse)=>self.browse_wallpaper(ui.ctx()),
                Some(lbm_ui::wallpaper_editor::Action::Preview) => {
                    self.wallpaper_preview_requested = true
                }
                Some(lbm_ui::wallpaper_editor::Action::Apply) => self.wallpaper_request(false),
                Some(lbm_ui::wallpaper_editor::Action::Reset) => {
                    self.panorama_editor = Default::default();
                    self.wallpaper.reset();
                    self.wallpaper_preview_requested = true;
                }
                None => {}
            }
            });
        });
        if self.wallpaper_preview_requested {
            ui.ctx().request_repaint();
        }
    }

    fn inline_properties<'a>(
        &self,
        ui: &mut egui::Ui,
        placements: impl Iterator<Item = (&'a str, lbm_ui::frame::Drawn)>,
        edits: &mut Vec<(String, lbm_ui::properties::Edit)>,
    ) -> Option<&'a str> {
        use lbm_ui::mode::Mode;
        if !matches!(self.mode, Mode::Info | Mode::Location | Mode::Size) {
            return None;
        }
        let layout = self.layout.as_deref()?;
        let enabled = !layout.is_virtual()
            && !self.refresh.pending()
            && !(self.outputs.changing() || self.topology_pending.is_some())
            && self.drag.is_none()
            && !self.screen_input.active();
        let mut picked = None;
        for (id, drawn) in placements {
            if matches!(self.mode, Mode::Info | Mode::Location) {
                if let Some(info) = lbm_app::properties::monitor_info(layout, id) {
                    let presentation = if self.mode == Mode::Location {
                        lbm_ui::monitor_info::Presentation::Location
                    } else {
                        lbm_ui::monitor_info::Presentation::Info
                    };
                    lbm_ui::monitor_info::draw(ui, drawn.content, &info, presentation);
                }
                if let Some(screen) = self.screens.iter().find(|s| s.id == id) {
                    if let Some(probe) = self.probe.overlay(&screen.source_id, screen.pixels) {
                        probe.draw(ui, drawn.content);
                    }
                }
            }
            let interaction = ui
                .add_enabled_ui(enabled, |ui| match self.mode {
                    Mode::Location => lbm_app::properties::location(layout, id)
                        .map(|value| lbm_ui::inline::location(ui, id, drawn.content, &value)),
                    Mode::Size => lbm_app::properties::size(layout, id)
                        .map(|value| lbm_ui::inline::size(ui, id, &drawn, &value)),
                    _ => None,
                })
                .inner;
            if let Some(interaction) = interaction {
                if interaction.engaged {
                    picked = Some(id);
                }
                edits.extend(
                    interaction
                        .edits
                        .into_iter()
                        .map(|edit| (id.to_owned(), edit)),
                );
            }
        }
        picked
    }

    fn screen_selector(&mut self, ui: &mut egui::Ui) {
        let previous = self.selected.clone();
        let mut single = self.view == View::List;
        let monitors = self
            .layout
            .as_ref()
            .map(|layout| {
                layout
                    .monitors()
                    .iter()
                    .map(|monitor| {
                        let name = layout
                            .model(&monitor.model)
                            .and_then(|model| model.pnp_device_name.as_deref())
                            .unwrap_or(&monitor.id);
                        (monitor.id.as_str(), name)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        lbm_ui::properties::monitor_selector(ui, &mut self.selected, &monitors, &mut single);
        if single != (self.view == View::List) {
            self.view = if single { View::List } else { View::Map };
            self.drag = None;
            self.resistance.cancel();
            self.screen_overlay_mode = lbm_overlay::Mode::Off;
            self.screen_overlay.clear();
        }
        if previous != self.selected {
            self.outputs.panel.confirm = None;
            self.resistance.clear();
            self.property_error = None;
        }
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        if self.mode == lbm_ui::mode::Mode::Wallpaper {
            return;
        }
        let location = self.mode == lbm_ui::mode::Mode::Location;
        if location && self.requests.is_some() && self.outputs.needs_read() {
            self.output_request("DisplayOutputs", serde_json::json!({}), false);
        }
        lbm_ui::properties::panel(ui, |ui| {
            if self.settings_open {
                ui.disable();
            }
            lbm_ui::mode::heading(ui, self.mode);
            if self.mode == lbm_ui::mode::Mode::Resistance {
                if let Some(layout) = self.layout.as_mut() {
                    let mut defaults = layout.options.resistance_defaults;
                    let enabled = !layout.is_virtual()
                        && !self.refresh.pending()
                        && !(self.outputs.changing() || self.topology_pending.is_some())
                        && !self.resistance.active();
                    if ui
                        .add_enabled_ui(enabled, |ui| {
                            lbm_ui::resistance::defaults(ui, &mut defaults)
                        })
                        .inner
                    {
                        layout.edit_options(|o| o.resistance_defaults = defaults);
                        self.probe.want(None, std::time::Instant::now());
                        self.next_screen_overlay = std::time::Instant::now();
                    }
                    self.resistance.defaults = defaults;
                }
            }
            if location {
                self.placement_properties(ui);
                ui.separator();
            }
            self.screen_selector(ui);
            self.screen_overlay_controls(ui);
            egui::ScrollArea::vertical()
                .id_salt(("monitor-properties-scroll", self.mode.id()))
                .show(ui, |ui| {
                    if location {
                        self.crossing_controls(ui);
                        ui.separator();
                    }
                    self.monitor_properties(ui);
                    // Inside the scrolled part and after the monitor's own controls, so a
                    // short window still reaches brightness and contrast.
                    if self.mode == lbm_ui::mode::Mode::Vcp {
                        ui.separator();
                        self.test_pattern_controls(ui);
                    }
                    if location {
                        self.system_display_properties(ui);
                    }
                });
        });
    }

    fn monitor_properties(&mut self, ui: &mut egui::Ui) {
        if self.mode == lbm_ui::mode::Mode::Vcp {
            self.vcp_properties(ui);
            return;
        }
        let Some(layout) = &self.layout else {
            ui.label("Waiting for displays…");
            return;
        };
        if !matches!(
            self.mode,
            lbm_ui::mode::Mode::Size
                | lbm_ui::mode::Mode::Location
                | lbm_ui::mode::Mode::Info
                | lbm_ui::mode::Mode::Resistance
        ) {
            return;
        }
        let Some(id) = self.selected.clone() else {
            ui.label("Select a screen on the map, in the list or above.");
            return;
        };
        let blocked = self.refresh.pending()
            || (self.outputs.changing() || self.topology_pending.is_some())
            || self.drag.is_some();
        let mut edits = Vec::new();
        let mut section_edit = None;
        ui.push_id(("properties", &id), |ui| {
            ui.vertical(|ui| {
                if self.mode == lbm_ui::mode::Mode::Resistance {
                    let monitors = lbm_app::resistance::monitors(layout);
                    section_edit = ui
                        .add_enabled_ui(
                            !blocked && !layout.is_virtual() && !self.resistance.active(),
                            |ui| {
                                self.resistance.panel(
                                    ui,
                                    &monitors,
                                    &id,
                                    layout.options.max_travel_distance,
                                )
                            },
                        )
                        .inner;
                    return;
                }
                if self.mode == lbm_ui::mode::Mode::Info {
                    lbm_ui::icons::title(
                        ui,
                        lbm_ui::icons::Icon::Chip,
                        "Detected · last display refresh",
                    );
                    let report = layout
                        .monitors()
                        .iter()
                        .find(|m| m.id == id)
                        .and_then(|m| m.device_id.as_ref())
                        .and_then(|device| self.reports.get(device));
                    if let Some(rows) = report {
                        lbm_ui::properties::rows(ui, rows);
                    } else {
                        ui.label("No hardware report available for this monitor.");
                    }
                    ui.separator();
                    lbm_ui::icons::title(ui, lbm_ui::icons::Icon::Layout, "Current layout");
                    lbm_ui::properties::rows(ui, &lbm_app::properties::layout_info(layout, &id));
                    return;
                }
                if self.mode == lbm_ui::mode::Mode::Location {
                    lbm_ui::icons::title(ui, lbm_ui::icons::Icon::Move, "Physical position");
                    if let Some(location) = lbm_app::properties::location(layout, &id) {
                        edits = ui
                            .add_enabled_ui(!blocked && !layout.is_virtual(), |ui| {
                                lbm_ui::properties::location(ui, &id, &location)
                            })
                            .inner;
                    }
                    if let Some(monitor) = layout.monitor(&id) {
                        ui.separator();
                        for (name, value) in
                            details_of(layout, monitor, self.mode).into_iter().skip(2)
                        {
                            ui.label(format!("{name}: {value}"));
                        }
                    }
                    return;
                }
                let Some(size) = lbm_app::properties::size(layout, &id) else {
                    ui.label("No active source for this monitor.");
                    return;
                };
                edits = ui
                    .add_enabled_ui(!blocked && !layout.is_virtual(), |ui| {
                        lbm_ui::properties::size(ui, &id, &size)
                    })
                    .inner;
            });
        });
        if let Some(action) = section_edit {
            self.section_edit(action);
        }
        for edit in edits {
            if let Some(layout) = self.layout.as_mut() {
                match lbm_app::properties::apply(
                    layout,
                    (self.requests.is_some() && !self.agent_keeps_intrinsic_size)
                        .then_some(&self.detected),
                    &id,
                    edit,
                ) {
                    Ok(()) => {
                        self.property_error = None;
                        self.screens = screens_of(layout, self.mode);
                        self.probe.want(None, std::time::Instant::now());
                    }
                    Err(error) => self.property_error = Some(error),
                }
            }
        }
        if let Some(error) = &self.property_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn placement_blocked(&self) -> Option<&'static str> {
        if self.requests.is_none() {
            Some("Connect to the agent to synchronize positions.")
        } else if self.refresh.pending() {
            Some("Reload the screen configuration first.")
        } else if self.layout.as_ref().is_none_or(|l| l.is_virtual()) {
            Some("A local screen layout is required.")
        } else if self.agent.suspended {
            Some("The display session is suspended.")
        } else if self.state.waiting
            || self.outputs.busy()
            || self.topology_pending.is_some()
            || self.drag.is_some()
        {
            Some("Wait for the current operation to finish.")
        } else {
            None
        }
    }

    fn place_from_system(&mut self) {
        if self.placement_blocked().is_some() {
            return;
        }
        let Some(layout) = self.layout.as_mut() else {
            return;
        };
        layout.set_locations_from_system_configuration(true);
        self.screens = screens_of(layout, self.mode);
        self.property_error = None;
        self.probe.want(None, std::time::Instant::now());
        self.next_screen_overlay = std::time::Instant::now();
    }

    fn placement_properties(&mut self, ui: &mut egui::Ui) {
        let blocked = self.placement_blocked();
        let needs_stop = cfg!(windows)
            && (self.agent.enabled != Some(false)
                || self.agent.previewing
                || self.state.live
                || matches!(
                    self.state.engine,
                    lbm_ui::Engine::Running | lbm_ui::Engine::Paused
                ));
        let action = self
            .placement
            .show(ui, blocked, needs_stop, cfg!(target_os = "linux"));
        match action {
            Some(lbm_ui::placement::Action::FromSystem) => self.place_from_system(),
            Some(lbm_ui::placement::Action::Preview) => self.dry_run_topology(),
            Some(lbm_ui::placement::Action::Apply) => self.apply_topology(),
            None => {}
        }
    }

    fn system_display_properties(&mut self, ui: &mut egui::Ui) {
        // Linux connectors and Windows target interface paths are both carried here;
        // monitor IDs and display names are not output identities.
        let selected = self.layout.as_deref().and_then(|layout| {
            let monitor = layout.monitor(self.selected.as_deref()?)?;
            let source = layout.source(monitor.active_source.as_deref()?)?;
            source
                .source
                .interface_path
                .as_ref()
                .filter(|path| !path.is_empty())
                .cloned()
        });
        if self.outputs.panel.selected != selected {
            self.outputs.panel.confirm = None;
        }
        self.outputs.panel.selected = selected;
        if !self.outputs.panel.wanted(self.outputs.outputs.as_ref()) {
            return;
        }
        ui.add_space(12.0);
        ui.separator();
        let blocked = self.output_blocked();
        let busy = self.outputs.busy();
        let can_stop = self.requests.is_some()
            && !self.state.waiting
            && (self.agent.enabled == Some(true)
                || self.agent.previewing
                || matches!(
                    self.state.engine,
                    lbm_ui::Engine::Running | lbm_ui::Engine::Paused
                ));
        let warn = self
            .layout
            .as_ref()
            .is_none_or(|l| l.options.show_monitor_action_warning);
        let action = self.outputs.panel.draw(
            ui,
            self.outputs.outputs.as_ref(),
            blocked,
            busy,
            warn,
            can_stop,
        );
        if let Some(error) = &self.outputs.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(action) = action {
            self.output_action(action);
        }
    }

    fn section_edit(&mut self, action: lbm_ui::resistance::Action) {
        if self.refresh.pending() || (self.outputs.changing() || self.topology_pending.is_some()) {
            return;
        }
        if let Some(layout) = self.layout.as_mut() {
            match lbm_app::resistance::apply(layout, action) {
                Ok(selected) => {
                    self.resistance.selected = selected;
                    self.property_error = None;
                    self.probe.want(None, std::time::Instant::now());
                }
                Err(error) => self.property_error = Some(error),
            }
        }
    }

    fn screen_overlay_controls(&mut self, ui: &mut egui::Ui) {
        use lbm_overlay::Mode;
        let (mode, label, selected) = match self.mode {
            lbm_ui::mode::Mode::Location => {
                (Mode::Rulers, "Rulers on screens", self.selected.is_some())
            }
            lbm_ui::mode::Mode::Resistance => (Mode::Resistance, "Resistance on screens", true),
            _ => return,
        };
        let available = !self.settings_open
            && lbm_overlay::SUPPORTED
            && self.requests.is_some()
            && !self.refresh.pending()
            && !(self.outputs.changing() || self.topology_pending.is_some())
            && !self.agent.suspended
            && self.layout.as_ref().is_some_and(|l| !l.is_virtual());
        let icon = if mode == Mode::Rulers {
            lbm_ui::icons::Icon::Size
        } else {
            lbm_ui::icons::Icon::Border
        };
        if lbm_ui::icons::toggle(
            ui,
            icon,
            label,
            self.screen_overlay_mode == mode,
            available && selected,
        )
        .clicked()
        {
            self.screen_overlay_mode = if self.screen_overlay_mode == mode {
                Mode::Off
            } else {
                mode
            };
            self.screen_overlay_error = None;
            self.next_screen_overlay = std::time::Instant::now();
        }
        if !lbm_overlay::SUPPORTED {
            ui.small("Screen overlays are unavailable on this platform.");
        }
        if self.screen_overlay_mode == mode {
            ui.small(if mode == Mode::Rulers {
                "Drag to align · wheel to scale · Ctrl: fine · Shift: coarse · Esc to cancel/hide"
            } else {
                "Drag to draw/move/resize · double-click to fill · Ctrl: no snap · Delete · Esc"
            });
        }
        if let Some(error) = &self.screen_overlay_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.add_space(5.0);
    }

    fn crossing_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            lbm_ui::icons::glyph(ui, lbm_ui::icons::Icon::Crossing);
            ui.checkbox(&mut self.show_probe, "Show crossings")
        })
        .inner
        .on_hover_text("Green: a passage. Red: a wall. Resistance is bypassed for this geometric report. Probing does not start or change the mouse engine.");
        if !self.show_probe {
            return;
        }
        ui.small("Green: passage · Red: wall · Resistance bypassed");
        if let Some(error) = &self.probe.error {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Crossings unavailable: {error}"),
            );
            if lbm_ui::icons::action(ui, lbm_ui::icons::Icon::Refresh, "Retry crossings", true)
                .clicked()
            {
                self.probe.retry(std::time::Instant::now());
                ui.ctx().request_repaint();
            }
        } else if self.probe.waiting() {
            ui.small("Checking crossings…");
        } else if self.requests.is_none() {
            ui.small("Connect to the agent to show crossings");
        } else if self
            .agent
            .layout
            .as_ref()
            .is_some_and(|s| s.revision.is_none())
        {
            ui.small("Update the agent to show crossings");
        }
    }

    fn screen_overlay_input(&mut self) {
        let allowed = !self.settings_open
            && self.requests.is_some()
            && !self.refresh.pending()
            && !(self.outputs.changing() || self.topology_pending.is_some())
            && !self.agent.suspended
            && matches!(self.view, View::Map | View::List)
            && self.screen_overlay_mode != lbm_overlay::Mode::Off;
        for input in self.screen_overlay.take_input() {
            if !allowed {
                break;
            }
            let Some(layout) = self.layout.as_mut() else {
                break;
            };
            let valid = lbm_overlay::scene::from_layout(
                layout,
                self.screen_overlay_mode,
                self.selected.as_deref(),
            )
            .is_ok_and(|bands| {
                bands.iter().any(|b| {
                    b.monitor == input.band.monitor
                        && b.connector == input.band.connector
                        && b.screen == input.band.screen
                        && b.side == input.band.side
                        && std::mem::discriminant(&b.content)
                            == std::mem::discriminant(&input.band.content)
                })
            });
            if !valid {
                self.screen_input.cancel(&mut self.resistance);
                continue;
            }
            match self
                .screen_input
                .handle(layout, &mut self.resistance, input)
            {
                Ok(out) => {
                    if out.hide {
                        self.screen_overlay_mode = lbm_overlay::Mode::Off;
                        break;
                    }
                    if let Some(id) = out.selected {
                        self.selected = Some(id);
                    }
                    if out.changed {
                        self.probe.want(None, std::time::Instant::now());
                        self.property_error = None;
                    }
                }
                Err(error) => {
                    self.property_error = Some(error);
                    self.screen_input.cancel(&mut self.resistance);
                }
            }
            self.screens = screens_of(self.screen_input.preview().unwrap_or(layout), self.mode);
            self.next_screen_overlay = std::time::Instant::now();
        }
        if !allowed {
            let active = self.screen_input.active();
            self.screen_input.cancel(&mut self.resistance);
            if active {
                if let Some(layout) = &self.layout {
                    self.screens = screens_of(layout, self.mode);
                }
            }
        }
    }

    fn screen_overlay_tick(&mut self, ctx: &egui::Context, movement: Option<(f64, f64)>) {
        use lbm_overlay::Mode;
        if self.settings_open
            || self.requests.is_none()
            || self.refresh.pending()
            || (self.outputs.changing() || self.topology_pending.is_some())
            || self.agent.suspended
            || !matches!(self.view, View::Map | View::List)
            || self.layout.as_ref().is_none_or(|l| l.is_virtual())
            || ctx
                .input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().minimized == Some(true))
        {
            self.screen_overlay_mode = Mode::Off;
        }
        if self.screen_overlay_mode == Mode::Off {
            let active = self.screen_input.active();
            self.screen_input.cancel(&mut self.resistance);
            if active {
                if let Some(layout) = &self.layout {
                    self.screens = screens_of(layout, self.mode);
                }
            }
            self.screen_overlay.clear();
            return;
        }
        // Limit CPU rasterization during pointer gestures. Hiding above is immediate.
        let now = std::time::Instant::now();
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        if now < self.next_screen_overlay {
            return;
        }
        self.next_screen_overlay = now + std::time::Duration::from_millis(50);
        // Rendering follows local gestures; Save/Live only see the original committed layout.
        let mut drawn = self
            .screen_input
            .preview()
            .unwrap_or(self.layout.as_deref().unwrap())
            .clone();
        if let Some(action) = self.resistance.preview_action() {
            let _ = lbm_app::resistance::apply(&mut drawn, action);
        }
        if let (Some(drag), Some((x, y))) = (&self.drag, movement) {
            drawn.offset_location(&drag.id, lbm_layout::geo::Vector::new(x, y));
        }
        let reference = self
            .resistance
            .guide()
            .and_then(lbm_app::resistance::overlay_reference);
        let result = lbm_overlay::scene::with_reference(
            &drawn,
            self.screen_overlay_mode,
            self.selected.as_deref(),
            reference,
        )
        .and_then(|mut bands| {
            if let Some(selected) = &self.resistance.selected {
                for band in &mut bands {
                    if band.monitor == selected.monitor
                        && lbm_app::screen_input::side(band.side) == selected.side
                    {
                        band.selected = Some(selected.index);
                    }
                }
            }
            self.screen_overlay.sync(&bands)
        });
        if let Err(error) = result {
            self.screen_overlay.clear();
            self.screen_overlay_mode = Mode::Off;
            self.screen_overlay_error = Some(error);
        }
    }

    fn test_pattern_target(&self) -> Option<String> {
        if self.mode != lbm_ui::mode::Mode::Vcp
            || self.refresh.pending()
            || self.topology_pending.is_some()
            || self.outputs.changing()
            || self.agent.suspended
            || self.requests.is_none()
        {
            return None;
        }
        let layout = self.layout.as_ref().filter(|l| !l.is_virtual())?;
        let monitor = layout.monitor(self.selected.as_deref()?)?;
        let source = layout.source(monitor.active_source.as_deref()?)?;
        source
            .source
            .interface_path
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
    }

    fn test_pattern_screens(&self) -> Vec<lbm_ui::patterns::Target> {
        if self.mode != lbm_ui::mode::Mode::Vcp
            || self.refresh.pending()
            || self.topology_pending.is_some()
            || self.outputs.changing()
            || self.agent.suspended
            || self.requests.is_none()
        {
            return Vec::new();
        }
        let Some(layout) = self.layout.as_ref().filter(|l| !l.is_virtual()) else {
            return Vec::new();
        };
        let mut screens = Vec::new();
        for monitor in layout.monitors() {
            let Some(source) = monitor
                .active_source
                .as_deref()
                .and_then(|id| layout.source(id))
            else {
                continue;
            };
            let Some(connector) = source
                .source
                .interface_path
                .as_ref()
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            if !source.source.attached_to_desktop
                || screens
                    .iter()
                    .any(|t: &lbm_ui::patterns::Target| &t.connector == connector)
            {
                continue;
            }
            let name = layout
                .model(&monitor.model)
                .and_then(|m| m.pnp_device_name.as_deref())
                .unwrap_or(&monitor.id);
            screens.push(lbm_ui::patterns::Target {
                connector: connector.clone(),
                name: format!("{name} · {connector}"),
            });
        }
        screens
    }

    fn test_pattern_tick(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.test_patterns.stop();
        }
        let available = self.test_pattern_screens();
        let selected = self.test_pattern_target();
        let targets = self.pattern_strip.targets(&available, selected.as_deref());
        if self.test_patterns.active().is_some() && self.test_patterns.targets() != targets {
            if targets.is_empty() {
                self.test_patterns.stop();
            } else {
                self.test_patterns
                    .show(&targets, &self.pattern_strip.settings, ctx);
            }
        }
        self.test_patterns.poll(ctx);
    }

    fn test_pattern_controls(&mut self, ui: &mut egui::Ui) {
        self.test_pattern_tick(ui.ctx());
        let available = self.test_pattern_screens();
        let selected = self.test_pattern_target();
        let blocked = if !lbm_app::patterns::Controller::supported() {
            Some("Native test patterns currently require a Wayland session.")
        } else if available.is_empty() || (!self.pattern_strip.matching && selected.is_none()) {
            Some("Select a local screen after the display configuration has loaded.")
        } else {
            None
        };
        let action = self.pattern_strip.show(
            ui,
            self.test_patterns.active(),
            self.test_patterns.ready(),
            blocked,
            &available,
            selected.as_deref(),
        );
        match action {
            Some(lbm_ui::patterns::Action::Hide) => self.test_patterns.stop(),
            Some(lbm_ui::patterns::Action::Show | lbm_ui::patterns::Action::Update) => {
                let targets = self.pattern_strip.targets(&available, selected.as_deref());
                self.test_patterns
                    .show(&targets, &self.pattern_strip.settings, ui.ctx());
            }
            None => {}
        }
        if let Some(error) = &self.test_patterns.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(note) = &self.test_patterns.note {
            ui.colored_label(lbm_ui::theme::AMBER, note);
        }
    }

    fn vcp_properties(&mut self, ui: &mut egui::Ui) {
        let key = self.wallpaper_key().and_then(|key| {
            Some(lbm_app::vcp::Key {
                layout: key.layout,
                revision: key.revision,
                monitor: self.selected.clone()?,
            })
        });
        self.remote.select(key.clone());
        self.vcp.select(key);
        if self.requests.is_none() {
            ui.label("Connect to the agent to control the monitor.");
            return;
        }
        if self.refresh.pending() {
            ui.label("Reload the display layout before controlling the monitor.");
            return;
        }
        if self.selected.is_none() {
            ui.label("Select a screen to read its monitor controls.");
            return;
        }
        if self.layout.as_ref().is_none_or(|l| l.is_virtual()) {
            ui.label("Select a local layout to control physical monitors.");
            return;
        }
        if self.remote.needs_read() {
            self.remote_request(lbm_ipc::remote::Command {
                brand: self.remote.panel.brand,
                action: lbm_ipc::remote::Action::Info,
            });
        }
        let recognized = self.remote.recognized();
        egui::CollapsingHeader::new("Network remote")
            .id_salt(("network-remote", self.selected.clone(), recognized))
            .default_open(recognized)
            .show(ui, |ui| {
                let busy = self.remote.busy();
                match self.remote.panel.show(
                    ui,
                    self.remote.state.as_ref(),
                    busy,
                    self.remote.error.as_deref(),
                ) {
                    Some(lbm_ui::remote::Did::Brand(brand)) => self.remote.switch(brand),
                    Some(lbm_ui::remote::Did::Command(command)) => self.remote_request(command),
                    None => {}
                }
            });
        ui.separator();
        // Smart displays can have no DDC bus at all. Network remotes never wait
        // for a DDC probe; allow an explicit probe on these displays instead.
        if self.vcp.needs_read() && !recognized {
            self.vcp_request(None);
        }
        ui.strong("DDC/CI controls");
        let busy = self.vcp.busy();
        let action = self.vcp.panel.show(
            ui,
            self.vcp.monitor.as_mut(),
            busy,
            self.vcp.error.as_deref(),
        );
        match action {
            Some(lbm_ui::vcp::Action::Refresh) => self.vcp_request(None),
            Some(lbm_ui::vcp::Action::Set(write)) => self.vcp_request(Some(write)),
            None => {}
        }
    }

    fn remote_request(&mut self, command: lbm_ipc::remote::Command) {
        let Some(requests) = &self.requests else {
            return;
        };
        let id = self.next_id;
        let Some(extra) = self.remote.request(id, command) else {
            return;
        };
        self.next_id += 1;
        if requests.send((id, "Remote", extra)).is_err() {
            self.remote
                .answered(id, &Err("The agent stopped listening.".into()));
        }
    }

    fn vcp_request(&mut self, write: Option<lbm_ipc::vcp::Write>) {
        let Some(requests) = &self.requests else {
            return;
        };
        let id = self.next_id;
        let Some(extra) = self.vcp.request(id, write) else {
            return;
        };
        self.next_id += 1;
        if requests.send((id, "Vcp", extra)).is_err() {
            self.vcp
                .answered(id, &Err("The agent stopped listening.".into()));
        }
    }

    fn output_request(&mut self, method: &'static str, extra: serde_json::Value, change: bool) {
        let Some(requests) = &self.requests else {
            return;
        };
        if self.outputs.busy() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.outputs.sent(id, change);
        if requests.send((id, method, extra)).is_err() {
            self.outputs
                .answered(id, &Err("the agent stopped listening".into()));
        }
    }

    fn output_action(&mut self, action: lbm_ui::displays::Did) {
        use lbm_ui::displays::Did;
        match action {
            Did::Refresh => {
                self.outputs.invalidate();
                self.output_request("DisplayOutputs", serde_json::json!({}), false);
            }
            Did::Stop => {
                self.state.live = false;
                self.send("Stop", serde_json::json!({}));
            }
            Did::Change(connector, change) if self.output_blocked().is_none() => {
                let Some(layout) = &self.layout else {
                    return;
                };
                let Some(revision) = self.agent.layout.as_ref().and_then(|s| s.revision) else {
                    return;
                };
                if let Some(extra) = self
                    .outputs
                    .request(&layout.id, revision, &connector, change)
                {
                    self.output_request("ChangeOutput", extra, true);
                }
            }
            _ => {}
        }
    }
}

// A live preview does not outlive the window that asked for it — and nothing here does
// that. The preview belongs to this window's *connection*: when the process ends, the
// socket closes, and the agent ends the preview on its own.
//
// This window used to send `EndPreview` from `on_exit`, which covered a clean close and
// nothing else: a window that is killed, crashes, or whose session is torn down runs no
// `on_exit` at all, and a preview is a `Load` **and** a `Run` — so the engine would be
// left driving an arrangement the user never saved, with nothing on screen able to turn
// it off. Only the agent can see a connection drop, so only the agent can close that gap,
// and now does (`runtime::left`). A second mechanism here would just be a second way to
// end somebody else's preview.
impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.instance_requests(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = _frame.window_handle() {
                if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    self.screen_overlay.set_owner(handle.hwnd.get());
                }
            }
        }
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            self.picker_parent = lbm_app::image_picker::Parent::None;
            if let Ok(handle) = _frame.window_handle() {
                self.picker_parent = match handle.as_raw() {
                    RawWindowHandle::Win32(h) => {
                        lbm_app::image_picker::Parent::Windows(h.hwnd.get())
                    }
                    RawWindowHandle::Xlib(h) => {
                        lbm_app::image_picker::Parent::X11(h.window as usize)
                    }
                    RawWindowHandle::Xcb(h) => {
                        lbm_app::image_picker::Parent::X11(h.window.get() as usize)
                    }
                    _ => lbm_app::image_picker::Parent::None,
                };
            }
            #[cfg(target_os = "linux")]
            if let Some(window) = _frame.winit_window() {
                self.picker_parent = lbm_app::image_picker::Parent::Window(window.clone());
            }
        }
        self.retire_departed_file_choice();
        let ctx = ui.ctx().clone();
        // The answer, when it has arrived. Nothing is polled: the thread asks for a
        // repaint after sending, so this runs on the pass that request causes.
        // Everything that has landed, not just the first: several thumbnails can arrive
        // between two passes, and so can a burst of agent states. Taken off the channel
        // before any of it is acted on, because acting on it needs the whole window and
        // the channel is part of it.
        let landed: Vec<Found> = match &self.arriving {
            Some(arriving) => std::iter::from_fn(|| arriving.try_recv().ok()).collect(),
            None => Vec::new(),
        };
        {
            for found in landed {
                match found {
                    Found::Displays {
                        generation,
                        mut layout,
                        reports,
                        detected,
                        excluded,
                    } => {
                        if self.detecting == Some(generation) {
                            self.detecting = None;
                        }
                        if !self.refresh.complete(generation, &layout.id) {
                            self.retry_display_at =
                                std::time::Instant::now() + std::time::Duration::from_millis(500);
                            self.display_error =
                                Some("Waiting for the displays to match the agent".into());
                            continue;
                        }
                        self.pending_options.apply(&mut layout.options);
                        let excluded = self.pending_options.excluded().or(excluded);
                        self.saved = Some(lbm_app::saved::Reference::of(&layout));
                        self.restore_replacement_draft(&mut layout);
                        self.screens = screens_of(&layout, self.mode);
                        self.reports = reports;
                        self.detected = detected;
                        self.resistance.clear();
                        self.property_error = None;
                        self.layout = Some(layout);
                        self.excluded = excluded;
                        self.desktop_wallpaper = Default::default();
                        self.desktop_textures.clear();
                        self.wallpapers.clear();
                        self.wallpaper.preview_generation += 1;
                        self.wallpaper_preview_busy = None;
                        self.drag = None;
                        self.selected = self
                            .selected
                            .take()
                            .filter(|id| self.screens.iter().any(|s| &s.id == id));
                        self.display_error = None;
                    }
                    Found::Session(event) => self.session_event(event),
                    Found::DetectionFailed { generation } => {
                        if self.detecting == Some(generation) {
                            self.detecting = None;
                        }
                        self.retry_display_at =
                            std::time::Instant::now() + std::time::Duration::from_secs(2);
                        self.display_error = Some("Display detection failed; retrying".into());
                    }
                    Found::WallpaperFile { choice, result } => {
                        self.wallpaper_file_chosen(*choice, result)
                    }
                    Found::WallpaperPreview {
                        generation,
                        document,
                        result,
                    } => {
                        if self.wallpaper_preview_busy == Some(generation) {
                            self.wallpaper_preview_busy = None;
                        }
                        if generation != self.wallpaper.preview_generation {
                            continue;
                        }
                        // A shared source is independent of geometry: project it against
                        // the current layout even if a monitor moved while decoding.
                        let shared_source = result.as_ref().is_ok_and(|p| p.panorama.is_some());
                        if !shared_source
                            && !self
                                .layout
                                .as_ref()
                                .is_some_and(|l| lbm_store::LayoutDocument::of(l) == *document)
                        {
                            self.wallpaper_preview_requested = true;
                            continue;
                        }
                        match result {
                            Ok(painted) => {
                                self.wallpapers.clear();
                                self.panorama = painted.panorama.map(|(path, size, pixels)| {
                                    let image = egui::ColorImage::from_rgba_unmultiplied(
                                        [pixels.width() as usize, pixels.height() as usize],
                                        &pixels,
                                    );
                                    (
                                        path,
                                        size,
                                        ctx.load_texture(
                                            "panorama-source",
                                            image,
                                            egui::TextureOptions::LINEAR,
                                        ),
                                    )
                                });
                                self.panorama_editor = Default::default();
                                for p in painted.screens {
                                    let size =
                                        [p.pixels.width() as usize, p.pixels.height() as usize];
                                    let image =
                                        egui::ColorImage::from_rgba_unmultiplied(size, &p.pixels);
                                    self.wallpapers.insert(
                                        p.id.clone(),
                                        ctx.load_texture(
                                            format!("wallpaper:{}", p.id),
                                            image,
                                            egui::TextureOptions::LINEAR,
                                        ),
                                    );
                                }
                            }
                            Err(e) => self.wallpaper.error = Some(e),
                        }
                    }
                }
            }
        }

        self.read_displays(&ctx, false);
        self.sync_wallpaper(&ctx);
        self.screen_overlay_input();
        if self.refresh.pending() && !self.dirty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        // One logo per frame, not all of them. Parsing and rasterising an SVG is not
        // free, and doing every screen's on the frame that first needs them is the same
        // stall as detecting on the UI thread, only later.
        let next = self
            .screens
            .iter()
            .filter_map(|s| s.logo.as_ref()?.to_str())
            .find(|path| !self.textures.contains_key(*path))
            .map(str::to_owned);
        if let Some(path) = next {
            let _ = self.logo(&ctx, &path);
            ctx.request_repaint();
        }
        let logos = self.textures.clone();

        let previous_view = self.view;
        let settings_was_open = self.settings_open;
        let previous_mode = self.mode;
        egui::Panel::top("modes")
            .frame(lbm_ui::theme::toolbar())
            .show(ui, |ui| {
                if let Some(action) =
                    lbm_ui::mode::toolbar(ui, self.mode, &self.offered(), self.settings_open)
                {
                    use lbm_ui::mode::ToolbarAction;
                    match action {
                        ToolbarAction::About => {
                            self.about_open = true;
                            self.settings_open = false;
                            self.drag = None;
                            self.resistance.cancel();
                            self.screen_overlay_mode = lbm_overlay::Mode::Off;
                            self.screen_overlay.clear();
                        }
                        ToolbarAction::Options => {
                            self.settings_open = !self.settings_open;
                            if self.settings_open {
                                self.send("SeenProcesses", serde_json::json!({}));
                            }
                        }
                        ToolbarAction::Mode(chosen) => {
                            self.settings_open = false;
                            self.mode = chosen;
                            self.drag = None;
                            if let Some(layout) = self.layout.as_deref() {
                                self.screens = screens_of(layout, chosen);
                            }
                        }
                    }
                }
            });

        if previous_view != self.view
            || previous_mode != self.mode
            || settings_was_open != self.settings_open
            || self.refresh.pending()
        {
            self.drag = None;
            self.resistance.cancel();
            self.screen_overlay_mode = lbm_overlay::Mode::Off;
            self.screen_overlay.clear();
        }
        if self.refresh.pending() {
            egui::Panel::top("display-change").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.dirty() {
                        ui.label("Displays changed or the agent reconnected. Your unsaved edits are kept until you reload.");
                        if ui.add_enabled(self.detecting.is_none(), egui::Button::new("Discard edits and reload screens")).clicked() {
                            self.read_displays(&ctx, true);
                        }
                    } else {
                        ui.label(self.display_error.as_deref().unwrap_or("Refreshing screens…"));
                    }
                    if self.requests.is_some() && ui.button("Stop engine").clicked() {
                        self.send("Stop", serde_json::json!({}));
                    }
                });
            });
        }

        egui::Panel::bottom("controls")
            .frame(lbm_ui::theme::toolbar())
            .show(ui, |ui| {
                if self.refresh.pending()
                    || (self.outputs.changing() || self.topology_pending.is_some())
                {
                    ui.disable();
                }
                // Without an agent every button is disabled on its own — `can` asks for a
                // hook — so nothing here has to hide them.
                // From the layout, every frame: the document it would save against the one it
                // loaded or last saved, so the bar cannot fall out of step with what there is
                // to save — and an edit undone by hand takes the buttons back out, which the
                // ported `Saved` latch cannot do. See `lbm_app::saved`.
                //
                // Rebuilt rather than cached: the cache would be one more thing to keep in
                // step with every edit anywhere in the window, which is the failure this is
                // fixing. It is a walk over owned data, the same one the live preview already
                // makes five times a second.
                self.state.saved = match (&self.layout, &self.saved) {
                    (Some(layout), Some(reference)) => reference.holds(layout),
                    // Nothing loaded, or loaded without a reference: nothing to save.
                    _ => true,
                };
                if let Some(lbm_ui::Action::Pressed(press)) =
                    lbm_ui::bottom_bar_with_agent(ui, &self.state, |ui| {
                        match (&self.without_agent, &self.agent.refused) {
                            (Some((why, where_)), _) => {
                                lbm_ui::theme::status(
                                    ui,
                                    if why == "connecting to agent" {
                                        "Connecting…"
                                    } else {
                                        "Agent disconnected"
                                    },
                                    lbm_ui::theme::AMBER,
                                )
                                .on_hover_text(format!("{why}\n{where_}"));
                            }
                            (None, Some(why)) => {
                                ui.colored_label(ui.visuals().error_fg_color, why);
                            }
                            (None, None) => {
                                lbm_ui::theme::status(ui, "Agent connected", lbm_ui::theme::ACCENT);
                            }
                        }
                    })
                {
                    for effect in lbm_ui::update(&mut self.state, lbm_ui::Action::Pressed(press)) {
                        match effect {
                            lbm_ui::Effect::Ask(press) => self.ask(press),
                            lbm_ui::Effect::EndPreview => self.end_preview(),
                        }
                    }
                }
            });

        let settings_bounds = ui.available_rect_before_wrap();
        self.properties(ui);
        self.wallpaper_panel(ui);
        self.probe_tick(&ctx);

        // The `Ui` handed to `App::ui` has no background of its own — the doc says so
        // outright — so the map would be drawn on nothing.
        let mut save_options = false;
        let mut excluded_did = None;
        let resistance_mode = self.mode == lbm_ui::mode::Mode::Resistance;
        let section_monitors = if resistance_mode {
            self.layout
                .as_deref()
                .map(lbm_app::resistance::monitors)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut section_editor = std::mem::take(&mut self.resistance);
        let mut section_action = None;
        let mut inline_edits = Vec::new();
        if self.mode != lbm_ui::mode::Mode::Wallpaper {
            self.panorama_editor = Default::default();
        }
        let mut panorama_editor = std::mem::take(&mut self.panorama_editor);
        let mut panorama_placement = self.wallpaper.draft.as_ref().and_then(|s| s.span_placement);
        let mut panorama_changed = false;
        let acted = egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(lbm_ui::theme::BACKGROUND)
                    .inner_margin(14),
            )
            .show(ui, |ui| {
                if self.settings_open {
                    ui.disable();
                }
                if self.refresh.pending()
                    || (self.outputs.changing() || self.topology_pending.is_some())
                {
                    ui.disable();
                }
                let monitors = self.monitors(&logos);
                let at = ui.max_rect();
                if self.mode == lbm_ui::mode::Mode::Wallpaper
                    && self.wallpaper.draft.as_ref().is_some_and(|s| {
                        s.mode == lbm_store::wallpaper_settings::WallpaperMode::Span
                    })
                {
                    if self.wallpaper.busy() {
                        ui.disable();
                    }
                    if let Some((path, size, texture)) = &self.panorama {
                        if self
                            .wallpaper
                            .draft
                            .as_ref()
                            .and_then(|s| s.span_image_path.as_ref())
                            == Some(path)
                        {
                            let attached: Vec<_> = monitors
                                .iter()
                                .filter(|m| {
                                    self.layout.as_ref().is_some_and(|l| {
                                        l.monitors()
                                            .iter()
                                            .find(|screen| screen.id == m.id)
                                            .and_then(|screen| screen.active_source.as_deref())
                                            .and_then(|id| l.source(id))
                                            .is_some_and(|s| s.source.attached_to_desktop)
                                    })
                                })
                                .cloned()
                                .collect();
                            panorama_changed = panorama_editor.show(
                                ui,
                                &attached,
                                texture,
                                *size,
                                &mut panorama_placement,
                                self.selected.as_deref(),
                            );
                            return (None, None);
                        }
                    }
                    ui.centered_and_justified(|ui| {
                        ui.label("Choose an image to frame the physical panorama.");
                    });
                    return (None, None);
                }
                match self.view {
                    View::Map => {
                        // **Measured before anything moves.** The Avalonia presenter asks
                        // the layout for its ratio on every move
                        // (`FrameMover.cs:166`) and the layout only republishes its
                        // extent when a position is written — which nothing does until
                        // the drop. So the scale and the corner belong to the desktop as
                        // it was when the gesture started, and a screen dragged past the
                        // edge goes off the map instead of shrinking it under the pointer.
                        let fit = map::fit(map::extent(&monitors), at);
                        // Ctrl held means no anchors: `MonitorLocationView.axaml.cs:185`.
                        let anchors = !ui.input(|i| i.modifiers.ctrl);
                        let held = self.held(&monitors, &fit, anchors);

                        // Drawn where the pointer has it, and **last**, so it stays on top
                        // of whatever it is sliding over.
                        let mut shown = monitors.clone();
                        if let Some((i, total, _)) = &held {
                            let moved = shown.remove(*i);
                            let by = Vector::new(total.0, total.1);
                            shown.push(MapMonitor {
                                mm_outside: moved.mm_outside.translate(by),
                                mm_content: moved.mm_content.translate(by),
                                ..moved
                            });
                        }

                        let mut gesture = map::draw_movable(
                            ui,
                            &shown,
                            &fit,
                            self.selected.as_deref(),
                            self.mode == lbm_ui::mode::Mode::Location,
                        );
                        let placements = shown.iter().map(|m| (m.id, fit.place(m)));
                        let picked = self.inline_properties(ui, placements, &mut inline_edits);
                        if let Some(picked) = picked {
                            gesture = Some(map::Gesture::Clicked(picked));
                        }
                        if resistance_mode {
                            if self.layout.as_ref().is_none_or(|l| l.is_virtual()) {
                                ui.disable();
                            }
                            let placements = shown
                                .iter()
                                .map(|m| (m.id.to_owned(), fit.place(m).content))
                                .collect::<Vec<_>>();
                            let interaction =
                                section_editor.bands(ui, &section_monitors, &placements);
                            section_action = interaction.action;
                            let picked =
                                interaction
                                    .monitor
                                    .map(Did::Clicked)
                                    .or_else(|| match gesture {
                                        Some(map::Gesture::Clicked(id)) => {
                                            Some(Did::Clicked(id.to_owned()))
                                        }
                                        _ => None,
                                    });
                            return (picked, None);
                        }
                        // After the frames: the lines are about the screens, so they are
                        // read over them. The C# adds its canvas to the panel the frames
                        // are already in, which puts it on top the same way.
                        if let Some((_, _, snap)) = &held {
                            drag::draw(ui, &fit, snap);
                        }
                        (owned(gesture), held.map(|(_, total, _)| total))
                    }
                    View::List => {
                        list::draw(ui, at, &monitors, self.selected.as_deref());
                        let mut picked = None;
                        let placements = monitors
                            .iter()
                            .filter(|m| Some(m.id) == self.selected.as_deref())
                            .map(|m| {
                                let fit = map::fit_in(list::screen_rect(at), m.mm_outside);
                                (m.id, fit.place(m))
                            });
                        picked = self
                            .inline_properties(ui, placements, &mut inline_edits)
                            .or(picked);
                        if resistance_mode {
                            if self.layout.as_ref().is_none_or(|l| l.is_virtual()) {
                                ui.disable();
                            }
                            let placements = monitors
                                .iter()
                                .find(|m| Some(m.id) == self.selected.as_deref())
                                .map(|m| {
                                    let fit = map::fit_in(list::screen_rect(at), m.mm_outside);
                                    vec![(m.id.to_owned(), fit.place(m).content)]
                                })
                                .unwrap_or_default();
                            let interaction =
                                section_editor.bands(ui, &section_monitors, &placements);
                            section_action = interaction.action;
                        }
                        (picked.map(|id| Did::Clicked(id.to_owned())), None)
                    }
                }
            })
            .inner;
        lbm_ui::properties::paint_measurement_highlights(&ctx);
        self.resistance = section_editor;
        self.panorama_editor = panorama_editor;
        if panorama_changed {
            if let Some(settings) = &mut self.wallpaper.draft {
                settings.span_placement = panorama_placement;
                self.wallpaper.preview_generation += 1;
                self.wallpaper.status =
                    Some("Framing previewed. Apply wallpaper to update the desktop.".into());
            }
        }
        let options_before = self.layout.as_ref().map(|l| l.options.clone());
        let mut settings_open = self.settings_open;
        let mut settings_page = self.settings_page;
        let escape_closes = self.recording != lbm_ui::shortcut::Recording::Yes;
        lbm_ui::options::drawer(
            &ctx,
            settings_bounds,
            &mut settings_open,
            &mut settings_page,
            escape_closes,
            |ui, page| {
                if self.refresh.pending()
                    || (self.outputs.changing() || self.topology_pending.is_some())
                {
                    ui.disable();
                }
                if self.requests.is_none() && !self.pending_options.is_empty() {
                    ui.colored_label(
                        lbm_ui::theme::AMBER,
                        "Not saved yet: sent when the agent reconnects.",
                    );
                }
                if page == lbm_ui::options::Page::Applications {
                    excluded_did = lbm_ui::excluded::panel(
                        ui,
                        self.excluded.as_deref(),
                        &mut self.pattern,
                        &self.agent.seen,
                    );
                } else if let Some(layout) = self.layout.as_mut() {
                    let touch = cfg!(windows) && page == lbm_ui::options::Page::Advanced;
                    let touch_displays = if touch {
                        lbm_app::settings::touch_displays(layout)
                    } else {
                        Vec::new()
                    };
                    layout.edit_options(|o| {
                        save_options = lbm_ui::options::panel(
                            ui,
                            o,
                            cfg!(windows),
                            &mut self.recording,
                            self.agent.shortcut_unavailable.as_deref(),
                            page,
                        )
                        .app;
                        if touch {
                            save_options |= lbm_ui::options::touch_panel(ui, o, &touch_displays);
                        }
                    });
                } else {
                    ui.label("Waiting for displays…");
                }
            },
        );
        if !settings_open || settings_page != self.settings_page {
            self.recording = lbm_ui::shortcut::Recording::No;
        }
        self.settings_open = settings_open;
        self.settings_page = settings_page;
        for (id, edit) in inline_edits {
            if let Some(layout) = self.layout.as_mut() {
                match lbm_app::properties::apply(
                    layout,
                    (self.requests.is_some() && !self.agent_keeps_intrinsic_size)
                        .then_some(&self.detected),
                    &id,
                    edit,
                ) {
                    Ok(()) => {
                        self.selected = Some(id);
                        self.property_error = None;
                        self.screens = screens_of(layout, self.mode);
                        self.probe.want(None, std::time::Instant::now());
                    }
                    Err(error) => self.property_error = Some(error),
                }
            }
        }
        if let Some(action) = section_action {
            self.section_edit(action);
        }
        let excluded_changed = excluded_did.is_some_and(|did| self.excluded_edit(did));
        if save_options || excluded_changed {
            if let Some(before) = &options_before {
                self.save_options(before, excluded_changed);
            }
        }
        lbm_ui::about::window(&ctx, &mut self.about_open, env!("CARGO_PKG_VERSION"));
        self.topology_report(&ctx);
        self.acted_on_the_map(acted.0, acted.1, &ctx);
        self.screen_overlay_tick(&ctx, acted.1);
        // Last, so a drag or a setting changed on this frame is in the document this
        // tick sends rather than waiting for the next one.
        self.test_pattern_tick(&ctx);
        self.preview_tick(&ctx);
    }
}

/// The gesture with the screens let go of, so the window can change them.
fn owned(gesture: Option<map::Gesture<'_>>) -> Option<Did> {
    Some(match gesture? {
        map::Gesture::Clicked(id) => Did::Clicked(id.to_owned()),
        map::Gesture::Dragged { id, by } => Did::Dragged {
            id: id.to_owned(),
            by,
        },
        map::Gesture::Dropped { id } => Did::Dropped { id: id.to_owned() },
    })
}

fn main() -> eframe::Result<()> {
    #[cfg(target_os = "linux")]
    if std::env::args().nth(1).as_deref() == Some("--test-pattern-host") {
        std::process::exit(lbm_pattern::viewer::run(std::env::args().skip(2)));
    }
    #[cfg(target_os = "linux")]
    if std::env::args().nth(1).as_deref() == Some("--screen-overlay-host") {
        std::process::exit(lbm_overlay::linux::run_host());
    }
    let mut replace = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--replace" => replace = true,
            "--help" | "-h" => {
                println!("LittleBigMouse\n\nUsage: lbm-app [--replace]\n\nWithout arguments, open or activate the session's editor.\n--replace  Close the current editor and open this build, carrying its layout draft.\n           The mouse engine keeps running.");
                return Ok(());
            }
            _ => {
                return Err(eframe::Error::AppCreation(Box::new(std::io::Error::other(
                    format!("unknown argument: {arg}"),
                ))))
            }
        }
    }
    let launch = lbm_app::instance::Address::session()
        .and_then(|address| lbm_app::instance::start(address, replace))
        .map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
    let lbm_app::instance::Launch::Open {
        owner: _owner,
        inbox,
        draft,
    } = launch
    else {
        return Ok(());
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LittleBigMouse")
            .with_icon(app_icon())
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([480.0, 320.0]),
        ..Default::default()
    };
    eframe::run_native(
        "LittleBigMouse",
        options,
        Box::new(move |cc| {
            inbox.attach(cc.egui_ctx.clone());
            let mut app = App::new(cc.egui_ctx.clone());
            app.instance = Some(inbox);
            app.replacement_draft = draft;
            Ok(Box::new(app))
        }),
    )
}

/// The Little Big Mouse logo for the title bar, taskbar and Alt+Tab. Without it eframe
/// substitutes its own egui logo. Same file as the exe icon (build.rs); the decoder
/// takes its largest picture and the system scales it down.
fn app_icon() -> std::sync::Arc<egui::IconData> {
    let logo = image::load_from_memory_with_format(
        include_bytes!(
            "../../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/lbm-logo.ico"
        ),
        image::ImageFormat::Ico,
    )
    .expect("bundled application icon")
    .into_rgba8();
    std::sync::Arc::new(egui::IconData {
        width: logo.width(),
        height: logo.height(),
        rgba: logo.into_raw(),
    })
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn replacement_restores_layout_edits_without_saving_them_or_overwriting_preferences() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut loaded = Layout::new(LayoutOptions {
            vcp_control: true,
            ..Default::default()
        });
        lbm_layout::linux::populate(&mut loaded, &[], |_| Ok::<(), std::io::Error>(())).unwrap();
        let saved = lbm_app::saved::Reference::of(&loaded);
        let mut edited = loaded.clone();
        let monitor = edited.monitors()[0].id.clone();
        edited.set_location(&monitor, lbm_layout::geo::Point::new(123., 45.));
        edited.edit_options(|o| o.vcp_control = false);
        let draft = serde_json::json!({
            "LayoutId": edited.id,
            "Document": lbm_app::settings::document(&edited),
            "Selected": monitor,
            "Mode": "size",
        });
        app.replacement_draft = Some(draft.clone());
        app.restore_replacement_draft(&mut loaded);
        assert!(
            !saved.holds(&loaded),
            "replacement must keep local edits unsaved"
        );
        assert_eq!(
            lbm_app::settings::document(&loaded),
            lbm_app::settings::document(&edited)
        );
        assert!(
            loaded.options.vcp_control,
            "current preferences belong to the store"
        );
        assert_eq!(app.selected.as_deref(), Some(monitor.as_str()));
        assert_eq!(app.mode, lbm_ui::mode::Mode::Size);
        assert!(app.replacement_draft.is_none());

        let mut other = Layout::new(LayoutOptions::default());
        other.id = "different displays".into();
        let before = lbm_app::settings::document(&other);
        app.replacement_draft = Some(draft);
        app.restore_replacement_draft(&mut other);
        assert_eq!(lbm_app::settings::document(&other), before);
    }

    #[test]
    fn the_window_carries_the_lbm_logo_rather_than_the_egui_default() {
        let icon = app_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        assert!(
            icon.rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 0),
            "transparent corners"
        );
    }

    #[test]
    fn test_patterns_follow_the_selected_connector_without_needing_ddc() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        lbm_layout::linux::populate(
            &mut layout,
            &[
                lbm_layout::linux::LinuxMonitor {
                    connector_name: "DP-2".into(),
                    ..lbm_layout::linux::LinuxMonitor::fallback()
                },
                lbm_layout::linux::LinuxMonitor {
                    connector_name: "DP-3".into(),
                    logical_x: 1920.,
                    primary: false,
                    ..lbm_layout::linux::LinuxMonitor::fallback()
                },
            ],
            |_| Ok::<(), ()>(()),
        )
        .unwrap();
        let ids: Vec<_> = layout.monitors().iter().map(|m| m.id.clone()).collect();
        app.layout = Some(Box::new(layout));
        app.requests = Some(tokio::sync::mpsc::unbounded_channel().0);
        app.refresh = Default::default();
        app.mode = lbm_ui::mode::Mode::Vcp;
        app.selected = Some(ids[0].clone());
        assert_eq!(app.test_pattern_target().as_deref(), Some("DP-2"));
        app.selected = Some(ids[1].clone());
        assert_eq!(app.test_pattern_target().as_deref(), Some("DP-3"));
        app.mode = lbm_ui::mode::Mode::Location;
        assert_eq!(app.test_pattern_target(), None);
        app.mode = lbm_ui::mode::Mode::Vcp;
        app.refresh.invalidate();
        assert_eq!(app.test_pattern_target(), None);
        app.refresh = Default::default();
        app.agent.suspended = true;
        assert_eq!(app.test_pattern_target(), None);
    }

    #[test]
    fn placement_import_can_be_undone_and_export_sends_current_edits() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        lbm_layout::linux::populate(
            &mut layout,
            &[
                lbm_layout::linux::LinuxMonitor::fallback(),
                lbm_layout::linux::LinuxMonitor {
                    connector_name: "DP-2".into(),
                    logical_x: 1920.,
                    primary: false,
                    ..lbm_layout::linux::LinuxMonitor::fallback()
                },
            ],
            |_| Ok::<(), ()>(()),
        )
        .unwrap();
        let monitor = layout.monitors()[1].id.clone();
        layout.set_location(&monitor, lbm_layout::geo::Point::new(123., 456.));
        app.saved = Some(lbm_app::saved::Reference::of(&layout));
        app.layout = Some(Box::new(layout));
        let (requests, incoming) = tokio::sync::mpsc::unbounded_channel();
        app.requests = Some(requests);
        app.refresh = Default::default();
        assert_eq!(app.placement_blocked(), None);
        app.place_from_system();
        assert!(app.dirty());
        assert_ne!(
            app.layout
                .as_ref()
                .unwrap()
                .monitor(&monitor)
                .unwrap()
                .location(),
            lbm_layout::geo::Point::new(123., 456.)
        );
        assert!(incoming.is_empty(), "Import only edits the local document");
        app.saved
            .as_ref()
            .unwrap()
            .restore(app.layout.as_mut().unwrap());
        assert!(!app.dirty());
        app.place_from_system();
        let expected = lbm_app::settings::document(app.layout.as_ref().unwrap());
        app.placement.adjust_scale = true;
        app.apply_topology();
        let mut incoming = incoming;
        let (id, method, fields) = incoming.try_recv().unwrap();
        assert_eq!(method, "ApplyTopology");
        assert_eq!(fields["AdjustScale"], true);
        assert_eq!(fields["Document"], serde_json::to_value(expected).unwrap());
        assert_eq!(app.topology_pending, Some(id));
        assert!(app.dirty(), "Sending is not acknowledgement");
        app.agent_said(Message::Answer {
            id,
            result: Ok(serde_json::json!({"Commands": [], "DryRun": false})),
        });
        assert!(app.topology_pending.is_none());
        assert!(
            !app.dirty(),
            "Successful apply saved exactly the sent layout"
        );
    }

    #[test]
    fn only_location_accepts_monitor_dragging_and_other_modes_keep_selection() {
        use lbm_ui::mode::Mode;
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        lbm_layout::linux::populate(
            &mut layout,
            &[lbm_layout::linux::LinuxMonitor::fallback()],
            |_| Ok::<(), ()>(()),
        )
        .unwrap();
        let id = layout.monitors()[0].id.clone();
        let before = lbm_store::LayoutDocument::of(&layout);
        app.layout = Some(Box::new(layout));
        for mode in std::iter::once(Mode::Default).chain(Mode::ALL) {
            if mode == Mode::Location {
                continue;
            }
            app.mode = Mode::Location;
            app.acted_on_the_map(
                Some(Did::Dragged {
                    id: id.clone(),
                    by: egui::vec2(80., 40.),
                }),
                None,
                &ctx,
            );
            assert!(app.drag.is_some());
            app.mode = mode;
            app.acted_on_the_map(
                Some(Did::Dropped { id: id.clone() }),
                Some((80., 40.)),
                &ctx,
            );
            assert!(app.drag.is_none());
            assert_eq!(
                before,
                lbm_store::LayoutDocument::of(app.layout.as_ref().unwrap())
            );
            app.acted_on_the_map(
                Some(Did::Dragged {
                    id: id.clone(),
                    by: egui::vec2(80., 40.),
                }),
                None,
                &ctx,
            );
            assert!(app.drag.is_none(), "{mode:?} accepted a monitor drag");
            app.selected = None;
            app.acted_on_the_map(Some(Did::Clicked(id.clone())), None, &ctx);
            assert_eq!(app.selected.as_ref(), Some(&id));
        }
    }
    use lbm_app::session::Event;
    use serde_json::json;

    #[test]
    fn screen_overlays_stop_on_disconnect_stale_layout_foreign_layout_suspend_or_escape() {
        for cause in 0..7 {
            let ctx = egui::Context::default();
            let mut app = App::with_config(ctx.clone(), None);
            let mut layout = Layout::new(LayoutOptions::default());
            layout.id = "A".into();
            let before = lbm_store::LayoutDocument::of(&layout);
            app.layout = Some(Box::new(layout));
            let (requests, mut incoming) = tokio::sync::mpsc::unbounded_channel();
            app.requests = Some(requests);
            let generation = app.refresh.begin(false).unwrap();
            app.refresh.complete(generation, "A");
            app.screen_overlay_mode = lbm_overlay::Mode::Resistance;
            let mut input = egui::RawInput::default();
            match cause {
                0 => app.requests = None,
                1 => app.refresh.invalidate(),
                2 => {
                    app.layout.as_mut().unwrap().source_kind =
                        lbm_layout::model::LayoutSource::VirtualImport
                }
                3 => app.agent.suspended = true,
                4 => app.settings_open = true,
                5 => input.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }),
                _ => {
                    input
                        .viewports
                        .entry(egui::ViewportId::ROOT)
                        .or_default()
                        .minimized = Some(true)
                }
            }
            let mut output = ctx.run_ui(input, |ui| app.screen_overlay_tick(ui.ctx(), None));
            output.textures_delta.clear();
            assert_eq!(app.screen_overlay_mode, lbm_overlay::Mode::Off);
            assert!(app.screen_overlay_error.is_none());
            assert_eq!(
                before,
                lbm_store::LayoutDocument::of(app.layout.as_ref().unwrap())
            );
            assert!(
                incoming.try_recv().is_err(),
                "overlays never send a Save or Preview request"
            );
        }
    }

    #[test]
    fn resistance_edits_are_previewed_and_a_stale_desktop_cannot_be_edited() {
        use lbm_ui::resistance::{Action, Edit, Side};
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        lbm_layout::linux::populate(
            &mut layout,
            &[lbm_layout::linux::LinuxMonitor::fallback()],
            |_| Ok::<(), ()>(()),
        )
        .unwrap();
        let action = Action::new(
            &lbm_app::resistance::monitors(&layout)[0],
            Side::Right,
            Edit::Insert(lbm_layout::model::BorderSection::new(
                10.0, 50.0, 3.0, false, 0.0, true,
            )),
        );
        app.saved = Some(lbm_app::saved::Reference::of(&layout));
        let id = layout.id.clone();
        app.layout = Some(Box::new(layout));
        let generation = app.refresh.begin(false).unwrap();
        app.refresh.complete(generation, &id);
        let (requests, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        app.requests = Some(requests);
        app.state.live = true;
        app.section_edit(action);
        assert!(app.dirty());
        app.preview_tick(&ctx);
        let (_, method, request) = incoming.try_recv().unwrap();
        assert_eq!(method, "Preview");
        let document: lbm_store::LayoutDocument =
            serde_json::from_value(request["Document"].clone()).unwrap();
        assert_eq!(
            document,
            lbm_app::settings::document(app.layout.as_ref().unwrap())
        );
        let action = Action::new(
            &lbm_app::resistance::monitors(app.layout.as_ref().unwrap())[0],
            Side::Right,
            Edit::Delete(0),
        );
        app.refresh.invalidate();
        app.section_edit(action);
        assert_eq!(
            document,
            lbm_app::settings::document(app.layout.as_ref().unwrap())
        );
    }

    #[cfg(unix)]
    #[test]
    fn another_launch_is_answered_without_waiting_for_a_frame_to_be_drawn() {
        let dir = tempfile::tempdir().unwrap();
        let lbm_app::instance::Launch::Open { owner, inbox, .. } =
            lbm_app::instance::start(lbm_app::instance::Address::in_directory(dir.path()), false)
                .unwrap()
        else {
            panic!("the first launch opens")
        };
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        app.instance = Some(inbox);
        let path = dir.path().to_owned();
        let second = std::thread::spawn(move || {
            lbm_app::instance::start(lbm_app::instance::Address::in_directory(&path), false)
        });
        // What eframe calls even for a minimized window: no `ui`, no painting.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !second.is_finished() {
            assert!(
                std::time::Instant::now() < deadline,
                "the activation was not answered"
            );
            app.instance_requests(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(matches!(
            second.join().unwrap().unwrap(),
            lbm_app::instance::Launch::Activated
        ));
        drop(owner);
    }

    #[test]
    fn a_short_window_still_shows_the_monitor_controls_of_display_lab() {
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        app.mode = lbm_ui::mode::Mode::Vcp;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480., 320.),
                )),
                ..Default::default()
            },
            |ui| app.properties(ui),
        );
        output.textures_delta.clear();
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("Connect to the agent to control the monitor."),
            "{text}"
        );
    }

    #[test]
    fn location_properties_load_outputs_and_keep_detached_ones_reachable() {
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        let (requests, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        app.requests = Some(requests);
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                // Tall enough that nothing is scrolled out of the painted shapes.
                egui::vec2(1100., 2400.),
            )),
            ..Default::default()
        };
        app.mode = lbm_ui::mode::Mode::Size;
        let mut output = ctx.run_ui(input(), |ui| app.properties(ui));
        output.textures_delta.clear();
        assert!(incoming.try_recv().is_err());
        app.mode = lbm_ui::mode::Mode::Location;
        let mut output = ctx.run_ui(input(), |ui| app.properties(ui));
        output.textures_delta.clear();
        let (id, method, _) = incoming.try_recv().unwrap();
        assert_eq!(method, "DisplayOutputs");
        let mut output = ctx.run_ui(input(), |ui| app.properties(ui));
        output.textures_delta.clear();
        assert!(
            incoming.try_recv().is_err(),
            "do not poll an outstanding read"
        );
        app.outputs.answered(
            id,
            &Ok(json!({"Signature":"live", "Outputs":[
                {"Connector":"DP-1", "Name":"Attached screen", "Enabled":true, "Primary":true},
                {"Connector":"HDMI-1", "Name":"Detached screen", "Enabled":false, "Primary":false}
            ]})),
        );
        assert!(app.selected.is_none());
        assert!(app.layout.is_none());
        let mut output = ctx.run_ui(input(), |ui| app.properties(ui));
        output.textures_delta.clear();
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        // No monitor of the layout stands for a detached output: nothing selects it.
        assert!(text.contains("Detached screen · HDMI-1"), "{text}");
        assert!(!text.contains("Attached screen · DP-1"), "{text}");
        let mut layout = Layout::new(LayoutOptions::default());
        lbm_layout::linux::populate(
            &mut layout,
            &[lbm_layout::linux::LinuxMonitor {
                connector_name: "DP-1".into(),
                ..lbm_layout::linux::LinuxMonitor::fallback()
            }],
            |_| Ok::<(), ()>(()),
        )
        .unwrap();
        app.selected = Some(layout.monitors()[0].id.clone());
        app.layout = Some(Box::new(layout));
        let mut output = ctx.run_ui(input(), |ui| app.properties(ui));
        output.textures_delta.clear();
        assert_eq!(app.outputs.panel.selected.as_deref(), Some("DP-1"));
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Attached screen · DP-1"), "{text}");
        assert!(text.contains("Detached screen · HDMI-1"), "{text}");
        assert!(
            incoming.try_recv().is_err(),
            "viewing the panel must never change an output"
        );
    }

    #[test]
    fn output_changes_wait_for_saved_edits_and_reload_after_their_own_answer() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        layout.id = "A".into();
        app.saved = Some(lbm_app::saved::Reference::of(&layout));
        app.layout = Some(Box::new(layout));
        let (requests, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        app.requests = Some(requests);
        app.agent.enabled = Some(false);
        app.agent.layout = Some(lbm_app::refresh::Stamp::from_snapshot(
            &json!({"LayoutId":"A", "LayoutRevision":1}),
        ));
        let inventory = json!({"Signature":"live", "Outputs":[{"Connector":"DP-1", "Name":"Main", "Enabled":true, "Primary":false}]});
        app.outputs.sent(99, false);
        app.refresh.complete(app.refresh.begin(false).unwrap(), "A");
        app.agent_said(Message::Answer {
            id: 99,
            result: Ok(inventory.clone()),
        });
        let action =
            lbm_ui::displays::Did::Change("DP-1".into(), lbm_ipc::displays::Change::Primary);
        app.layout
            .as_mut()
            .unwrap()
            .edit_options(|o| o.loop_x = true);
        app.output_action(action.clone());
        assert!(
            incoming.try_recv().is_err(),
            "dirty edits cannot cause an output change"
        );
        app.saved = Some(lbm_app::saved::Reference::of(app.layout.as_ref().unwrap()));
        assert_eq!(app.output_blocked(), None);
        app.output_action(action);
        let (id, method, request) = incoming.try_recv().unwrap();
        assert_eq!(method, "ChangeOutput");
        assert_eq!(request["Signature"], "live");
        assert_eq!(request["LayoutId"], "A");
        assert!(app.outputs.changing());
        app.state.waiting = true;
        app.agent_said(Message::Answer {
            id,
            result: Ok(inventory),
        });
        assert!(app.state.waiting, "output replies never settle Save/Start");
        assert!(!app.outputs.busy());
        assert!(
            app.refresh.pending(),
            "reread the desktop after an output action"
        );
    }

    #[test]
    fn a_probe_answer_does_not_acknowledge_an_outstanding_save() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        layout.id = "A".into();
        app.probe.want(
            Some(lbm_app::probe::Key::of(&layout, 1)),
            std::time::Instant::now(),
        );
        app.probe.sent(12);
        app.agent.asking(13, "SaveLayout");
        app.state.waiting = true;
        app.agent_said(Message::Answer {
            id: 12,
            result: Ok(json!({"LayoutId":"A", "LayoutRevision":1, "Zones":[]})),
        });
        assert!(
            app.state.waiting,
            "only the save's own answer can settle it"
        );
        assert!(!app.probe.waiting());
        app.agent_said(Message::Answer {
            id: 13,
            result: Ok(json!(null)),
        });
        assert!(!app.state.waiting);
    }

    /// #795 review: an agent predating `IntrinsicSize` transposes any model size that
    /// crosses the detected shape in the document it is sent, so a bezel edit alone must
    /// not send such a document to it; an agent announcing the capability gets it.
    #[test]
    fn a_document_an_old_agent_would_swap_is_not_sent_even_for_a_bezel_edit() {
        for (capabilities, goes_out) in [
            (Vec::new(), false),
            (vec!["IntrinsicSize".to_owned()], true),
        ] {
            let mut app = App::with_config(egui::Context::default(), None);
            let mut layout = Layout::new(LayoutOptions::default());
            lbm_layout::linux::populate(
                &mut layout,
                &[lbm_layout::linux::LinuxMonitor::fallback()],
                |_| Ok::<(), ()>(()),
            )
            .unwrap();
            layout.id = "A".into();
            app.detected = lbm_app::properties::Detected::of(&layout); // 527 x 296
            let model = layout.monitors()[0].model.clone();
            // Saved by an updated agent: taller than wide, as a PBP half is.
            layout.edit_model(&model, |s, _| {
                s.set_fixed_aspect_ratio(false);
                s.set_width(250.0);
            });
            app.saved = Some(lbm_app::saved::Reference::of(&layout));
            app.layout = Some(Box::new(layout));
            let (requests, mut receiver) = tokio::sync::mpsc::unbounded_channel();
            app.session_event(Event::Connected {
                requests,
                snapshot: json!({"LayoutId":"A", "LayoutRevision":1,
                    "DisplaySignature":"1920x1080", "Engine":"Running", "HookConnected":true}),
                capabilities: capabilities.clone(),
            });
            while receiver.try_recv().is_ok() {}

            app.layout.as_mut().unwrap().edit_model(&model, |s, _| {
                s.set_top_border(15.0);
            });
            app.ask(lbm_ui::Press::Save);
            app.dry_run_topology();

            let (mut saved, mut simulated) = (false, false);
            while let Ok((_, method, extra)) = receiver.try_recv() {
                saved |= method == "SaveLayout";
                simulated |= method == "ApplyTopology" && extra["DryRun"] == true;
            }
            assert_eq!(saved, goes_out, "{capabilities:?}");
            assert_eq!(simulated, goes_out, "System arrangement's Preview changes");
            assert_eq!(
                app.property_error.as_deref() == Some(lbm_app::properties::OLD_AGENT_WOULD_SWAP),
                !goes_out,
                "{capabilities:?}"
            );
        }
    }

    #[test]
    fn a_preference_changed_without_the_agent_is_replayed_and_survives_a_reload() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        layout.id = "A".into();
        app.saved = Some(lbm_app::saved::Reference::of(&layout));
        app.layout = Some(Box::new(layout));
        let snapshot = json!({"LayoutId":"A", "LayoutRevision":1,
            "DisplaySignature":"1920x1080", "Engine":"Running", "HookConnected":true});
        let (requests, _first) = tokio::sync::mpsc::unbounded_channel();
        app.session_event(Event::Connected {
            requests,
            snapshot: snapshot.clone(),
            capabilities: Vec::new(),
        });
        app.session_event(Event::Disconnected("agent restarted".into()));

        let before = app.layout.as_ref().unwrap().options.clone();
        let wanted = !before.start_minimized;
        app.layout
            .as_mut()
            .unwrap()
            .edit_options(|o| o.start_minimized = wanted);
        app.save_options(&before, false);
        assert!(!app.pending_options.is_empty());

        // What a fresh read of the store would give back must not undo the change.
        let mut reloaded = before.clone();
        app.pending_options.apply(&mut reloaded);
        assert_eq!(reloaded.start_minimized, wanted);

        let saves = |receiver: &mut tokio::sync::mpsc::UnboundedReceiver<_>| {
            let mut found = Vec::new();
            while let Ok((id, method, extra)) = receiver.try_recv() {
                let (id, method, extra): (u64, &str, serde_json::Value) = (id, method, extra);
                if method == "SaveOptions" {
                    found.push((id, extra));
                }
            }
            found
        };
        let (requests, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        app.session_event(Event::Connected {
            requests,
            snapshot: snapshot.clone(),
            capabilities: Vec::new(),
        });
        let sent = saves(&mut receiver);
        assert_eq!(sent.len(), 1);
        let (id, extra) = &sent[0];
        assert_eq!(extra["Options"], json!({"StartMinimized": wanted}));
        assert!(!app.pending_options.is_empty(), "not answered yet");

        app.session_event(Event::Message(Message::Answer {
            id: *id,
            result: Ok(serde_json::Value::Null),
        }));
        assert!(app.pending_options.is_empty());
        let (requests, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        app.session_event(Event::Connected {
            requests,
            snapshot,
            capabilities: Vec::new(),
        });
        assert!(saves(&mut receiver).is_empty(), "nothing is left to replay");
    }

    #[test]
    fn reconnect_keeps_the_edit_but_drops_live_mode_and_unacknowledged_saves() {
        let mut app = App::with_config(egui::Context::default(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        layout.id = "A".into();
        app.saved = Some(lbm_app::saved::Reference::of(&layout));
        app.layout = Some(Box::new(layout));
        let snapshot = json!({"LayoutId":"A", "LayoutRevision":1,
            "DisplaySignature":"1920x1080", "Engine":"Running", "HookConnected":true});
        let (requests, _receiver) = tokio::sync::mpsc::unbounded_channel();
        app.session_event(Event::Connected {
            requests,
            snapshot: snapshot.clone(),
            capabilities: Vec::new(),
        });
        app.refresh.complete(app.refresh.begin(false).unwrap(), "A");
        app.layout
            .as_mut()
            .unwrap()
            .edit_options(|o| o.loop_x = !o.loop_x);
        assert!(app.dirty());
        let reference = lbm_app::saved::Reference::of(app.layout.as_ref().unwrap());
        app.agent.asking(10, "SaveLayout");
        app.agent.earning(10, reference);
        app.state.live = true;
        app.state.waiting = true;

        app.session_event(Event::Disconnected("agent restarted".into()));
        assert!(app.dirty());
        assert!(!app.state.live);
        assert!(!app.state.waiting);
        let (requests, _receiver) = tokio::sync::mpsc::unbounded_channel();
        app.session_event(Event::Connected {
            requests,
            snapshot,
            capabilities: Vec::new(),
        });
        assert!(app.dirty());
        assert!(
            !app.refresh.pending(),
            "same desktop: the edit must remain usable"
        );
        assert!(app.state.hook_connected);
        assert!(
            !app.state.live,
            "reconnecting must not restart an unsaved preview"
        );
        assert!(app.agent.earned().is_none());

        app.session_event(Event::Message(Message::State(json!({
            "LayoutId":"A", "LayoutRevision":2, "DisplaySignature":"3840x2160",
            "Engine":"Running", "HookConnected":true
        }))));
        assert!(app.refresh.pending());
        assert_eq!(app.refresh.begin(app.dirty()), None);
        assert!(app.dirty(), "changing resolution must not discard the edit");
    }
    #[test]
    fn desktop_wallpaper_is_visible_without_saved_settings_and_preview_stays_in_its_editor() {
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        app.screens.push(Screen {
            id: "screen".into(),
            source_id: "output".into(),
            name: "Monitor".into(),
            logo: None,
            mm_outside: Rect::new(0., 0., 600., 340.),
            mm_content: Rect::new(0., 0., 600., 340.),
            pixels: Rect::new(0., 0., 1920., 1080.),
            details: vec![],
        });
        let texture = |name, color| {
            ctx.load_texture(
                name,
                egui::ColorImage::filled([2, 2], color),
                Default::default(),
            )
        };
        let desktop = texture("desktop", egui::Color32::BLUE);
        let preview = texture("preview", egui::Color32::RED);
        app.desktop_textures
            .insert("screen".into(), desktop.clone());
        app.wallpapers.insert("screen".into(), preview.clone());
        let logos = HashMap::new();
        assert!(app.wallpaper.draft.is_none());
        assert!(app.requests.is_none());
        for mode in [
            lbm_ui::mode::Mode::Default,
            lbm_ui::mode::Mode::Location,
            lbm_ui::mode::Mode::Resistance,
        ] {
            app.mode = mode;
            assert_eq!(
                app.monitors(&logos)[0].wallpaper.unwrap().id(),
                desktop.id()
            );
        }
        app.mode = lbm_ui::mode::Mode::Wallpaper;
        assert_eq!(
            app.monitors(&logos)[0].wallpaper.unwrap().id(),
            preview.id()
        );
        app.wallpapers.clear();
        assert_eq!(
            app.monitors(&logos)[0].wallpaper.unwrap().id(),
            desktop.id()
        );
    }
    #[test]
    fn wallpaper_requests_belong_to_the_editor_and_reconnect_keeps_the_draft() {
        let ctx = egui::Context::default();
        let mut app = App::with_config(ctx.clone(), None);
        let mut layout = Layout::new(LayoutOptions::default());
        layout.id = "A".into();
        app.layout = Some(Box::new(layout));
        app.agent.layout = Some(lbm_app::refresh::Stamp::from_snapshot(
            &json!({"LayoutId":"A","LayoutRevision":1}),
        ));
        let generation = app.refresh.begin(false).unwrap();
        assert!(app.refresh.complete(generation, "A"));
        let (requests, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        app.requests = Some(requests);
        app.sync_wallpaper(&ctx);
        let (id, method, extra) = incoming.try_recv().unwrap();
        assert_eq!(method, "WallpaperSettings");
        assert_eq!(extra["LayoutRevision"], 1);
        app.sync_wallpaper(&ctx);
        assert!(incoming.try_recv().is_err());
        app.agent_said(Message::Answer{id,result:Ok(json!({"Settings":lbm_store::wallpaper_settings::LayoutWallpaperSettings::default(),"Supported":true}))});
        assert!(app.wallpaper.supported);
        assert!(app.wallpaper_preview_requested);
        assert!(!app.state.waiting);
        app.wallpaper.draft.as_mut().unwrap().mode =
            lbm_store::wallpaper_settings::WallpaperMode::Span;
        app.wallpaper_request(false);
        let (id, method, _) = incoming.try_recv().unwrap();
        assert_eq!(method, "ApplyWallpaper");
        app.session_event(Event::Disconnected("test disconnect".into()));
        app.agent_said(Message::Answer {
            id,
            result: Ok(json!({"Saved":true,"Applied":true})),
        });
        assert!(app.wallpaper.dirty());
        assert!(app.wallpaper.stale);
        assert!(!app.wallpaper.busy());
    }
    #[test]
    fn choosing_an_image_requests_only_a_local_preview() {
        let mut app = App::with_config(egui::Context::default(), None);
        let (requests, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        app.requests = Some(requests);
        app.mode = lbm_ui::mode::Mode::Wallpaper;
        app.wallpaper.draft = Some(lbm_store::wallpaper_settings::LayoutWallpaperSettings {
            mode: lbm_store::wallpaper_settings::WallpaperMode::Span,
            ..Default::default()
        });
        let choice = app.wallpaper.choose_file(None).unwrap();
        app.wallpaper_picking = true;
        app.wallpaper_file_chosen(choice, Ok(Some("/images/chosen.png".into())));
        assert!(!app.wallpaper_picking);
        assert!(app.wallpaper_preview_requested);
        assert!(
            incoming.try_recv().is_err(),
            "choosing a path never sends Save or Apply"
        );
        app.wallpaper_preview_requested = false;
        let choice = app.wallpaper.choose_file(None).unwrap();
        app.settings_open = true;
        app.wallpaper_picking = true;
        app.wallpaper_file_chosen(choice, Ok(Some("/images/late.png".into())));
        assert!(!app.wallpaper_picking);
        assert!(!app.wallpaper_preview_requested);
        assert_eq!(
            app.wallpaper
                .draft
                .as_ref()
                .unwrap()
                .span_image_path
                .as_deref(),
            Some("/images/chosen.png")
        );
    }
    fn per_screen_app() -> App {
        use lbm_store::wallpaper_settings::{
            LayoutWallpaperSettings, ScreenWallpaperKind, ScreenWallpaperSettings, WallpaperMode,
        };
        let mut app = App::with_config(egui::Context::default(), None);
        app.mode = lbm_ui::mode::Mode::Wallpaper;
        let mut draft = LayoutWallpaperSettings {
            mode: WallpaperMode::PerScreen,
            ..Default::default()
        };
        for id in ["A", "B"] {
            let screen = ScreenWallpaperSettings {
                kind: ScreenWallpaperKind::Image,
                ..Default::default()
            };
            draft.per_screen.insert(id.into(), screen);
        }
        app.wallpaper.draft = Some(draft);
        app.selected = Some("A".into());
        app
    }
    fn image_of(app: &App, id: &str) -> Option<String> {
        app.wallpaper.draft.as_ref().unwrap().per_screen[id]
            .image_path
            .clone()
    }
    #[test]
    fn leaving_wallpaper_and_coming_back_before_the_chooser_answers_retires_it() {
        let mut app = per_screen_app();
        let choice = app.begin_file_choice().unwrap();
        app.mode = lbm_ui::mode::Mode::default();
        app.retire_departed_file_choice(); // the pass that shows the other mode
        app.mode = lbm_ui::mode::Mode::Wallpaper;
        app.retire_departed_file_choice();
        app.wallpaper_file_chosen(choice, Ok(Some("/images/late.png".into())));
        assert!(!app.wallpaper_picking);
        assert!(!app.wallpaper_preview_requested);
        assert_eq!(image_of(&app, "A"), None);

        // The same round trip through another view, and through the settings drawer.
        let choice = app.begin_file_choice().unwrap();
        app.view = View::List;
        app.retire_departed_file_choice();
        app.view = View::Map;
        app.wallpaper_file_chosen(choice, Ok(Some("/images/late.png".into())));
        assert_eq!(image_of(&app, "A"), None);
        let choice = app.begin_file_choice().unwrap();
        app.settings_open = true;
        app.retire_departed_file_choice();
        app.settings_open = false;
        app.wallpaper_file_chosen(choice, Ok(Some("/images/late.png".into())));
        assert_eq!(image_of(&app, "A"), None);

        // Staying put, the answer still lands.
        let choice = app.begin_file_choice().unwrap();
        app.retire_departed_file_choice();
        app.wallpaper_file_chosen(choice, Ok(Some("/images/kept.png".into())));
        assert!(app.wallpaper_preview_requested);
        assert_eq!(image_of(&app, "A").as_deref(), Some("/images/kept.png"));
    }
    #[test]
    fn selecting_another_screen_and_back_before_the_chooser_answers_retires_it() {
        let mut app = per_screen_app();
        let choice = app.begin_file_choice().unwrap();
        app.selected = Some("B".into());
        app.retire_departed_file_choice();
        app.selected = Some("A".into());
        app.retire_departed_file_choice();
        app.wallpaper_file_chosen(choice, Ok(Some("/images/late.png".into())));
        assert!(!app.wallpaper_preview_requested);
        assert_eq!(image_of(&app, "A"), None);
        assert_eq!(image_of(&app, "B"), None);

        // A departure in the same pass as the answer is caught when it lands.
        let choice = app.begin_file_choice().unwrap();
        app.selected = Some("B".into());
        app.wallpaper_file_chosen(choice, Ok(Some("/images/late.png".into())));
        assert_eq!(image_of(&app, "B"), None);
    }
    #[test]
    fn a_new_agent_revision_retires_file_choices_before_the_next_frame() {
        let mut app = App::with_config(egui::Context::default(), None);
        app.mode = lbm_ui::mode::Mode::Wallpaper;
        let key = lbm_app::wallpaper_editor::Key {
            layout: "A".into(),
            revision: 1,
        };
        app.wallpaper.read(50, key);
        app.wallpaper.answered(
            50,
            &Ok(json!({"Settings":{"Mode":"Span"},"Supported":true})),
        );
        let choice = app.wallpaper.choose_file(None).unwrap();
        app.agent_said(Message::State(json!({"LayoutId":"A","LayoutRevision":2})));
        app.wallpaper_file_chosen(choice, Ok(Some("/images/obsolete.png".into())));
        assert!(app.wallpaper.stale);
        assert_eq!(app.wallpaper.draft.as_ref().unwrap().span_image_path, None);
        assert!(!app.wallpaper_preview_requested);
    }
}
