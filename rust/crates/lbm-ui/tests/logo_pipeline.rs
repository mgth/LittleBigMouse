//! From the string the model carries to pixels the frame could upload.
//!
//! `lbm-icons` knows how to resolve and paint an icon; `lbm-ui::frame` knows where a logo
//! goes and how big. Each is tested on its own, and each could be right while the two do
//! not meet — the frame asking for a size the renderer will not give, or a colour the
//! substitution does not reach. This walks the whole chain on the icons that ship, at the
//! band a real screen on a real map actually produces.

use std::path::PathBuf;

use lbm_icons::{catalogue, load, render, resolve, Rgba};
use lbm_layout::geo::Rect;
use lbm_ui::frame::{self, Ratio};

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets")
}

/// A 20 mm-bezelled screen, the size `grid-2x2` has, drawn at the ratio a four-screen
/// desktop gets in a 1000x700 window.
fn a_screen_on_the_map(ratio: f64) -> frame::Drawn {
    frame::draw(
        Rect::new(0.0, 0.0, 567.0, 336.0),
        Rect::new(20.0, 20.0, 527.0, 296.0),
        (0.0, 0.0),
        Ratio { x: ratio, y: ratio },
    )
}

fn as_rgba(c: egui::Color32) -> Rgba {
    Rgba(c.r(), c.g(), c.b(), c.a())
}

#[test]
fn a_logo_string_becomes_pixels_at_the_size_of_the_bezel() {
    let (found, _) = catalogue(&assets());
    // What a Dell reports, in the shape the model stores it.
    let file = resolve(&found, "icon/Pnp/DEL?icon/Pnp/LBM").expect("Dell has a logo");
    let svg = std::fs::read(file).expect("the logo reads");

    let colour = frame::logo_colour(&egui::Visuals::dark());
    let tree = load(&svg, as_rgba(colour)).expect("it parses");

    let drawn = a_screen_on_the_map(0.829);
    let size = tree.size();
    let placed = frame::fit_uniform(drawn.logo_band, egui::vec2(size.width(), size.height()));

    // The band is the bottom bezel: 20 mm at 0.829 is about 16.6 points high, and the
    // logo is at most that tall.
    assert!(
        (drawn.logo_band.height() - 16.58).abs() < 0.1,
        "the bezel came out {} points",
        drawn.logo_band.height()
    );
    assert!(placed.height() <= drawn.logo_band.height() + 1e-3);
    assert!(placed.width() >= 1.0, "nothing left to draw: {placed:?}");

    let pixels = render(&tree, placed.height().ceil() as u32).expect("it renders");
    assert!(
        pixels.chunks(4).any(|p| p[3] > 0),
        "the logo rendered to nothing at the size the frame asks for"
    );
}

/// The two crates have to agree about *when* there is no logo, not only about where it
/// goes. A screen drawn small enough leaves a bezel under a point tall, and the frame
/// skips the drawing there — which is also the size at which the renderer would be asked
/// for a zero-pixel pixmap.
#[test]
fn a_screen_too_small_for_a_logo_asks_for_no_pixels() {
    let (found, _) = catalogue(&assets());
    let file = resolve(&found, "icon/Pnp/DEL?icon/Pnp/LBM").expect("Dell");
    let svg = std::fs::read(file).expect("the logo reads");
    let tree = load(&svg, Rgba(0xFF, 0xFF, 0xFF, 255)).expect("it parses");
    let size = egui::vec2(tree.size().width(), tree.size().height());

    let tiny = a_screen_on_the_map(0.02);
    let placed = frame::fit_uniform(tiny.logo_band, size);

    assert!(
        placed.height() < 1.0,
        "a 20 mm bezel at 1:50 should be under a point: {placed:?}"
    );
    assert_eq!(
        render(&tree, placed.height() as u32),
        None,
        "the renderer refuses a zero size, so the frame's skip is the thing keeping it \
         from being asked"
    );
}

/// The colour the frame chooses is a colour the substitution actually reaches. A logo
/// asked for in one theme and drawn in the other's colour would be the same bug as the
/// Inkscape one, from the other end.
#[test]
fn the_frames_colour_reaches_the_paint() {
    let (found, _) = catalogue(&assets());
    let file = resolve(&found, "icon/Pnp/LBM").expect("the fallback logo");
    let svg = std::fs::read(file).expect("the logo reads");

    let light = frame::logo_colour(&egui::Visuals::light());
    let dark = frame::logo_colour(&egui::Visuals::dark());
    assert_ne!(light, dark, "the two themes ask for the same colour");

    let one = render(&load(&svg, as_rgba(light)).expect("parses"), 32).expect("renders");
    let two = render(&load(&svg, as_rgba(dark)).expect("parses"), 32).expect("renders");

    assert_ne!(
        one, two,
        "both themes rendered the same pixels: the colour never reached the paint"
    );
}
