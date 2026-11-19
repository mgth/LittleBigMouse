//! A character missing from the interface font is drawn as an empty box, which no
//! other test sees: kittest finds widgets by their text, not by what is painted.
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

/// Non-ASCII characters inside the string literals of a line (comments excluded).
fn literal_symbols(line: &str) -> impl Iterator<Item = char> + '_ {
    let code = line.trim_start();
    let code = if code.starts_with("//") { "" } else { code };
    code.split('"')
        .skip(1)
        .step_by(2)
        .flat_map(str::chars)
        .filter(|c| !c.is_ascii())
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
        for (number, line) in text.lines().enumerate() {
            for symbol in literal_symbols(line) {
                let drawn = harness
                    .ctx
                    .fonts_mut(|fonts| fonts.has_glyph(&egui::FontId::proportional(13.0), symbol));
                if !drawn {
                    missing.push(format!(
                        "{symbol:?} (U+{:04X}) in {}:{}",
                        symbol as u32,
                        file.display(),
                        number + 1
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
