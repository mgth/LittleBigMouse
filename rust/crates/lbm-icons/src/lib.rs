//! The icon set: which file a name resolves to, and what colour it comes out.
//!
//! Two things the Avalonia side does that the frontend has to keep doing, and one it
//! does that the frontend should not.
//!
//! **Keep: the aliases.** A manufacturer logo serves several PnP codes, and the Avalonia
//! icon loader encodes that in the file name — every dot-separated token of
//! `Acer.CHE.ALI.ACR.API.svg` becomes a key of its own, so `Pnp/CHE` and `Pnp/ACR` and
//! `Pnp/Acer` are all that one file. Seventy-odd files cover the whole table that way,
//! and nothing else records it: lose the rule and you lose the table.
//!
//! **Keep: the recolouring.** An icon is drawn in black and painted in the theme's
//! colour at use.
//!
//! **Do not keep: the guess.** `IconView.EffectiveForeground` turns *any* pure black
//! foreground into white in a dark theme, on the grounds that "black in dark is never
//! meant". It is sometimes meant: the Location panel paints the ruler's graduations with
//! the background brush — black, deliberately — so they show on the ruler's white body,
//! and the heuristic makes them white on white. Here the caller's colour is the colour:
//! ask for black and you get black.
//!
//! The substitution itself is the Avalonia one — the three quoted spellings of black in
//! the source text — and it inherits that side\'s own TODO, "Black might be used inside
//! strings". Doing it properly means rewriting colour *attributes* after an XML parse,
//! and usvg\'s tree is immutable once built, so it is not a line or two. Deliberately
//! left: a spike answers whether the thing can be done, and this part is already known
//! to be doable — what it must not do is quietly pretend to have solved it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use resvg::tiny_skia;
use resvg::usvg;

/// What one icon name resolves to.
pub type Catalogue = BTreeMap<String, PathBuf>;

/// Every name the icons under `root` answer to, keyed as the Avalonia loader keys them:
/// the directories below `root`, then one entry per dot-separated token of the file
/// name.
///
/// `Pnp/Acer.CHE.ALI.ACR.API.svg` therefore answers to `Pnp/Acer`, `Pnp/CHE`, `Pnp/ALI`,
/// `Pnp/ACR` and `Pnp/API`. A token claimed by two files is a collision the caller
/// should know about, so the first one wins and the other is reported.
pub fn catalogue(root: &Path) -> (Catalogue, Vec<String>) {
    let mut found = Catalogue::new();
    let mut collisions = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("svg") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let prefix = path
                .parent()
                .and_then(|p| p.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            for token in stem.split('.') {
                if token.is_empty() {
                    continue;
                }
                let key = if prefix.is_empty() {
                    token.to_owned()
                } else {
                    format!("{prefix}/{token}")
                };
                match found.entry(key.clone()) {
                    std::collections::btree_map::Entry::Vacant(slot) => {
                        slot.insert(path.clone());
                    }
                    std::collections::btree_map::Entry::Occupied(_) => collisions.push(key),
                }
            }
        }
    }
    (found, collisions)
}

/// A colour to paint an icon in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

/// Loads `svg` with everything drawn in pure black painted `foreground` instead.
///
/// Substitutes the three spellings the Avalonia side substitutes, quoted as attribute
/// values are: a document that spells black some other way keeps it, and so does one
/// that is genuinely coloured — a brand logo stays its own colour, which is the reason
/// this is a substitution and not a mask over the alpha channel.
pub fn load(svg: &[u8], foreground: Rgba) -> Result<usvg::Tree, usvg::Error> {
    let source = String::from_utf8_lossy(svg);
    let painted = recolour(&source, foreground);
    usvg::Tree::from_data(painted.as_bytes(), &usvg::Options::default())
}

/// The substitution, on its own so a test can read it without rendering anything.
///
/// Two shapes, because SVGs come from two kinds of tool. A hand-written or exported
/// icon paints with an attribute — `fill="#000000"` — and Inkscape paints with a CSS
/// declaration inside one — `style="fill:#000000;stroke-width:0.99"`. The Avalonia side
/// only ever handled the first (it replaces the quoted spellings), so an Inkscape icon
/// is never themed there at all: the LittleBigMouse logo itself is one, and stays black
/// whatever the theme. Handling both is what makes this worth porting rather than
/// copying.
pub fn recolour(source: &str, foreground: Rgba) -> String {
    let Rgba(r, g, b, _) = foreground;
    let colour = format!("#{r:02X}{g:02X}{b:02X}");

    // The attribute form, quoted, as the Avalonia side does it.
    let mut out = source.to_owned();
    for black in ["\"Black\"", "\"black\"", "\"#FF000000\"", "\"#000000\""] {
        out = out.replace(black, &format!("\"{colour}\""));
    }

    // The declaration form, inside a `style` attribute. Anchored on the property name so
    // that a colour is only touched where a colour is being set — which is the whole of
    // what the Avalonia side's own TODO asks for ("Black might be used inside strings").
    for property in ["fill", "stroke", "stop-color", "flood-color"] {
        for black in ["#000000", "#FF000000", "#000", "black", "Black"] {
            out = out.replace(
                &format!("{property}:{black}"),
                &format!("{property}:{colour}"),
            );
            out = out.replace(
                &format!("{property}: {black}"),
                &format!("{property}: {colour}"),
            );
        }
    }
    out
}

/// Renders `tree` into `size`x`size` RGBA pixels, for a test to look at or a frontend to
/// upload as a texture.
pub fn render(tree: &usvg::Tree, size: u32) -> Option<Vec<u8>> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(pixmap.take())
}
