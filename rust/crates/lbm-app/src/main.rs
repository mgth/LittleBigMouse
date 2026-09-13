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
    arriving: Option<std::sync::mpsc::Receiver<Vec<Screen>>>,
    screens: Vec<Screen>,
    selected: Option<String>,
    view: View,
    icons: Catalogue,
    /// Uploaded logos, by icon path. Loaded once, on the frame that first needs one.
    textures: HashMap<String, egui::TextureHandle>,
    /// What the bottom bar shows. No agent answers here, so it stays as it opens.
    state: lbm_ui::State,
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
fn detect() -> Vec<Screen> {
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
        return Vec::new();
    }

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
            })
        })
        .collect()
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
        let waker = ctx.clone();
        std::thread::spawn(move || {
            let found = detect();
            // The receiver is gone only if the window closed first, which is not a
            // failure worth reporting.
            let _ = send.send(found);
            // **The thread wakes the window.** Nothing polls, and nothing needs to:
            // measured, including the adversarial case where the answer is held back
            // three seconds so that it lands while the window is idle — eframe schedules
            // the pass and the answer is picked up on it.
            waker.request_repaint();
        });
        App {
            arriving: Some(arriving),
            screens: Vec::new(),
            selected: None,
            view: View::Map,
            icons,
            textures: HashMap::new(),
            state: lbm_ui::State::default(),
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
        if let Some(arriving) = &self.arriving {
            if let Ok(found) = arriving.try_recv() {
                self.screens = found;
                self.arriving = None;
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
            });
        });

        egui::Panel::bottom("controls").show(ui, |ui| {
            // Drawn, and every button disabled: there is no agent to ask, and saying so
            // by greying them is more honest than hiding them.
            let _ = lbm_ui::bottom_bar(ui, &self.state);
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
