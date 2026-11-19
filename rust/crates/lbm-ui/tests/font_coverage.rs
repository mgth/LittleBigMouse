//! A character missing from the interface font is drawn as an empty box, which no
//! other test sees: kittest finds widgets by their text, not by what is painted.
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::path::{Path, PathBuf};

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// The decoded text of every string and char literal, with its line. The tokenizer
/// drops comments; attributes, which carry the doc comments, are skipped. Macro
/// arguments are plain tokens, so `format!` and `ui.label` literals are both seen.
fn literals(tokens: TokenStream, found: &mut Vec<(String, usize)>) {
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Punct(p) if p.as_char() == '#' => {
                if matches!(tokens.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '!') {
                    tokens.next();
                }
                if matches!(tokens.peek(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket)
                {
                    tokens.next();
                }
            }
            TokenTree::Group(group) => literals(group.stream(), found),
            TokenTree::Literal(literal) => {
                let line = literal.span().start().line;
                match syn::Lit::new(literal) {
                    syn::Lit::Str(s) => found.push((s.value(), line)),
                    syn::Lit::Char(c) => found.push((c.value().to_string(), line)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn literals_of(source: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    literals(source.parse().expect("Rust source"), &mut found);
    found
}

#[test]
fn literals_are_read_decoded_and_without_comments() {
    let source = r#"
        /// A doc comment with "→" is not drawn.
        #[doc = "→"]
        fn f() {
            ui.label("OK"); // an old "→" in a comment
            /* "→" */
            ui.label("\u{2192}");
            ui.strong(format!("{} » {}", a, '·'));
        }
    "#;
    let found = literals_of(source);
    let texts: Vec<_> = found.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(texts, ["OK", "→", "{} » {}", "·"]);
    assert_eq!(found[1].1, 7);
}

#[test]
fn the_interface_font_draws_every_symbol_the_labels_use() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for name in ["lbm-ui", "lbm-app"] {
        sources(&crates.join(name).join("src"), &mut files);
    }
    let mut harness = egui_kittest::Harness::new_ui(|ui| {
        lbm_ui::theme::install(ui.ctx());
        ui.label("");
    });
    harness.run();
    let mut missing = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for (literal, line) in literals_of(&text) {
            for symbol in literal.chars().filter(|c| !c.is_ascii()) {
                let drawn = harness
                    .ctx
                    .fonts_mut(|fonts| fonts.has_glyph(&egui::FontId::proportional(13.0), symbol));
                if !drawn {
                    missing.push(format!(
                        "{symbol:?} (U+{:04X}) in {}:{line}",
                        symbol as u32,
                        file.display(),
                    ));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "drawn as an empty box:\n{}",
        missing.join("\n")
    );
}
