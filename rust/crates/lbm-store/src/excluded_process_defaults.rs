//! The canonical default exclusion list — port of
//! `LittleBigMouse.DisplayLayout/Monitors/ExcludedProcessDefaults.cs`.
//!
//! Game-launcher install-path fragments whose windows LBM should leave alone, matched
//! by the daemon as substrings of the foreground process path. Single source of truth
//! for the seed file, the one-time top-up
//! ([`ExcludedListPersistence`](crate::ExcludedListPersistence)) and the UI's "Add
//! defaults" button. Per-OS: the fragments describe install conventions, which differ
//! between Windows and Linux.

/// C# `ExcludedProcessDefaults.Version`: bumped whenever entries are added to [`ALL`],
/// to drive the one-time migration that tops up the lists of users who kept the stock
/// defaults. A later manual removal is respected (the migration only runs once per
/// version). History: 1 = `\XboxGames\` (#494); 2 = per-OS lists, Linux entries
/// added (#515).
pub const VERSION: i32 = 2;

/// C# `ExcludedProcessDefaults.Header`: the comment written atop a freshly seeded file
/// (`:`-prefixed lines are ignored by the daemon).
pub const HEADER: &str = ":Excluded processes";

/// C# `ExcludedProcessDefaults.LegacyV0`: the defaults that shipped before
/// [`VERSION`] 1, on both OSes.
pub const LEGACY_V0: &[&str] = &[r"\Epic Games\", r"\steamapps\", r"\Riot Games\"];

/// C# `ExcludedProcessDefaults.Windows`: [`LEGACY_V0`], then `\XboxGames\`, added in
/// version 1 (#494) — Xbox / Game Pass titles install under `…\XboxGames\…\Content\`
/// whatever the chosen drive.
pub const WINDOWS: &[&str] = &[
    r"\Epic Games\",
    r"\steamapps\",
    r"\Riot Games\",
    r"\XboxGames\",
];

/// C# `ExcludedProcessDefaults.Linux`: the Linux defaults, in the native separator
/// style. The daemon matches separator-insensitively (`\` == `/`), so these also cover
/// the Windows-style command lines Wine/Proton games expose
/// (`Z:\…\steamapps\common\…\Game.exe`, `C:\Riot Games\…`). No `/XboxGames/`: Game
/// Pass does not exist on Linux.
pub const LINUX: &[&str] = &[
    // Steam, native and Proton (…/Steam/steamapps/common/…)
    "/steamapps/",
    // Epic launcher inside a Wine prefix (C:\Program Files\Epic Games\…)
    "/Epic Games/",
    // Riot inside a Wine prefix (C:\Riot Games\…)
    "/Riot Games/",
    // Heroic Games Launcher library (default ~/Games/Heroic)
    "/Heroic/",
    // ~/Games — Lutris default install dir, common convention
    "/Games/",
];

/// C# `ExcludedProcessDefaults.All`: the current default entries for the OS this runs
/// on.
#[cfg(windows)]
pub const ALL: &[&str] = WINDOWS;

/// C# `ExcludedProcessDefaults.All`: the current default entries for the OS this runs
/// on (every OS but Windows gets the Linux list, as in C#).
#[cfg(not(windows))]
pub const ALL: &[&str] = LINUX;

/// C# `ExcludedProcessDefaults.ContainsEntry`: separator-insensitive containment (`\`
/// == `/`, like the daemon's matching), otherwise exact and case-sensitive. A list
/// seeded with the pre-version-2 Windows-style entries already covers the slash-style
/// Linux defaults — the top-up must not duplicate them.
pub fn contains_entry<I>(list: I, entry: &str) -> bool
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let entry = normalize(entry);
    list.into_iter().any(|e| normalize(e.as_ref()) == entry)
}

/// C# `ExcludedProcessDefaults.Normalize`.
fn normalize(s: &str) -> String {
    s.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_extends_the_legacy_list() {
        assert_eq!(&WINDOWS[..LEGACY_V0.len()], LEGACY_V0);
        assert_eq!(&WINDOWS[LEGACY_V0.len()..], [r"\XboxGames\"]);
    }

    #[test]
    fn all_is_the_list_of_this_os() {
        let expected = if cfg!(windows) { WINDOWS } else { LINUX };
        assert_eq!(ALL, expected);
    }

    #[test]
    fn contains_entry_ignores_the_separator_style_only() {
        let list = [r"\Epic Games\", "/steamapps/"];
        assert!(contains_entry(list, "/Epic Games/"));
        assert!(contains_entry(list, r"\steamapps\"));
        assert!(!contains_entry(list, "/epic games/"));
        assert!(!contains_entry(list, "Epic Games"));
        assert!(!contains_entry(Vec::<String>::new(), "/Games/"));
    }

    #[test]
    fn legacy_windows_entries_cover_three_linux_defaults() {
        let covered: Vec<&str> = LINUX
            .iter()
            .copied()
            .filter(|e| contains_entry(LEGACY_V0, e))
            .collect();
        assert_eq!(covered, ["/steamapps/", "/Epic Games/", "/Riot Games/"]);
    }
}
