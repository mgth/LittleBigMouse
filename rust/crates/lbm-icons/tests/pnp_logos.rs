//! Against the icons that actually ship, not fixtures.
//!
//! The alias table lives nowhere but in the file names — `Acer.CHE.ALI.ACR.API.svg` is
//! the only record that those four PnP codes are one manufacturer. A test on invented
//! files would prove the parser parses; this one proves the shipped set still resolves.

use std::path::PathBuf;

use lbm_icons::{catalogue, recolour, render, Rgba};

fn assets() -> PathBuf {
    // From `rust/crates/lbm-icons` up to the repository root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon")
}

#[test]
fn every_dot_in_a_name_is_another_code_the_logo_answers_to() {
    let (found, collisions) = catalogue(&assets());

    // The manufacturer's own name, and each PnP code it ships under.
    let acer = found.get("Pnp/Acer").expect("Acer has a logo");
    for code in ["CHE", "ALI", "ACR", "API"] {
        assert_eq!(
            found.get(&format!("Pnp/{code}")),
            Some(acer),
            "{code} is Acer and resolves to the same file"
        );
    }
    // A single-token name is just itself.
    assert!(found.contains_key("Pnp/AMD"));
    assert!(
        collisions.is_empty(),
        "two files claim the same name: {collisions:?}"
    );
}

/// The point of the table: a monitor reports a four-letter code, and something has to
/// turn it into a picture. Enough of them have to be there for that to be worth doing.
#[test]
fn the_shipped_set_answers_to_a_great_many_codes() {
    let (found, _) = catalogue(&assets());
    let pnp = found.keys().filter(|k| k.starts_with("Pnp/")).count();

    assert!(
        pnp > 90,
        "only {pnp} names under Pnp/ — the aliases are not being read"
    );
    // The fallback the Linux factory names when a code has no logo of its own
    // (`icon/Pnp/{code}?icon/Pnp/LBM`) has to exist, or the fallback falls through.
    assert!(
        found.contains_key("Pnp/LBM"),
        "the fallback logo is missing"
    );
}

/// The upstream heuristic turns any pure black foreground into white in a dark theme,
/// which is how the Location ruler's graduations became white on white. Here the
/// caller's colour is the colour, and black is a colour.
#[test]
fn asking_for_black_gets_black() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="#000000"/></svg>"##;

    let painted = recolour(source, Rgba(0, 0, 0, 255));

    assert!(painted.contains(r##""#000000""##), "{painted}");
}

#[test]
fn what_is_not_black_keeps_its_colour() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="#000000"/><rect fill="#ED1C24"/></svg>"##;

    let painted = recolour(source, Rgba(0xFF, 0xFF, 0xFF, 255));

    assert!(painted.contains(r##""#FFFFFF""##), "the black became white");
    assert!(
        painted.contains(r##""#ED1C24""##),
        "a brand's own red is not the theme's business"
    );
}

/// And the colour reaches the pixels — the part that fails if the substitution is right
/// but the rendering is not.
#[test]
fn the_theme_colour_comes_out_in_the_rendering() {
    let (found, _) = catalogue(&assets());
    let path = found.get("Pnp/LBM").expect("the fallback logo");
    let source = std::fs::read(path).expect("the logo reads");

    let white = render(
        &lbm_icons::load(&source, Rgba(0xFF, 0xFF, 0xFF, 255)).expect("it parses"),
        64,
    )
    .expect("it renders");
    let red = render(
        &lbm_icons::load(&source, Rgba(0xFF, 0x00, 0x00, 255)).expect("it parses"),
        64,
    )
    .expect("it renders");

    assert_eq!(white.len(), red.len());
    assert_ne!(
        white, red,
        "the two colourings render the same: the colour never reached the paint"
    );
    // Something was actually drawn, rather than two empty pixmaps differing by nothing.
    assert!(
        white.chunks(4).any(|p| p[3] > 0),
        "the logo rendered to nothing at all"
    );
}
