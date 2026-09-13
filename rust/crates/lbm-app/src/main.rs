//! The frontend's window: what `lbm-ui` draws, on a screen someone can look at.
//!
//! Everything up to now has been a library with no window — driven and asserted on
//! through the accessibility tree, which is how it can be tested at all on a Wayland
//! machine. That is worth keeping, so `lbm-ui` stays window-free and everything that
//! needs winit and a GL context lives here.
//!
//! **This window reads and draws. It does nothing else.** It does not start an agent, it
//! does not speak to one, it does not start a hook, and it writes no file. Finding the
//! displays is `kscreen-doctor --json` or xrandr and the EDID under sysfs, all of them
//! read-only; the layout is built in memory with a loader that does nothing, so no stored
//! profile is read and none is written. The engine controls are drawn because they are
//! part of the window, and they are all disabled, which is the truth: there is no agent
//! to ask.
//!
//! It links `lbm-display` and `lbm-layout` directly rather than `lbm-agent`, mirroring
//! `lbm-agent/src/discovery.rs`. The architecture has the frontend talk to the agent over
//! a socket; a frontend that linked the agent would be a frontend that could start a
//! hook, and this one cannot.

use std::collections::HashMap;
use std::path::PathBuf;

use lbm_app::client::{self, Message};
use lbm_icons::{Catalogue, Rgba};
use lbm_layout::geo::Rect;
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_ui::map::MapMonitor;
use lbm_ui::{frame, list, map};

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
}

/// What the worker sends back. Two kinds, so the screens can be drawn as soon as they
/// are known instead of waiting on a wallpaper that may take a second to decode.
enum Found {
    Screens(Vec<Screen>),
    /// Something the agent said.
    Agent(Message),
    /// The agent went away, or was never there.
    AgentGone,
    Thumbnail {
        screen: String,
        picture: image::RgbaImage,
    },
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
    screens: Vec<Screen>,
    selected: Option<String>,
    view: View,
    icons: Catalogue,
    /// Uploaded logos, by icon path. Loaded once, on the frame that first needs one.
    textures: HashMap<String, egui::TextureHandle>,
    /// Uploaded wallpaper thumbnails, by screen id.
    wallpapers: HashMap<String, egui::TextureHandle>,
    /// What the bottom bar shows, from what the agent says.
    state: lbm_ui::State,
    /// What to ask the agent, when there is one to ask.
    requests: Option<Asks>,
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
/// the agent. Gives the layout's id too: the wallpaper settings are keyed by it.
fn detect() -> (String, Vec<Screen>) {
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
        return (String::new(), Vec::new());
    }

    let screens = layout
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
            })
        })
        .collect();
    (layout.id.clone(), screens)
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

