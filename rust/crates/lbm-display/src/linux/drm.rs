//! Monitor EDIDs from sysfs — port of `DrmEdidReader.cs`.
//!
//! `/sys/class/drm/card<N>-<connector>/edid`, indexed by connector name (the `cardN-`
//! prefix stripped, e.g. `DP-2`), which is how KScreen and native-X11 xrandr name
//! their outputs. On a multi-GPU machine the same connector name can exist on several
//! cards: only entries whose status is `connected` with a full EDID count, so the idle
//! card's dangling connectors never shadow the live ones.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::edid::{self, Edid};

/// Where the kernel lists the DRM connectors.
pub const DRM_CLASS: &str = "/sys/class/drm";

/// EDIDs by connector name, compared case-insensitively (C#: a dictionary with
/// `StringComparer.OrdinalIgnoreCase`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EdidMap(HashMap<String, Edid>);

impl EdidMap {
    pub fn get(&self, connector: &str) -> Option<&Edid> {
        self.0.get(&connector.to_uppercase())
    }

    pub fn insert(&mut self, connector: &str, edid: Edid) {
        self.0.insert(connector.to_uppercase(), edid);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// `Directory.EnumerateDirectories(dir, "card*-*")`: the connector entries (symbolic
/// links to directories in sysfs), in the directory's own order, as C# gets them.
fn connectors(dir: &Path) -> Option<Vec<PathBuf>> {
    let entries = fs::read_dir(dir).ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.len() > 4 && n.starts_with("card") && n[4..].contains('-'))
                    && p.is_dir()
            })
            .collect(),
    )
}

fn status(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join("status"))
        .ok()
        .map(|s| s.trim().to_owned())
}

/// C# `DrmEdidReader.ReadAll`, over the connectors under `drm_class`: the EDID of every
/// connected connector with a full block (128 bytes or more), by connector name. A
/// later entry of the same name wins, as in C#. Unreadable entries are skipped:
/// discovery must never fail here.
pub fn read_all_in(drm_class: &Path) -> EdidMap {
    let mut result = EdidMap::default();
    for dir in connectors(drm_class).unwrap_or_default() {
        if status(&dir).as_deref() != Some("connected") {
            continue;
        }
        let Ok(bytes) = fs::read(dir.join("edid")) else {
            continue;
        };
        if bytes.len() < 128 {
            continue;
        }
        // "card1-DP-2" -> "DP-2"
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((_, connector)) = name.split_once('-') else {
            continue;
        };
        result.insert(connector, edid::parse(dir.to_string_lossy(), &bytes));
    }
    result
}

/// [`read_all_in`] the system's [`DRM_CLASS`].
pub fn read_all() -> EdidMap {
    read_all_in(Path::new(DRM_CLASS))
}

/// C# `DrmEdidReader.PlugSignature`, over `drm_class`: a cheap fingerprint of what is
/// physically plugged — every connector's name, status and EDID length, in ordinal
/// order. Pure sysfs reads, pollable at will: catches plugging, unplugging and monitor
/// swaps; moves made in the compositor only show in the display signature.
pub fn plug_signature_in(drm_class: &Path) -> String {
    let Some(mut dirs) = connectors(drm_class) else {
        return String::new();
    };
    dirs.sort();
    dirs.iter()
        .map(|dir| {
            let status = status(dir).unwrap_or_else(|| "?".to_owned());
            let edid = if status == "connected" {
                fs::read(dir.join("edid")).map_or(0, |b| b.len())
            } else {
                0
            };
            let name = dir
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            format!("{name}:{status}:{edid}")
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// [`plug_signature_in`] the system's [`DRM_CLASS`].
pub fn plug_signature() -> String {
    plug_signature_in(Path::new(DRM_CLASS))
}
