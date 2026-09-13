//! Against the icons that actually ship, not fixtures.
//!
//! The alias table lives nowhere but in the file names — `Acer.CHE.ALI.ACR.API.svg` is
//! the only record that those four PnP codes are one manufacturer. A test on invented
//! files would prove the parser parses; this one proves the shipped set still resolves.

use std::path::PathBuf;

use lbm_icons::{catalogue, recolour, render, resolve, Rgba};

fn assets() -> PathBuf {
    // From `rust/crates/lbm-icons` up to the repository root. The root is `Assets` and
    // not `Assets/Icon`, so that the keys come out as the `Logo` strings spell them:
    // the C# loader strips `/Assets/` and keeps `Icon/Pnp/…`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets")
}

#[test]
fn every_dot_in_a_name_is_another_code_the_logo_answers_to() {
    let (found, collisions) = catalogue(&assets());

    // The manufacturer's own name, and each PnP code it ships under.
    let acer = found.get("icon/pnp/acer").expect("Acer has a logo");
    for code in ["CHE", "ALI", "ACR", "API"] {
        assert_eq!(
            found.get(&format!("icon/pnp/{}", code.to_lowercase())),
            Some(acer),
            "{code} is Acer and resolves to the same file"
        );
    }
    // A single-token name is just itself.
    assert!(found.contains_key("icon/pnp/amd"));
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
    let pnp = found.keys().filter(|k| k.starts_with("icon/pnp/")).count();

    assert!(
        pnp > 90,
        "only {pnp} names under Pnp/ — the aliases are not being read"
    );
    // The fallback the Linux factory names when a code has no logo of its own
    // (`icon/Pnp/{code}?icon/Pnp/LBM`) has to exist, or the fallback falls through.
    assert!(
        found.contains_key("icon/pnp/lbm"),
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
    let path = found.get("icon/pnp/lbm").expect("the fallback logo");
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

//==================//
// Resolution       //
//==================//

/// The `?` is the whole of the fallback mechanism, and the codes below are not invented:
/// they are what the oracle's `six-monitors` desktop reports.
#[test]
fn a_code_with_no_logo_of_its_own_falls_back() {
    let (found, _) = catalogue(&assets());

    // DEL has a logo; the fallback is never reached.
    let dell = resolve(&found, "icon/Pnp/DEL?icon/Pnp/LBM").expect("Dell");
    assert!(dell.file_name().unwrap().to_string_lossy().contains("Dell"));

    // XEC has none, so the LittleBigMouse logo stands in.
    let unknown = resolve(&found, "icon/Pnp/XEC?icon/Pnp/LBM").expect("the fallback");
    assert_eq!(
        unknown,
        resolve(&found, "icon/Pnp/LBM").expect("the fallback exists on its own")
    );
}

/// The lookup is case-insensitive on both sides, as `AddIconProvider` and every
/// `TryGetValue` are. A port that compared exactly would answer differently depending on
/// how a brand happens to be spelled in a file name.
#[test]
fn the_lookup_does_not_care_about_case() {
    let (found, _) = catalogue(&assets());

    let one = resolve(&found, "icon/Pnp/DEL").expect("Dell");
    for spelling in ["icon/pnp/del", "ICON/PNP/DEL", "  icon/Pnp/del  "] {
        assert_eq!(resolve(&found, spelling), Some(one), "{spelling}");
    }
}

/// There is no last resort. The C# looks for `icons/default` after both halves miss, and
/// nothing can ever register that key — the loader builds every key from `Assets/…` with
/// an `icon/` prefix, so `icons/` with an s is unreachable. A miss is an icon that is not
/// drawn, and here that is `None` rather than some stand-in of this port's invention.
#[test]
fn nothing_answers_for_a_code_that_is_not_there() {
    let (found, _) = catalogue(&assets());

    assert_eq!(resolve(&found, "icon/Pnp/ZZZ"), None);
    assert_eq!(resolve(&found, "icon/Pnp/ZZZ?icon/Pnp/YYY"), None);
    assert!(
        !found.contains_key("icons/default"),
        "if this key ever exists, the last-resort branch stops being dead"
    );
}

/// Two ASUS screens, two EDID codes, and only one of them was covered. The alias table
/// is the file name, so `Asus.ATK.ACI.ASU.AUS.svg` is the whole of the fix — and this
/// test is what keeps a later rename from quietly undoing it.
#[test]
fn both_of_the_codes_asus_actually_reports_find_the_asus_logo() {
    let (found, _) = catalogue(&assets());

    let asus = resolve(&found, "icon/Pnp/ACI").expect("ASUS PB278 reports ACI");
    let same = resolve(&found, "icon/Pnp/AUS").expect("ASUS VP28U reports AUS");
    assert_eq!(asus, same, "the same manufacturer, the same logo");
}

/// A key is a name in the resource space, not a file path: it is spelled with `/` on
/// every platform. This bit on Windows — the keys came out `icon\pnp/del`, nothing
/// resolved, and the count of names under `icon/pnp/` was **zero** — while every test
/// here passed on Linux. It is asserted as a property of the whole shipped set so that
/// the platform that cannot run this locally is still the one that catches it.
#[test]
fn keys_are_spelled_with_slashes_whatever_the_platform() {
    let (found, _) = catalogue(&assets());

    for key in found.keys() {
        assert!(
            !key.contains('\\'),
            "a key built with the platform's separator: {key}"
        );
        assert_eq!(key, &key.to_lowercase(), "a key that was not lowercased");
    }
    // And the nesting really is two deep, so the join above is exercised rather than
    // being trivially right on a one-segment prefix — which is how the spike missed it.
    assert!(found.keys().any(|k| k.matches('/').count() == 2));
}
