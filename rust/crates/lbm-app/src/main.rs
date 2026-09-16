//! The frontend's window: what `lbm-ui` draws, on a screen someone can look at.
//!
//! Everything up to now has been a library with no window — driven and asserted on
//! through the accessibility tree, which is how it can be tested at all on a Wayland
//! machine. That is worth keeping, so `lbm-ui` stays window-free and everything that
//! needs winit and a GL context lives here.
//!
//! **This window never starts anything and writes no file.** It does not start an agent,
//! it does not start a hook. Finding the displays is `kscreen-doctor --json` or xrandr and
//! the EDID under sysfs, all of them read-only.
//!
//! It **does** read the store now — `options.json` and the layout profile — which it did
//! not when it could only draw. Reading is not what was dangerous; starting a process and
//! writing over the user's configuration were, and neither happens here. What the map
//! shows is therefore the arrangement the engine runs rather than the one the system
//! reports, which is what an edit has to start from. Everything that writes goes through
//! the agent, which stays the only writer: `SaveOptions` for the app-wide settings,
//! `SaveLayout` for the layout.
//!
//! Deliberately **not** read: the excluded list. `ExcludedListPersistence::load` writes
//! `Excluded.txt` when it is missing, and this window creates nothing — which is why
//! `settings::save_layout` has to strip the list back out of the document it sends.
//!
//! It links `lbm-display` and `lbm-layout` directly rather than `lbm-agent`, mirroring
//! `lbm-agent/src/discovery.rs`. The architecture has the frontend talk to the agent over
//! a socket; a frontend that linked the agent would be a frontend that could start a
//! hook, and this one cannot.

use std::collections::HashMap;
use std::path::PathBuf;

use lbm_app::client::{self, Message};
use lbm_icons::{Catalogue, Rgba};
use lbm_layout::geo::{Rect, Vector};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_ui::map::MapMonitor;
use lbm_ui::{drag, frame, list, map};

