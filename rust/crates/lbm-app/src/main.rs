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
    Thumbnail {
        generation: u64,
        screen: String,
        picture: image::RgbaImage,
    },
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
    Settings,
    Displays,
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
    refresh: lbm_app::refresh::Refresh,
    detecting: Option<u64>,
    retry_display_at: std::time::Instant,
    display_error: Option<String>,
    /// Everything the agent has said, and the requests still out.
    agent: lbm_app::conversation::Conversation,
    probe: lbm_app::probe::Probe,
    show_probe: bool,
    outputs: lbm_app::displays::Displays,
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
    /// The model sizes as detected, which the size editor must not contradict.
    detected: lbm_app::properties::Detected,
    property_error: Option<String>,
    resistance: lbm_ui::resistance::Editor,
    screen_overlay: lbm_overlay::Overlay,
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
    adjust_scale: bool,
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
    /// What every frame shows inside it — the mode bar's choice. See `lbm_ui::mode`.
    mode: lbm_ui::mode::Mode,
    icons: Catalogue,
    /// Uploaded logos, by icon path. Loaded once, on the frame that first needs one.
    textures: HashMap<String, egui::TextureHandle>,
    /// Uploaded wallpaper thumbnails, by screen id.
    wallpapers: HashMap<String, egui::TextureHandle>,
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
    Some((Box::new(layout), reports, detected))
}

/// The stored profile onto the layout in memory: the app-wide options, the layout's own
/// options, and every monitor's saved place and model.
///
/// **Read, never written.** The frontend reads files the agent owns, exactly as it
/// already reads `wallpaper.json` for the thumbnails; writing goes back through the
/// agent, which stays the only writer.
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
        // C# `AboutMonitorView`: this app's version, and what identifies the screen.
        Mode::About => {
            let mut rows = vec![
                ("version", env!("CARGO_PKG_VERSION").to_owned()),
                ("id", monitor.id.clone()),
            ];
            if let Some(serial) = &monitor.serial_number {
                rows.push(("serial", serial.clone()));
            }
            rows
        }
        // These modes use the selected-monitor properties panel instead of map rows.
        Mode::Info | Mode::Size | Mode::Resistance => Vec::new(),
        // Not drawn by this window; the bar does not offer them.
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

