//! Locations of the daemon's data files.

use std::path::PathBuf;

/// Path to `$XDG_DATA_HOME/LittleBigMouse/<name>` (default
/// `~/.local/share/LittleBigMouse/<name>`).
///
/// Must match where the agent writes `Excluded.txt` (C#: `LbmPaths.DataDir`), which
/// is the only file the daemon still reads from here — `Current.xml` went with the
/// standalone mode. Note the Linux convention drops the `Mgth` vendor level used on
/// Windows.
pub fn lbm_data_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                let mut p = PathBuf::from(home);
                p.push(".local");
                p.push("share");
                p
            })
        })?;

    let mut path = base;
    path.push("LittleBigMouse");
    path.push(name);
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_under_data_home() {
        // Whichever env var wins, the tail must be LittleBigMouse/<name>.
        let path = lbm_data_file("Excluded.txt").expect("HOME should be set in tests");
        assert!(path.ends_with("LittleBigMouse/Excluded.txt"));
    }
}