/// One monitor, owned, because `MapMonitor` borrows.
struct Screen {
    id: String,
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

/// What the worker sends back. Two kinds, so the screens can be drawn as soon as they
/// are known instead of waiting on a wallpaper that may take a second to decode.
enum Found {
    /// The layout as detected, and the screens read off it. Both, because the window now
    /// keeps the layout: dragging a screen writes a millimetre position into it, and
    /// [`screens_of`] reads the answer back out.
    Displays {
        layout: Box<Layout>,
        screens: Vec<Screen>,
        /// `None` when `Excluded.txt` does not exist yet — see [`read_excluded`].
        excluded: Option<Vec<String>>,
    },
    /// Something the agent said.
    Agent(Message),
    /// The agent went away, or was never there.
    AgentGone,
    Thumbnail {
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
}

struct App {
    /// Not yet arrived. Finding the displays runs `kscreen-doctor` — a **subprocess** —
    /// and the architecture note this crate was written against says effects never run
    /// on the UI thread. It takes 77 ms here, which is not a stall, but the thread is
    /// what the agent connection will need and it is better exercised now than invented
    /// later.
    arriving: Option<std::sync::mpsc::Receiver<Found>>,
    /// Everything the agent has said, and the requests still out.
    agent: lbm_app::conversation::Conversation,
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
    requests: Option<Asks>,
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
fn detect() -> Option<Box<Layout>> {
    let mut layout = Layout::new(LayoutOptions::default());

    #[cfg(windows)]
    let built = {
        match lbm_display::windows::discover() {
            Ok(tree) => lbm_layout::windows::populate(
                &mut layout,
                lbm_display::windows::thread_dpi_awareness(),
                &tree.layout_input(),
                |_| Ok::<(), std::convert::Infallible>(()),
            )
            .is_ok(),
            Err(error) => {
                eprintln!("[lbm-app] display discovery failed: {error}");
                false
            }
        }
    };

    #[cfg(not(windows))]
    let built = {
        let outputs = lbm_display::linux::Backend::detect()
            .map(|backend| {
                backend.query().unwrap_or_else(|error| {
                    eprintln!("[lbm-app] display discovery failed: {error}");
                    Vec::new()
                })
            })
            .unwrap_or_default();
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
    read_stored(&mut layout);
    Some(Box::new(layout))
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
        // Not drawn by this window; the bar does not offer them.
        Mode::Info | Mode::Resistance | Mode::Size | Mode::Vcp | Mode::Wallpaper => Vec::new(),
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

/// What the window asks the agent: the id its answer will carry, a method, and whatever
/// the API wants beside it.
///
/// **The window picks the id, not the writer thread.** It is the only way the pairing
/// between a request and its answer cannot lose a race: the window records `id -> method`
/// and *then* hands the request over, both on the UI thread, so there is no window in
/// which an answer exists and its meaning does not. Letting the writer assign the id and
/// announce it back would put that announcement on one thread and the answer on another,
/// and the ordering would hold only by luck.
type Asks = std::sync::mpsc::Sender<(u64, &'static str, serde_json::Value)>;

/// A joined agent: where to send requests, and the first id the window may use.
struct Joined {
    asks: Asks,
    next_id: u64,
    /// The handshake's own requests, by the id their answers will carry.
    asked: Vec<(u64, &'static str)>,
}

/// Why there is no agent: a short reason for the bar, and the endpoint it looked at.
type NoAgent = (String, String);

/// Connects to a running agent and keeps talking to it, or says why it cannot.
///
/// **Nothing is started.** No agent means a window that draws what it can and a bottom
/// bar that stays grey — never a process brought up behind the user's back, because the
/// process behind the agent is the hook and the hook takes the mice.
///
/// Two threads: one reads and pushes what the agent says at the window, one writes what
/// the window asks. Neither is the UI thread.
fn join_agent(
    ctx: &egui::Context,
    found: std::sync::mpsc::Sender<Found>,
) -> (Option<Joined>, Option<NoAgent>) {
    let Some(endpoint) = client::default_endpoint() else {
        return (
            None,
            Some((
                "this session has no endpoint".to_owned(),
                "neither XDG_RUNTIME_DIR nor a data directory".to_owned(),
            )),
        );
    };
    let where_ = endpoint.display().to_string();
    let (mut incoming, mut outgoing) = match client::connect(&endpoint) {
        Ok(both) => both,
        Err(error) => return (None, Some((error.to_string(), where_))),
    };

    // Who is there, and then everything it has to say. Asked once, before the reader
    // takes the connection over — so their ids are handed back with the rest: the
    // answer to `Subscribe` **is** the agent's state, and a window that did not know
    // which request it answered would throw it away.
    let handshake = match outgoing
        .ask("Hello", serde_json::json!({ "Client": "lbm-app" }))
        .and_then(|hello| {
            outgoing
                .ask("Subscribe", serde_json::json!({}))
                .map(|subscribe| vec![(hello, "Hello"), (subscribe, "Subscribe")])
        }) {
        Ok(asked) => asked,
        Err(error) => return (None, Some((error.to_string(), where_))),
    };

    let waker = ctx.clone();
    let reading = found.clone();
    std::thread::spawn(move || loop {
        match incoming.receive() {
            Ok(message) => {
                if reading.send(Found::Agent(message)).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = reading.send(Found::AgentGone);
                waker.request_repaint();
                return;
            }
        }
        waker.request_repaint();
    });

    // The ids the window may use, after the two spent above.
    let next_id = outgoing.reserve();
    let (asks, to_write) = std::sync::mpsc::channel::<(u64, &'static str, serde_json::Value)>();
    std::thread::spawn(move || {
        while let Ok((id, method, extra)) = to_write.recv() {
            if outgoing.ask_as(id, method, extra).is_err() {
                return;
            }
        }
    });
    (
        Some(Joined {
            asks,
            next_id,
            asked: handshake,
        }),
        None,
    )
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        let icons = icons_root()
            .map(|root| lbm_icons::catalogue(&root).0)
            .unwrap_or_default();
        if icons.is_empty() {
            eprintln!("[lbm-app] no icons found: logos will not be drawn (set LBM_ICONS)");
        }
        let (send, arriving) = std::sync::mpsc::channel();
        let detected = send.clone();
        let waker = ctx.clone();
        std::thread::spawn(move || {
            let Some(layout) = detect() else {
                return;
            };
            let screens = screens_of(&layout, lbm_ui::mode::Mode::default());
            // The wallpaper is worked out from what the screens turn out to be, so the
            // settings are read here and not before.
            let wanted = wallpapers(&layout.id, &screens);
            // The receiver is gone only if the window closed first, which is not a
            // failure worth reporting.
            let _ = detected.send(Found::Displays {
                layout,
                screens,
                excluded: lbm_app::settings::read_excluded(
                    lbm_store::layout_persistence::default_excluded_list_file(),
                ),
            });
            // **The thread wakes the window.** Nothing polls, and nothing needs to:
            // measured, including the adversarial case where the answer is held back
            // three seconds so that it lands while the window is idle — eframe schedules
            // the pass and the answer is picked up on it.
            waker.request_repaint();

            // Then the pictures, one at a time and each announced as it is ready:
            // decoding a 4K wallpaper is not quick, and the map is worth looking at
            // before they arrive.
            for (screen, picture) in wanted {
                if detected.send(Found::Thumbnail { screen, picture }).is_err() {
                    return;
                }
                waker.request_repaint();
            }
        });
        let (joined, without_agent) = join_agent(&ctx, send);
        // Without an agent there is nothing to number; the counter starts where the
        // handshake left off when there is.
        let (requests, next_id, asked) = match joined {
            Some(Joined {
                asks,
                next_id,
                asked,
            }) => (Some(asks), next_id, asked.into_iter().collect()),
            None => (None, 0, HashMap::new()),
        };

        App {
            arriving: Some(arriving),
            layout: None,
            saved: None,
            screens: Vec::new(),
            selected: None,
            drag: None,
            agent: lbm_app::conversation::Conversation::after(asked),
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
            requests,
            next_id,
            without_agent,
        }
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
        let (method, extra) = lbm_app::settings::apply_topology(layout, self.adjust_scale);
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
    /// Re-read rather than kept as a copy from the start — a copy would be one more thing
    /// that can drift, and the store is what Save writes to anyway. It runs on the UI
    /// thread: it is two small files, where the detection was a subprocess.
    fn undo(&mut self) {
        let Some(layout) = self.layout.as_mut() else {
            return;
        };
        read_stored(layout);
        self.saved = Some(lbm_app::saved::Reference::of(layout));
        self.screens = screens_of(layout, self.mode);
        self.drag = None;
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
        for effect in self.agent.said(message, &mut self.state) {
            match effect {
                lbm_ui::Effect::Ask(press) => self.ask(press),
                lbm_ui::Effect::EndPreview => self.end_preview(),
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
            })
            .collect()
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
                        layout,
                        screens,
                        excluded,
                    } => {
                        // The reference is taken here, after the store has been read and
                        // any migration applied: a profile written by an older version
                        // opens with nothing to save, which is the truth.
                        self.saved = Some(lbm_app::saved::Reference::of(&layout));
                        self.layout = Some(layout);
                        self.screens = screens;
                        self.excluded = excluded;
                    }
                    Found::Agent(message) => self.agent_said(message),
                    Found::AgentGone => {
                        self.requests = None;
                        self.without_agent =
                            Some(("the agent went away".to_owned(), String::new()));
                        self.state = lbm_ui::State::default();
                    }
                    Found::Thumbnail { screen, picture } => {
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

        egui::Panel::top("modes").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, View::Map, "Map");
                ui.selectable_value(&mut self.view, View::List, "List");
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
                if self.view != View::Settings {
                    ui.separator();
                    if let Some(chosen) = lbm_ui::mode::bar(ui, self.mode, &self.offered()) {
                        self.mode = chosen;
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

        egui::Panel::bottom("controls").show(ui, |ui| {
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

        // The `Ui` handed to `App::ui` has no background of its own — the doc says so
        // outright — so the map would be drawn on nothing.
        let mut save_options = false;
        let mut excluded_did = None;
        let mut apply = false;
        let mut adjust_scale = self.adjust_scale;
        let acted = egui::CentralPanel::default()
            .show(ui, |ui| {
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
                            ui.checkbox(&mut adjust_scale, "Also adjust the scales");
                            ui.separator();
                            if ui.button("Apply").clicked() {
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
                        (picked.map(|id| Did::Clicked(id.to_owned())), None)
                    }
                    // Returned above, before the screens were borrowed: the panel edits
                    // the very options they were read from.
                    View::Settings => unreachable!("handled before the monitors are built"),
                }
            })
            .inner;
        self.adjust_scale = adjust_scale;
        if apply {
            self.apply_topology();
        }
        if let Some(did) = excluded_did {
            save_options |= self.excluded_edit(did);
        }
        if save_options {
            self.save_options();
        }
        self.acted_on_the_map(acted.0, acted.1, &ctx);
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
