//! What the user asked of their wallpaper, per layout — port of the C# plugin's
//! `WallpaperSettings` and `WallpaperSettingsStore` (`wallpaper.json` in the
//! configuration directory).
//!
//! Wallpapers are applied live, outside the layout's save and undo, so they stay out of
//! the layout model and of the layout store: one file, one entry per layout id. The file
//! is the C# one, member for member — `System.Text.Json` writes the enums with a string
//! converter where the C# type asks for one (`Mode`, `Style`) and as a number where it
//! does not (`Kind`).

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::lbm_paths;

/// C# `WallpaperMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperMode {
    /// Each screen has its own image or color.
    #[default]
    PerScreen,
    /// One image over the whole monitor set, sliced in physical mm space.
    Span,
}

/// C# `WallpaperStyle` (`DisplaySource.cs`), as the desktops draw an image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallpaperStyle {
    #[default]
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
    Span,
}

/// C# `ScreenWallpaperKind`: written as a number, the enum having no string converter
/// (`System.Text.Json` writes an unannotated enum as its value).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ScreenWallpaperKind {
    #[default]
    Image = 0,
    Color = 1,
}

impl Serialize for ScreenWallpaperKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for ScreenWallpaperKind {
    /// As tolerant as `System.Text.Json`, which takes any number for an enum.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match u8::deserialize(deserializer)? {
            1 => ScreenWallpaperKind::Color,
            _ => ScreenWallpaperKind::Image,
        })
    }
}

/// C# `ScreenWallpaperSettings`: one screen's own wallpaper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ScreenWallpaperSettings {
    pub kind: ScreenWallpaperKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    pub style: WallpaperStyle,
    /// `#RRGGBB`.
    pub color: String,
}

impl Default for ScreenWallpaperSettings {
    fn default() -> Self {
        ScreenWallpaperSettings {
            kind: ScreenWallpaperKind::default(),
            image_path: None,
            style: WallpaperStyle::default(),
            color: "#204060".to_owned(),
        }
    }
}

/// C# `LayoutWallpaperSettings`: what one layout asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutWallpaperSettings {
    pub mode: WallpaperMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_image_path: Option<String>,
    #[serde(default)]
    pub per_screen: BTreeMap<String, ScreenWallpaperSettings>,
}

impl LayoutWallpaperSettings {
    /// C# `HasContent`: the user configured something worth pushing to the desktop.
    pub fn has_content(&self) -> bool {
        match self.mode {
            WallpaperMode::Span => self.span_image_path.as_ref().is_some_and(|p| !p.is_empty()),
            WallpaperMode::PerScreen => self.per_screen.values().any(|screen| {
                screen.kind == ScreenWallpaperKind::Color
                    || screen.image_path.as_ref().is_some_and(|p| !p.is_empty())
            }),
        }
    }
}

/// Every layout's settings, by layout id.
pub type WallpaperSettings = BTreeMap<String, LayoutWallpaperSettings>;

/// C# `WallpaperSettingsStore.FilePath`.
pub fn settings_path() -> PathBuf {
    lbm_paths::config_dir().join("wallpaper.json")
}

/// C# `Load`: what the file holds, nothing when it is absent — and nothing, said out
/// loud, when it cannot be read (a wallpaper is never a reason not to start).
pub fn load(path: &Path) -> WallpaperSettings {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
            eprintln!("[lbm-agent] wallpaper settings unreadable, starting fresh: {error}");
            WallpaperSettings::new()
        }),
        Err(_) => WallpaperSettings::new(),
    }
}

/// C# `Save`: indented JSON, written beside the file and renamed over it.
pub fn save(path: &Path, settings: &WallpaperSettings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let staged = path.with_extension("json.tmp");
    std::fs::write(&staged, text)?;
    std::fs::rename(&staged, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> LayoutWallpaperSettings {
        LayoutWallpaperSettings {
            mode: WallpaperMode::Span,
            span_image_path: Some("/home/u/pictures/wide.jpg".to_owned()),
            per_screen: BTreeMap::new(),
        }
    }

    #[test]
    fn the_file_is_the_csharp_one() {
        let mut settings = WallpaperSettings::new();
        settings.insert("DELA0B1+DELA0B2".to_owned(), span());
        settings.insert(
            "LEN1234".to_owned(),
            LayoutWallpaperSettings {
                per_screen: BTreeMap::from([(
                    "MON1".to_owned(),
                    ScreenWallpaperSettings {
                        kind: ScreenWallpaperKind::Color,
                        color: "#102030".to_owned(),
                        ..ScreenWallpaperSettings::default()
                    },
                )]),
                ..LayoutWallpaperSettings::default()
            },
        );
        let written = serde_json::to_string_pretty(&settings).unwrap();
        // The enums with a string converter in C#, and the one without.
        assert!(written.contains(r#""Mode": "Span""#));
        assert!(written.contains(r#""Style": "Fill""#));
        assert!(written.contains(r#""Kind": 1"#));
        assert!(written.contains(r#""SpanImagePath": "/home/u/pictures/wide.jpg""#));

        let read: WallpaperSettings = serde_json::from_str(&written).unwrap();
        assert_eq!(read, settings);
    }

    #[test]
    fn what_a_csharp_file_holds_is_read_back() {
        let text = r##"{
  "DELA0B1": {
    "Mode": "PerScreen",
    "SpanImagePath": null,
    "PerScreen": {
      "MON1": {
        "Kind": 0,
        "ImagePath": "/pictures/a.png",
        "Style": "Center",
        "Color": "#204060"
      }
    }
  }
}"##;
        let settings: WallpaperSettings = serde_json::from_str(text).unwrap();
        let layout = &settings["DELA0B1"];
        assert_eq!(layout.mode, WallpaperMode::PerScreen);
        assert_eq!(layout.span_image_path, None);
        let screen = &layout.per_screen["MON1"];
        assert_eq!(screen.kind, ScreenWallpaperKind::Image);
        assert_eq!(screen.style, WallpaperStyle::Center);
        assert_eq!(screen.image_path.as_deref(), Some("/pictures/a.png"));
    }

    #[test]
    fn only_a_configured_layout_is_worth_pushing() {
        assert!(span().has_content());
        assert!(!LayoutWallpaperSettings {
            mode: WallpaperMode::Span,
            span_image_path: Some(String::new()),
            ..LayoutWallpaperSettings::default()
        }
        .has_content());
        assert!(!LayoutWallpaperSettings::default().has_content());
        assert!(LayoutWallpaperSettings {
            per_screen: BTreeMap::from([(
                "MON1".to_owned(),
                ScreenWallpaperSettings {
                    kind: ScreenWallpaperKind::Color,
                    ..ScreenWallpaperSettings::default()
                }
            )]),
            ..LayoutWallpaperSettings::default()
        }
        .has_content());
    }

    #[test]
    fn a_file_that_cannot_be_read_starts_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wallpaper.json");
        assert!(load(&path).is_empty(), "no file yet");

        let mut settings = WallpaperSettings::new();
        settings.insert("DELA0B1".to_owned(), span());
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), settings);

        std::fs::write(&path, "{ not json").unwrap();
        assert!(load(&path).is_empty());
    }
}
