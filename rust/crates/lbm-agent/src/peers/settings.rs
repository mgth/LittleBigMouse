//! `multi-pc.json`: whether this machine takes part in the multi-PC mode, and as what.
//!
//! Its own small file rather than an option of the layout store: the mode belongs to
//! the machine, not to a layout, and the store's documents are lot 3's to change.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file, in the configuration directory.
pub const SETTINGS_FILE: &str = "multi-pc.json";

/// What the user chose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Settings {
    /// Off unless the user turned it on: an agent that listens on the network is not
    /// something to find out about afterwards.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    /// The name shown to the other machines. Absent: the host name, read when needed,
    /// so that renaming the machine renames it here too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The shortcut that locks the pointer on the machine it is on, and unlocks it
    /// (`lbm_hook::hook::crossing`, "The pointer lock"). Absent: none, and nothing
    /// watches for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_lock_shortcut: Option<String>,
}

fn default_port() -> u16 {
    lbm_net::DEFAULT_PORT
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: false,
            port: default_port(),
            name: None,
            pointer_lock_shortcut: None,
        }
    }
}

impl Settings {
    /// The settings kept in `directory`. A missing or unreadable file is the defaults —
    /// the mode off — rather than an error: the agent has nothing better to do with it,
    /// and the next change replaces it.
    pub fn load(directory: &Path) -> Settings {
        std::fs::read(directory.join(SETTINGS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Writes the whole file beside its final name, then renames it over: a crash
    /// leaves the old settings or the new ones, never half of either.
    pub fn save(&self, directory: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        std::fs::create_dir_all(directory)?;
        let temporary = temporary_path(directory);
        let result = (|| {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, directory.join(SETTINGS_FILE))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    /// The name this machine goes by: the user's, else the host's.
    pub fn effective_name(&self) -> String {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(host_name)
    }
}

fn temporary_path(directory: &Path) -> PathBuf {
    directory.join(format!(".{SETTINGS_FILE}.{}.tmp", std::process::id()))
}

/// The machine's host name, or a placeholder when the system will not say.
pub fn host_name() -> String {
    #[cfg(windows)]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(not(windows))]
    let name = std::fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| std::fs::read_to_string("/etc/hostname"))
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok());
    name.map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "LittleBigMouse".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_file_the_mode_is_off_on_the_default_port() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::load(dir.path());
        assert_eq!(settings, Settings::default());
        assert!(!settings.enabled);
        assert_eq!(settings.port, lbm_net::DEFAULT_PORT);
        assert!(!settings.effective_name().is_empty());
        assert!(
            std::fs::read_dir(dir.path()).unwrap().next().is_none(),
            "reading writes nothing"
        );
    }

    #[test]
    fn the_settings_survive_a_restart_and_leave_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            enabled: true,
            port: 30000,
            name: Some("desk".into()),
            pointer_lock_shortcut: Some("Ctrl+Alt+L".into()),
        };
        settings.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()), settings);
        assert_eq!(settings.effective_name(), "desk");
        let text = std::fs::read_to_string(dir.path().join(SETTINGS_FILE)).unwrap();
        for key in [
            "\"Enabled\": true",
            "\"Port\": 30000",
            "\"Name\": \"desk\"",
            "\"PointerLockShortcut\": \"Ctrl+Alt+L\"",
        ] {
            assert!(text.contains(key), "{key} in {text}");
        }
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [SETTINGS_FILE]);
    }

    #[test]
    fn a_damaged_file_counts_as_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SETTINGS_FILE), "{ nope").unwrap();
        assert_eq!(Settings::load(dir.path()), Settings::default());
    }
}