/// What the window asks the agent: a method and whatever the API wants beside it.
type Asks = std::sync::mpsc::Sender<(&'static str, serde_json::Value)>;

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
) -> (Option<Asks>, Option<(String, String)>) {
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
    // takes the connection over.
    if let Err(error) = outgoing
        .ask("Hello", serde_json::json!({ "Client": "lbm-app" }))
        .and_then(|_| outgoing.ask("Subscribe", serde_json::json!({})))
    {
        return (None, Some((error.to_string(), where_)));
    }

    let waker = ctx.clone();
    std::thread::spawn(move || loop {
        match incoming.receive() {
            Ok(message) => {
                if found.send(Found::Agent(message)).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = found.send(Found::AgentGone);
                waker.request_repaint();
                return;
            }
        }
        waker.request_repaint();
    });

    let (asks, to_write) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        while let Ok((method, extra)) = to_write.recv() {
            if outgoing.ask(method, extra).is_err() {
                return;
            }
        }
    });
    (Some(asks), None)
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
            let (id, screens) = detect();
            // The wallpaper is worked out from what the screens turn out to be, so the
            // settings are read here and not before.
            let wanted = wallpapers(&id, &screens);
            // The receiver is gone only if the window closed first, which is not a
            // failure worth reporting.
            let _ = detected.send(Found::Screens(screens));
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
        let (requests, without_agent) = join_agent(&ctx, send);

        App {
            arriving: Some(arriving),
            screens: Vec::new(),
            selected: None,
            view: View::Map,
            icons,
            textures: HashMap::new(),
            wallpapers: HashMap::new(),
            state: lbm_ui::State::default(),
            requests,
            without_agent,
        }
    }

    /// Asks the agent for what the user pressed.
    ///
    /// Start and Stop only. Save and Undo would have to send a layout document, and
    /// this window has none to send — see [`App::agent_said`].
    fn ask(&mut self, press: lbm_ui::Press) {
        let method = match press {
            lbm_ui::Press::Start => "Start",
            lbm_ui::Press::Stop => "Stop",
            lbm_ui::Press::Save | lbm_ui::Press::Undo => return,
        };
        if let Some(requests) = &self.requests {
            if requests.send((method, serde_json::json!({}))).is_err() {
                self.requests = None;
                self.without_agent =
                    Some(("the agent stopped listening".to_owned(), String::new()));
            }
        }
    }

    /// What the agent said, in the bar's terms.
    ///
    /// **`saved` is not taken from the agent, and that is deliberate.** Save and Undo
    /// need a layout document to send, and this window cannot make one: it draws the
    /// screens it detects and edits nothing. A Save offered here would be a button with
    /// nothing behind it, which is worse than a button that is grey. When the window
    /// learns to edit, the flag comes with it.
    fn agent_said(&mut self, message: Message) {
        let Message::State(state) = message else {
            // Answers and hook events are not the bar's business yet: the bar reads
            // state, and every request this window makes changes state, so the change
            // is what it hears. Kept rather than dropped, so that wiring the probe
            // report later is a matter of reading them.
            return;
        };
        let engine = state
            .get("Engine")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Dead");
        self.state.engine = lbm_ui::Engine::from_agent(engine);
        self.state.hook_connected = state
            .get("HookConnected")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        self.state.waiting = false;
        self.without_agent = None;
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
            })
            .collect()
    }
}

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
                    Found::Screens(screens) => self.screens = screens,
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
                ui.separator();
                ui.label(match self.screens.len() {
                    0 => "no screens detected".to_owned(),
                    1 => "1 screen".to_owned(),
                    n => format!("{n} screens"),
                });
                ui.separator();
                // Said plainly. A window that looked the same with and without an agent
                // would leave the user guessing why the buttons do nothing.
                match &self.without_agent {
                    Some((why, where_)) => ui
                        .label(format!("no agent — {why}"))
                        .on_hover_text(where_.clone()),
                    None => ui.label("agent connected"),
                };
            });
        });

        egui::Panel::bottom("controls").show(ui, |ui| {
            // Without an agent every button is disabled on its own — `can` asks for a
            // hook — so nothing here has to hide them.
            if let Some(lbm_ui::Action::Pressed(press)) = lbm_ui::bottom_bar(ui, &self.state) {
                for effect in lbm_ui::update(&mut self.state, lbm_ui::Action::Pressed(press)) {
                    let lbm_ui::Effect::Ask(press) = effect;
                    self.ask(press);
                }
            }
        });

        // The `Ui` handed to `App::ui` has no background of its own — the doc says so
        // outright — so the map would be drawn on nothing.
        egui::CentralPanel::default().show(ui, |ui| {
            let monitors = self.monitors(&logos);
            let at = ui.max_rect();
            let picked = match self.view {
                View::Map => {
                    let fit = map::fit(map::extent(&monitors), at);
                    map::draw(ui, &monitors, &fit, self.selected.as_deref())
                }
                View::List => list::draw(ui, at, &monitors, self.selected.as_deref()),
            };
            picked.map(str::to_owned)
        });
    }
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