/// The thumbnails for one layout's screens, in the order they will be wanted.
///
/// Read-only, from the file the agent owns (`wallpaper.json`) and the C# frontend reads
/// the same way — `WallpaperSettings.FilePath`, and the wire test that says outright
/// "the agent writes what it is sent into wallpaper.json, and this side reads that".
///
/// Everything is done in quarter-pixels, as `MonitorFrameViewModel.cs:176-201` does:
/// the screen's rectangle and the desktop's are divided by four and so is the picture,
/// which is what keeps a wall of 4K wallpapers out of memory. A screen smaller than the
/// divisor gets nothing, as there too.
fn wallpapers(layout_id: &str, screens: &[Screen]) -> Vec<(String, image::RgbaImage)> {
    use lbm_store::wallpaper_settings::{self, ScreenWallpaperKind, WallpaperMode};

    if layout_id.is_empty() {
        return Vec::new();
    }
    let all = wallpaper_settings::load(&wallpaper_settings::settings_path());
    let Some(settings) = all.get(layout_id) else {
        return Vec::new();
    };

    let shrink = lbm_ui::wallpaper::SHRINK;
    let quarter = |r: Rect| lbm_ui::wallpaper::Rect {
        x: (r.left() as i32) / shrink,
        y: (r.top() as i32) / shrink,
        w: (r.width() as i32) / shrink,
        h: (r.height() as i32) / shrink,
    };
    // The desktop the spanned styles are cut out of: every screen's pixels together.
    let desktop = screens
        .iter()
        .fold(None::<lbm_ui::wallpaper::Rect>, |box_, s| {
            let r = quarter(s.pixels);
            Some(match box_ {
                None => r,
                Some(b) => {
                    let left = b.x.min(r.x);
                    let top = b.y.min(r.y);
                    lbm_ui::wallpaper::Rect {
                        x: left,
                        y: top,
                        w: (b.x + b.w).max(r.x + r.w) - left,
                        h: (b.y + b.h).max(r.y + r.h) - top,
                    }
                }
            })
        });

    let mut made = Vec::new();
    for screen in screens {
        if screen.pixels.width() < shrink as f64 || screen.pixels.height() < shrink as f64 {
            continue;
        }
        let here = quarter(screen.pixels);

        let (path, style, colour) = match settings.mode {
            WallpaperMode::Span => {
                let Some(path) = settings.span_image_path.as_ref().filter(|p| !p.is_empty()) else {
                    continue;
                };
                (path.clone(), lbm_ui::wallpaper::Style::Span, [0, 0, 0, 255])
            }
            WallpaperMode::PerScreen => {
                let Some(wanted) = settings.per_screen.get(&screen.id) else {
                    continue;
                };
                if wanted.kind == ScreenWallpaperKind::Color {
                    // A colour is not a picture, and painting one into a texture to
                    // show a flat rectangle would be work for nothing; the frame's own
                    // background is already a flat rectangle. Left for when the frame
                    // learns to take a colour.
                    continue;
                }
                let Some(path) = wanted.image_path.as_ref().filter(|p| !p.is_empty()) else {
                    continue;
                };
                (
                    path.clone(),
                    style_of(wanted.style),
                    colour_of(&wanted.color),
                )
            }
        };

        // A picture that has been moved or deleted is not an error worth shouting
        // about: the desktop keeps what it had, and so does the map.
        let Ok(decoded) = image::open(&path) else {
            continue;
        };
        let full = decoded.to_rgba8();
        let source = lbm_ui::wallpaper::Size {
            w: full.width() as i32,
            h: full.height() as i32,
        };
        let Some(small) = lbm_ui::wallpaper::shrunk(source, shrink) else {
            continue;
        };
        let picture = lbm_ui::wallpaper::apply(
            full,
            &[lbm_ui::wallpaper::Step::Resize(small)],
            image::Rgba(colour),
        );

        let steps = match style {
            lbm_ui::wallpaper::Style::Span | lbm_ui::wallpaper::Style::Tile => {
                let Some(desktop) = desktop else { continue };
                lbm_ui::wallpaper::across_the_desktop(style, small, here, desktop)
            }
            _ => lbm_ui::wallpaper::on_one_screen(style, small, here.size()),
        };
        made.push((
            screen.id.clone(),
            lbm_ui::wallpaper::apply(picture, &steps, image::Rgba(colour)),
        ));
    }
    made
}

/// The store's style, as the view names it. Exhaustive on purpose: a style added to the
/// store has to be answered for here rather than quietly drawn as something else.
fn style_of(style: lbm_store::wallpaper_settings::WallpaperStyle) -> lbm_ui::wallpaper::Style {
    use lbm_store::wallpaper_settings::WallpaperStyle as Stored;
    use lbm_ui::wallpaper::Style;
    match style {
        Stored::Fill => Style::Fill,
        Stored::Fit => Style::Fit,
        Stored::Stretch => Style::Stretch,
        Stored::Tile => Style::Tile,
        Stored::Center => Style::Center,
        Stored::Span => Style::Span,
    }
}

