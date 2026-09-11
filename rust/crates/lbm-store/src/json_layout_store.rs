//! The JSON files backend — port of `LittleBigMouse.Platform.Linux/JsonLayoutStore.cs`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::json_format;
use crate::layout_dtos::{GlobalOptionsDto, LayoutDto, ModelDto};
use crate::layout_store::{LayoutStore, LayoutStoreData};
use crate::layout_store_key;
use crate::lbm_paths;

/// C# `JsonLayoutStore.LayoutExtension`.
const LAYOUT_EXTENSION: &str = ".json";

/// C# `JsonLayoutStore`: JSON files under the config directory
/// (`~/.config/LittleBigMouse` on Linux). The structure mirrors the Windows registry
/// tree so field semantics stay 1:1: `options.json` = the root key, `models.json` = the
/// per-PnP "monitors" keys, `layouts/<key>.json` = one `Layouts\{id}` key with its
/// monitors and sources.
///
/// Reads never fail: a missing, unreadable or corrupt file is absent, so a broken file
/// can never keep the app from starting. Writes are atomic (temp file + rename) and
/// replace the whole document — which, unlike the registry store, does delete a stored
/// value the written DTO leaves out. Unlike C#, they report their failure instead of
/// swallowing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonLayoutStore {
    config_dir: PathBuf,
}

impl Default for JsonLayoutStore {
    /// C# `JsonLayoutStore()`: the store of the current user, under
    /// [`lbm_paths::config_dir`].
    fn default() -> Self {
        JsonLayoutStore::new(lbm_paths::config_dir())
    }
}

impl JsonLayoutStore {
    /// C# `JsonLayoutStore(string configDir)`: a store rooted elsewhere (tests, tools).
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        JsonLayoutStore {
            config_dir: config_dir.into(),
        }
    }

    /// The directory the store lives in (C# `JsonLayoutStore._configDir`).
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// C# `JsonLayoutStore.OptionsPath`.
    pub fn options_path(&self) -> PathBuf {
        self.config_dir.join("options.json")
    }

    /// C# `JsonLayoutStore.ModelsPath`.
    pub fn models_path(&self) -> PathBuf {
        self.config_dir.join("models.json")
    }

    /// C# `JsonLayoutStore.LayoutPath`: the file a layout is stored in. The id loses
    /// the characters a file name cannot hold (each run of them becomes one `_`), then
    /// goes through [`layout_store_key`] with the file-name cap minus the extension —
    /// the sanitising and the #589 cap are part of the format.
    pub fn layout_path(&self, layout_id: &str) -> PathBuf {
        let id = sanitize_file_name(layout_id);
        // A file name is capped at 255 bytes on every common filesystem, extension
        // included, and a nine-monitor id is longer (#589): same rule as the registry.
        let name = layout_store_key::key_for_capped(
            &id,
            layout_store_key::MAX_LENGTH - LAYOUT_EXTENSION.len(),
        )
        .expect("250 characters leave room for the digest");
        self.config_dir
            .join("layouts")
            .join(name + LAYOUT_EXTENSION)
    }
}

impl LayoutStore for JsonLayoutStore {
    /// C# `JsonLayoutStore.Read`. Every stored model comes back, whatever
    /// `pnp_codes` asks for — the C# store ignores the list too, and the mapper only
    /// looks up the codes it has.
    fn read(&self, layout_id: &str, _pnp_codes: &[&str]) -> io::Result<LayoutStoreData> {
        Ok(LayoutStoreData {
            global_options: read_json(&self.options_path()),
            layout: read_json(&self.layout_path(layout_id)),
            models: read_json(&self.models_path()).unwrap_or_default(),
        })
    }

    /// C# `JsonLayoutStore.WriteGlobalOptions`.
    fn write_global_options(&self, options: &GlobalOptionsDto) -> io::Result<()> {
        write_json(&self.options_path(), options)
    }

    /// C# `JsonLayoutStore.WriteLayout`.
    fn write_layout(&self, layout_id: &str, layout: &LayoutDto) -> io::Result<()> {
        write_json(&self.layout_path(layout_id), layout)
    }

