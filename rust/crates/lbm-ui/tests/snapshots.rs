//! What the views actually look like.
//!
//! Every other test in this crate asks the **accessibility tree** — which is how `lbm-ui`
//! can be tested at all on a Wayland machine, and which is genuinely the right question
//! for "is the name there", "did the click land", "is Save disabled". It is the wrong
//! question for "does this look like anything". A view can have a perfect tree and paint
//! white on white, put a label under a panel, or lose its background: the tree says all
//! is well because, to the tree, all is.
//!
//! So these render. `egui_kittest` draws through wgpu and compares against a committed
//! PNG; a view that changes shape fails until someone looks at the diff and says yes.
//!
//! **Linux only, and not by taste.** The renderer is wgpu, and wgpu cannot enter this
//! workspace's dependency graph on Windows — see the note on the target-gated
//! dev-dependency in `Cargo.toml`. On Windows this file compiles to nothing.
//!
//! The references are generated **on CI's renderer**, not on a developer's. Software
//! rasterization (lavapipe) and a real GPU do not antialias identically, so a PNG made
//! here would fail there and vice versa; `UPDATE_SNAPSHOTS=1` regenerates, and the
//! workflow uploads what it rendered when a comparison fails.
#![cfg(not(windows))]

use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::frame::{self, Ratio};
use lbm_ui::map::{self, MapMonitor};

/// A window of a stated size, with a background — both on purpose.
///
/// The size, because the fit follows the window: a harness left at its default would make
/// every reference depend on that default.
///
/// The `CentralPanel`, because **`Harness::render` paints on nothing.** The `Ui` handed to
/// `eframe::App::ui` has no background of its own and the window wraps it in a panel
/// (`main.rs`); a snapshot taken without one renders white-on-transparent, which composites
/// into whatever the viewer happens to put behind it. The first run of these tests produced
/// exactly that — a map that looked like black rectangles on white paper, and a stray dark
/// block that was nothing but premultiplied alpha. Picture and app must be the same thing
/// or the reference is pinning a fiction.
fn harness<'a>(size: (f32, f32), mut app: impl FnMut(&mut egui::Ui) + 'a) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .build_ui(move |ui| {
            egui::CentralPanel::default().show(ui, &mut app);
        })
}

/// The two screens the other map tests use, with the 20 mm bezel the oracle's desktops
/// really have.
fn two_screens() -> Vec<MapMonitor<'static>> {
    vec![
        MapMonitor {
            id: "left",
            name: "Left screen",
            mm_outside: Rect::new(0.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(20.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
        },
        MapMonitor {
            id: "right",
            name: "Right screen",
            mm_outside: Rect::new(700.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(720.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
        },
    ]
}

/// The bar, running, with a screen selected — the state a user spends their time in.
#[test]
fn the_bottom_bar_running() {
    let state = lbm_ui::State {
        engine: lbm_ui::Engine::Running,
        hook_connected: true,
        ..Default::default()
    };
    let mut harness = harness((520.0, 60.0), |ui| {
        lbm_ui::bottom_bar(ui, &state);
    });
    harness.run();
    harness.snapshot("bottom-bar-running");
}

/// And with no agent: **every button disabled**. The one state the tree cannot vouch for
/// on its own — "disabled" in the tree is a flag, on screen it is a colour, and a theme
/// that greyed nothing would read as an app that simply does not respond.
#[test]
fn the_bottom_bar_without_an_agent() {
    let state = lbm_ui::State::default();
    let mut harness = harness((520.0, 60.0), |ui| {
        lbm_ui::bottom_bar(ui, &state);
    });
    harness.run();
    harness.snapshot("bottom-bar-no-agent");
}

/// The map: two screens, one selected. The bezels, the fit — and **no names**, at a size
/// a window is very likely to be. That is not a bug in the picture, it is the picture of
/// the open question: the name is half the top bezel, here that lands under the 7 pt
/// legibility floor, and the floor drops it. `docs/v7-frontend.md` says the floor decides
/// the common case; this is the common case, and it is why the reference is kept.
#[test]
fn the_map_with_two_screens() {
    let screens = two_screens();
    let mut harness = harness((900.0, 600.0), |ui| {
        let fit = map::fit(map::extent(&screens), ui.max_rect());
        map::draw(ui, &screens, &fit, Some("left"));
    });
    harness.run();
    harness.snapshot("map-two-screens");
}

/// One frame, large. This is the picture behind the open question about the name: it is
/// **half the top bezel**, printed on the plastic, and on a thin-bezel screen that lands
/// between 1 and 5 points. Here the bezel is 20 mm and the frame is big, so the name is
/// comfortably drawn — which is exactly the case the table in `docs/v7-frontend.md` says
/// is *not* the common one.
#[test]
fn a_monitor_frame_with_its_name() {
    let outside = Rect::new(0.0, 0.0, 640.0, 380.0);
    let content = Rect::new(20.0, 20.0, 600.0, 340.0);
    let drawn = frame::draw(outside, content, (0.0, 0.0), Ratio { x: 0.6, y: 0.6 });
    let mut harness = harness((420.0, 260.0), |ui| {
        frame::monitor(
            ui,
            &drawn,
            &frame::Look {
                name: "Odyssey G80SD",
                ..Default::default()
            },
        );
    });
    harness.run();
    harness.snapshot("monitor-frame");
}