/// `#RRGGBB`, and black for anything else — a wallpaper is never a reason to stop.
fn colour_of(text: &str) -> [u8; 4] {
    let hex = text.trim_start_matches('#');
    if hex.len() != 6 {
        return [0, 0, 0, 255];
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
    [byte(0), byte(2), byte(4), 255]
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        Self::with_config(ctx, lbm_app::session::Config::for_session())
    }

    fn with_config(ctx: egui::Context, config: Option<lbm_app::session::Config>) -> Self {
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
            property_error: None,
            resistance: lbm_ui::resistance::Editor::default(),
            screen_overlay: Default::default(),
            screen_overlay_mode: Default::default(),
            screen_overlay_error: None,
            next_screen_overlay: std::time::Instant::now(),
            selected: None,
            drag: None,
            agent: lbm_app::conversation::Conversation::default(),
            probe: lbm_app::probe::Probe::default(),
            show_probe: true,
            outputs: Default::default(),
            excluded: None,
            pattern: String::new(),
            adjust_scale: false,
            recording: lbm_ui::shortcut::Recording::default(),
            previewed: None,
            view: View::Map,
            mode: lbm_ui::mode::Mode::default(),
            icons,
            textures: HashMap::new(),
            wallpapers: HashMap::new(),
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
            Event::Connected { requests, snapshot } => {
                self.outputs = Default::default();
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
            }
            Event::Message(message) => self.agent_said(message),
            Event::Disconnected(why) => {
                self.screen_overlay_mode = lbm_overlay::Mode::Off;
                self.screen_overlay.clear();
                self.outputs = Default::default();
                self.probe = lbm_app::probe::Probe::default();
                self.requests = None;
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
            let id = layout.id.clone();
            let screens = screens_of(&layout, lbm_ui::mode::Mode::default());
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
            for (screen, picture) in wallpapers(&id, &screens) {
                if found
                    .send(Found::Thumbnail {
                        generation,
                        screen,
                        picture,
                    })
                    .is_err()
                {
                    return;
                }
                waker.request_repaint();
            }
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
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let (method, mut extra) = lbm_app::settings::apply_topology(layout, self.adjust_scale);
        extra["LayoutRevision"] =
            serde_json::json!(self.agent.layout.as_ref().and_then(|s| s.revision));
        self.send(method, extra);
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
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        self.agent.topology = None;
        let (method, mut extra) = lbm_app::settings::dry_run_topology(layout, self.adjust_scale);
        extra["LayoutRevision"] =
            serde_json::json!(self.agent.layout.as_ref().and_then(|s| s.revision));
        self.send(method, extra);
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
        if !self.state.live {
            return;
        }
        // Without this the window only redraws when something happens, so an edit made
        // by a key repeat or by the agent would wait for the next stray repaint.
        ctx.request_repaint_after(lbm_app::settings::PREVIEW_INTERVAL);

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
    fn save_options(&mut self) {
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        // A new shortcut makes the hook's old verdict meaningless: it was about the one
        // before. Cleared here rather than in the panel, because this is the moment the
        // change actually leaves — the hook will say again if the new one is no better.
        self.agent.shortcut_changed();
        let (method, extra) =
            lbm_app::settings::save_options(&layout.options, self.excluded.as_deref());
        self.send(method, extra);
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
            if self.refresh.observe(stamp, false, self.dirty()) {
                self.outputs.invalidate();
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
        const SIDE: u32 = 128;
        let pixels = lbm_icons::render(&tree, SIDE)?;
        let image = egui::ColorImage::from_rgba_unmultiplied([SIDE as usize; 2], &pixels);
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
    /// same `VcpControl` the C# button follows. `wallpaper_supported` is `false` until
    /// the agent is asked: only it knows whether this desktop has a wallpaper service,
    /// and a window that guessed would offer a picker that does nothing.
    fn offered(&self) -> lbm_ui::mode::Offered {
        lbm_ui::mode::Offered {
            vcp_enabled: self
                .layout
                .as_deref()
                .is_some_and(|l| l.options.vcp_control),
            wallpaper_supported: false,
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
                wallpaper: self.wallpapers.get(&s.id),
                logo: s
                    .logo
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .and_then(|p| logos.get(p)),
                details: &s.details,
                probe: if self.mode == lbm_ui::mode::Mode::Resistance {
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

    fn properties(&mut self, ui: &mut egui::Ui) {
        if !matches!(self.view, View::Map | View::List)
            || !matches!(
                self.mode,
                lbm_ui::mode::Mode::Size
                    | lbm_ui::mode::Mode::Info
                    | lbm_ui::mode::Mode::Resistance
            )
        {
            return;
        }
        lbm_ui::properties::panel(ui, |ui| {
            ui.heading(self.mode.label());
            let Some(layout) = &self.layout else {
                ui.label("Waiting for displays…");
                return;
            };
            let previous = self.selected.clone();
            egui::ComboBox::from_id_salt("properties-monitor")
                .selected_text(self.selected.as_deref().unwrap_or("Select a monitor"))
                .show_ui(ui, |ui| {
                    for monitor in layout.monitors() {
                        let name = layout
                            .model(&monitor.model)
                            .and_then(|m| m.pnp_device_name.as_deref())
                            .unwrap_or(&monitor.id);
                        ui.selectable_value(
                            &mut self.selected,
                            Some(monitor.id.clone()),
                            format!("{name} · {}", monitor.id),
                        );
                    }
                });
            if previous != self.selected {
                self.resistance.clear();
                self.property_error = None;
            }
            let Some(id) = self.selected.clone() else {
                ui.label("Select a screen on the map, in the list or above.");
                return;
            };
            let blocked = self.refresh.pending() || self.outputs.changing() || self.drag.is_some();
            let mut edits = Vec::new();
            let mut section_edit = None;
            ui.push_id(("properties", &id), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("monitor-properties-scroll")
                    .show(ui, |ui| {
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
                            ui.heading("Detected · last display refresh");
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
                            ui.heading("Current layout");
                            lbm_ui::properties::rows(
                                ui,
                                &lbm_app::properties::layout_info(layout, &id),
                            );
                            return;
                        }
                        let Some(size) = lbm_app::properties::size(layout, &id) else {
                            ui.label("No active source for this monitor.");
                            return;
                        };
                        edits = ui
                            .add_enabled_ui(!blocked && !layout.is_virtual(), |ui| {
                                lbm_ui::properties::size(ui, &size)
                            })
                            .inner;
                    });
            });
            if let Some(action) = section_edit {
                self.section_edit(action);
            }
            for edit in edits {
                if let Some(layout) = self.layout.as_mut() {
                    match lbm_app::properties::apply(layout, &self.detected, &id, edit) {
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
        });
    }

    fn section_edit(&mut self, action: lbm_ui::resistance::Action) {
        if self.refresh.pending() || self.outputs.changing() {
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
        if !matches!(self.view, View::Map | View::List) {
            return;
        }
        egui::Panel::top("screen-overlays").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let available = lbm_overlay::SUPPORTED
                    && self.requests.is_some()
                    && !self.refresh.pending()
                    && !self.outputs.changing()
                    && !self.agent.suspended
                    && self.layout.as_ref().is_some_and(|l| !l.is_virtual());
                for (mode, label, enabled) in [
                    (Mode::Rulers, "Rulers on screens", self.selected.is_some()),
                    (
                        Mode::Resistance,
                        "Resistance on screens",
                        self.mode == lbm_ui::mode::Mode::Resistance,
                    ),
                ] {
                    if ui
                        .add_enabled(
                            available && enabled,
                            egui::Button::new(label).selected(self.screen_overlay_mode == mode),
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
                }
                if !lbm_overlay::SUPPORTED {
                    ui.small("Screen overlays are unavailable on this platform.");
                }
                if self.screen_overlay_mode != Mode::Off {
                    ui.small(if self.screen_overlay_mode == Mode::Rulers {
                        "Millimetres · large numbers: decimetres · click-through · Esc to hide"
                    } else {
                        "Edited resistance sections · click-through · Esc to hide"
                    });
                }
                if let Some(error) = &self.screen_overlay_error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        });
    }

    fn screen_overlay_tick(&mut self, ctx: &egui::Context, movement: Option<(f64, f64)>) {
        use lbm_overlay::Mode;
        if self.requests.is_none()
            || self.refresh.pending()
            || self.outputs.changing()
            || self.agent.suspended
            || !matches!(self.view, View::Map | View::List)
            || self.layout.as_ref().is_none_or(|l| l.is_virtual())
            || ctx
                .input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().minimized == Some(true))
        {
            self.screen_overlay_mode = Mode::Off;
        }
        if self.screen_overlay_mode == Mode::Off {
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
        let mut drawn = self.layout.as_deref().unwrap().clone();
        if let Some(action) = self.resistance.preview_action() {
            let _ = lbm_app::resistance::apply(&mut drawn, action);
        }
        if let (Some(drag), Some((x, y))) = (&self.drag, movement) {
            drawn.offset_location(&drag.id, lbm_layout::geo::Vector::new(x, y));
        }
        let result = lbm_overlay::scene::from_layout(
            &drawn,
            self.screen_overlay_mode,
            self.selected.as_deref(),
        )
        .and_then(|bands| self.screen_overlay.sync(&bands));
        if let Err(error) = result {
            self.screen_overlay.clear();
            self.screen_overlay_mode = Mode::Off;
            self.screen_overlay_error = Some(error);
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
                        layout,
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
                        self.saved = Some(lbm_app::saved::Reference::of(&layout));
                        self.screens = screens_of(&layout, self.mode);
                        self.reports = reports;
                        self.detected = detected;
                        self.resistance.clear();
                        self.property_error = None;
                        self.layout = Some(layout);
                        self.excluded = excluded;
                        self.wallpapers.clear();
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
                    Found::Thumbnail {
                        generation,
                        screen,
                        picture,
                    } => {
                        if !self.refresh.accepts(generation) || self.refresh.pending() {
                            continue;
                        }
                        let size = [picture.width() as usize, picture.height() as usize];
                        let image = egui::ColorImage::from_rgba_unmultiplied(size, &picture);
                        self.wallpapers.insert(
                            screen.clone(),
                            ctx.load_texture(
                                format!("wallpaper:{screen}"),
                                image,
                                egui::TextureOptions::LINEAR,
                            ),
                        );
                    }
                }
            }
        }

        self.read_displays(&ctx, false);
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
        let previous_mode = self.mode;
        egui::Panel::top("modes").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, View::Map, "Map");
                ui.selectable_value(&mut self.view, View::List, "List");
                ui.selectable_value(&mut self.view, View::Displays, "Displays");
                ui.add_enabled(self.mode != lbm_ui::mode::Mode::Resistance, egui::Checkbox::new(&mut self.show_probe, "Show crossings"))
                    .on_disabled_hover_text("Switch away from Border resistance to show the geometric crossing report.")
                    .on_hover_text("Green: a passage. Red: a wall. Resistance is bypassed for this geometric report. Probing does not start or change the mouse engine.");
                // Asked when the panel is opened rather than on a timer: the list only
                // grows, and nobody is looking at it the rest of the time.
                if ui
                    .selectable_value(&mut self.view, View::Settings, "Settings")
                    .clicked()
                {
                    self.send("SeenProcesses", serde_json::json!({}));
                }
                ui.separator();
                ui.label(match self.screens.len() {
                    0 => "no screens detected".to_owned(),
                    1 => "1 screen".to_owned(),
                    n => format!("{n} screens"),
                });
                // What the frames show inside them — a second axis over Map/List, as in
                // C# where `ContentViewMode` and the list toggle are separate. Not on the
                // settings view, which has no frames to write in.
                if matches!(self.view, View::Map | View::List) {
                    ui.separator();
                    if let Some(chosen) = lbm_ui::mode::bar(ui, self.mode, &self.offered()) {
                        self.mode = chosen;
                        self.drag = None;
                        // The rows are built with the screens, so changing the mode is
                        // reading them off the layout again.
                        if let Some(layout) = self.layout.as_deref() {
                            self.screens = screens_of(layout, chosen);
                        }
                    }
                }
                ui.separator();
                // Said plainly. A window that looked the same with and without an agent
                // would leave the user guessing why the buttons do nothing.
                match (&self.without_agent, &self.agent.refused) {
                    (Some((why, where_)), _) => ui
                        .label(format!("no agent — {why}"))
                        .on_hover_text(where_.clone()),
                    // A request the agent turned down used to vanish without a word.
                    (None, Some(why)) => ui.colored_label(ui.visuals().error_fg_color, why.clone()),
                    (None, None) => ui.label("agent connected"),
                };
            });
        });

        if previous_view != self.view || previous_mode != self.mode || self.refresh.pending() {
            self.resistance.cancel();
            self.screen_overlay_mode = lbm_overlay::Mode::Off;
            self.screen_overlay.clear();
        }
        self.screen_overlay_controls(ui);
        self.probe_tick(&ctx);
        if self.show_probe
            && self.mode != lbm_ui::mode::Mode::Resistance
            && matches!(self.view, View::Map | View::List)
        {
            egui::Panel::top("probe-status").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Green: passage · Red: wall · Resistance bypassed");
                    if let Some(error) = self.probe.error.clone() {
                        ui.label(format!("Crossings unavailable: {error}"));
                        if ui.button("Retry crossings").clicked() {
                            self.probe.retry(std::time::Instant::now());
                            ctx.request_repaint();
                        }
                    } else if self.probe.waiting() {
                        ui.label("Checking crossings…");
                    } else if self.requests.is_none() {
                        ui.label("Connect to the agent to show crossings");
                    } else if self
                        .agent
                        .layout
                        .as_ref()
                        .is_some_and(|s| s.revision.is_none())
                    {
                        ui.label("Update the agent to show crossings");
                    }
                });
            });
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

        egui::Panel::bottom("controls").show(ui, |ui| {
            if self.refresh.pending() || self.outputs.changing() {
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
            if let Some(lbm_ui::Action::Pressed(press)) = lbm_ui::bottom_bar(ui, &self.state) {
                for effect in lbm_ui::update(&mut self.state, lbm_ui::Action::Pressed(press)) {
                    match effect {
                        lbm_ui::Effect::Ask(press) => self.ask(press),
                        lbm_ui::Effect::EndPreview => self.end_preview(),
                    }
                }
            }
        });

        self.properties(ui);

        // The `Ui` handed to `App::ui` has no background of its own — the doc says so
        // outright — so the map would be drawn on nothing.
        let mut save_options = false;
        let mut excluded_did = None;
        let mut apply = false;
        let mut dry_run = false;
        let mut adjust_scale = self.adjust_scale;
        let topology_needs_stop = cfg!(windows)
            && (self.agent.enabled != Some(false)
                || self.agent.previewing
                || self.agent.suspended
                || matches!(
                    self.state.engine,
                    lbm_ui::Engine::Running | lbm_ui::Engine::Paused
                ));
        if self.view == View::Displays && self.requests.is_some() && self.outputs.needs_read() {
            self.output_request("DisplayOutputs", serde_json::json!({}), false);
        }
        let mut output_action = None;
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
        let acted = egui::CentralPanel::default()
            .show(ui, |ui| {
                if self.view == View::Displays {
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
                    output_action = self.outputs.panel.draw(
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
                    return (None, None);
                }
                if self.refresh.pending() || self.outputs.changing() {
                    ui.disable();
                }
                if self.view == View::Settings {
                    let mut recording = self.recording;
                    let unavailable = self.agent.shortcut_unavailable.clone();
                    match self.layout.as_mut() {
                        // `edit_options` is what marks the layout unsaved and what
                        // republishes the extent when the border values move; a panel
                        // writing round it would leave both wrong. The per-layout half
                        // needs no more than that — being unsaved *is* what Save reads.
                        Some(layout) => {
                            layout.edit_options(|o| {
                                save_options = lbm_ui::options::panel(
                                    ui,
                                    o,
                                    cfg!(windows),
                                    &mut recording,
                                    unavailable.as_deref(),
                                )
                                .app;
                            });
                            // Outside `edit_options`: the excluded list is not one of the
                            // layout's options here — the window keeps it apart precisely
                            // because it may not have read it, and `LayoutOptions` has no
                            // way to say "not read".
                            excluded_did = lbm_ui::excluded::panel(
                                ui,
                                self.excluded.as_deref(),
                                &mut self.pattern,
                                &self.agent.seen,
                            );
                        }
                        None => {
                            ui.label("the displays have not been read yet");
                        }
                    }
                    self.recording = recording;
                    return (None, None);
                }
                let monitors = self.monitors(&logos);
                let at = ui.max_rect();
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

                        let gesture = map::draw(ui, &shown, &fit, self.selected.as_deref());
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
                        // The presenter's context menu, as the C# has it. Right-click on
                        // the map, not a button in the bar: this is the one action in the
                        // window that moves the user's real screens, and it should take
                        // some finding rather than sit next to Save.
                        ui.response().context_menu(|ui| {
                            ui.label("Apply this layout to the system configuration?");
                            ui.label(
                                egui::RichText::new(
                                    "Your screens will move. The layout is saved first, \
                                     because the system change reloads the saved one.",
                                )
                                .small()
                                .color(ui.visuals().weak_text_color()),
                            );
                            if cfg!(windows) {
                                adjust_scale = false;
                                ui.label("Windows keeps the current display scales.");
                                if topology_needs_stop {
                                    ui.label(
                                        "Stop the engine and end Live preview before applying.",
                                    );
                                }
                            } else {
                                ui.checkbox(&mut adjust_scale, "Also adjust the scales");
                            }
                            ui.separator();
                            // Offered **above** Apply, and on purpose: this is the one
                            // action in the window that cannot be undone, and reading
                            // the commands costs a click and changes nothing.
                            if ui
                                .add_enabled(
                                    !topology_needs_stop,
                                    egui::Button::new("Show what it would run"),
                                )
                                .clicked()
                            {
                                dry_run = true;
                                ui.close();
                            }
                            if ui
                                .add_enabled(!topology_needs_stop, egui::Button::new("Apply"))
                                .clicked()
                            {
                                apply = true;
                                ui.close();
                            }
                            if ui.button("Cancel").clicked() {
                                ui.close();
                            }
                        });
                        // After the frames: the lines are about the screens, so they are
                        // read over them. The C# adds its canvas to the panel the frames
                        // are already in, which puts it on top the same way.
                        if let Some((_, _, snap)) = &held {
                            drag::draw(ui, &fit, snap);
                        }
                        (owned(gesture), held.map(|(_, total, _)| total))
                    }
                    View::List => {
                        let picked = list::draw(ui, at, &monitors, self.selected.as_deref());
                        if resistance_mode {
                            if self.layout.as_ref().is_none_or(|l| l.is_virtual()) {
                                ui.disable();
                            }
                            let placements = monitors
                                .iter()
                                .find(|m| Some(m.id) == self.selected.as_deref())
                                .map(|m| {
                                    let fit = map::fit_in(list::panes(at).screen, m.mm_outside);
                                    vec![(m.id.to_owned(), fit.place(m).content)]
                                })
                                .unwrap_or_default();
                            let interaction =
                                section_editor.bands(ui, &section_monitors, &placements);
                            section_action = interaction.action;
                        }
                        (picked.map(|id| Did::Clicked(id.to_owned())), None)
                    }
                    // Returned above, before the screens were borrowed: the panel edits
                    // the very options they were read from.
                    View::Settings | View::Displays => {
                        unreachable!("handled before the monitors are built")
                    }
                }
            })
            .inner;
        self.resistance = section_editor;
        if let Some(action) = section_action {
            self.section_edit(action);
        }
        self.adjust_scale = adjust_scale;
        if let Some(action) = output_action {
            self.output_action(action);
        }
        if dry_run {
            self.dry_run_topology();
        }
        if apply {
            self.apply_topology();
        }
        if let Some(did) = excluded_did {
            save_options |= self.excluded_edit(did);
        }
        if save_options {
            self.save_options();
        }
        self.topology_report(&ctx);
        self.acted_on_the_map(acted.0, acted.1, &ctx);
        self.screen_overlay_tick(&ctx, acted.1);
        // Last, so a drag or a setting changed on this frame is in the document this
        // tick sends rather than waiting for the next one.
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
    if std::env::args().nth(1).as_deref() == Some("--screen-overlay-host") {
        std::process::exit(lbm_overlay::linux::run_host());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LittleBigMouse")
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([480.0, 320.0]),
        ..Default::default()
    };
    eframe::run_native(
        "LittleBigMouse",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))),
    )
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
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
                4 => app.view = View::Settings,
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
        app.session_event(Event::Connected { requests, snapshot });
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
}