    /// C# `JsonLayoutStore.WriteModels`: read `models.json`, replace or append the
    /// given models, write it back. A replaced model keeps its place in the file, a
    /// new one goes last.
    ///
    /// A `models.json` that does not parse counts as empty, as in C#: it is then
    /// overwritten with the given models alone, and every other stored model is lost.
    fn write_models(&self, models: &IndexMap<String, ModelDto>) -> io::Result<()> {
        let path = self.models_path();
        let mut all: IndexMap<String, ModelDto> = read_json(&path).unwrap_or_default();
        for (pnp_code, model) in models {
            all.insert(pnp_code.clone(), model.clone());
        }
        write_json(&path, &all)
    }
}

/// C# `JsonLayoutStore.ReadJson`: the document, or `None` when the file is missing or
/// cannot be read or parsed — a corrupt file must never prevent the app from starting.
fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    // C# File.Exists: false for a directory, as for anything it cannot see.
    if !path.is_file() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    json_format::from_slice(&bytes).ok()
}

/// C# `JsonLayoutStore.WriteJson`: create the directory, write a temp twin, rename it
/// over the target. The temp file has the same name with the `.tmp` extension instead
/// — the same length as the final name, not longer: a layout name may already sit at
/// the 255-byte cap, and a longer temp twin would fail right there (#589).
///
/// Like C#, a document that cannot be serialized (a NaN, an infinity) writes nothing,
/// and a failed rename leaves the temp file behind.
fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = change_extension(path, ".tmp");
    let json = json_format::to_string(value)?;
    fs::write(&temp, json)?;
    fs::rename(&temp, path)
}

/// C# `Path.ChangeExtension(path, extension)`: everything from the last `.` of the
/// file name on is replaced — a leading dot included, where Rust's `with_extension`
/// would see no extension — and without a `.` the extension is appended.
fn change_extension(path: &Path, extension: &str) -> PathBuf {
    let Some(name) = path.file_name() else {
        return path.to_path_buf();
    };
    let name = name.to_string_lossy();
    let stem = name.rfind('.').map_or(&*name, |dot| &name[..dot]);
    path.with_file_name(format!("{stem}{extension}"))
}

/// The C# `string.Join("_", layoutId.Split(Path.GetInvalidFileNameChars(),
/// StringSplitOptions.RemoveEmptyEntries))` of `LayoutPath`.
fn sanitize_file_name(layout_id: &str) -> String {
    layout_id
        .split(is_invalid_file_name_char)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// C# `Path.GetInvalidFileNameChars()`, which depends on the OS .NET runs on.
#[cfg(not(windows))]
fn is_invalid_file_name_char(c: char) -> bool {
    matches!(c, '\0' | '/')
}

/// C# `Path.GetInvalidFileNameChars()`, which depends on the OS .NET runs on.
#[cfg(windows)]
fn is_invalid_file_name_char(c: char) -> bool {
    matches!(
        c,
        '"' | '<' | '>' | '|' | '\0'..='\u{1f}' | ':' | '*' | '?' | '\\' | '/'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_extension_matches_dotnet() {
        let cases = [
            ("dir/options.json", "dir/options.tmp"),
            ("dir/A.B.json", "dir/A.B.tmp"),
            ("dir/.json", "dir/.tmp"),
            ("dir/name", "dir/name.tmp"),
            ("dir.d/name", "dir.d/name.tmp"),
        ];
        for (path, expected) in cases {
            assert_eq!(
                change_extension(Path::new(path), ".tmp"),
                Path::new(expected)
            );
        }
    }

    #[test]
    fn runs_of_invalid_characters_become_one_underscore() {
        assert_eq!(sanitize_file_name("A/B"), "A_B");
        assert_eq!(sanitize_file_name("/A//B/"), "A_B");
        assert_eq!(sanitize_file_name("A\0B"), "A_B");
        assert_eq!(sanitize_file_name("//"), "");
        assert_eq!(sanitize_file_name("A+B@DP-1"), "A+B@DP-1");
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_names_lose_more() {
        assert_eq!(
            sanitize_file_name(r#"A:B\C*D?E"F<G>H|I"#),
            "A_B_C_D_E_F_G_H_I"
        );
    }
}
