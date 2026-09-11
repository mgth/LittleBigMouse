//! The application's per-user directories, one convention per OS — port of
//! `LittleBigMouse.Plugins.Core/LbmPaths.cs`.
//!
//! Windows keeps the historical `%LOCALAPPDATA%\Mgth\LittleBigMouse` for everything
//! (settings live in the registry there). Linux splits per the XDG spec: runtime data
//! (`Current.xml`, `Excluded.txt` — read by the daemon) under
//! `~/.local/share/LittleBigMouse`, settings under `~/.config/LittleBigMouse`. The
//! daemon resolves the same data directory on its side (`lbm-hook`,
//! `platform::paths::lbm_data_file`).

use std::ffi::OsString;
use std::path::PathBuf;

/// The last component of both directories.
const APP_DIR: &str = "LittleBigMouse";

/// C# `LbmPaths.DataDir`: where the files the daemon reads live.
pub fn data_dir() -> PathBuf {
    data_dir_in(&env_var, &home_dir)
}

/// C# `LbmPaths.ConfigDir`: where the settings live — the JSON store on Linux, the
/// data directory itself on Windows.
pub fn config_dir() -> PathBuf {
    config_dir_in(&env_var, &home_dir)
}

type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;
type Home<'a> = &'a dyn Fn() -> Option<PathBuf>;

fn env_var(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

/// C# `Environment.GetFolderPath(SpecialFolder.LocalApplicationData)`, which asks the
/// shell for the known folder; its environment twin here, as in the daemon. They only
/// differ when `LOCALAPPDATA` was overridden or removed. As in C#, an unknown folder
/// leaves a relative path.
#[cfg(windows)]
fn data_dir_in(env: Env, _home: Home) -> PathBuf {
    let mut dir = PathBuf::from(env("LOCALAPPDATA").unwrap_or_default());
    dir.push("Mgth");
    dir.push(APP_DIR);
    dir
}

#[cfg(windows)]
fn config_dir_in(env: Env, home: Home) -> PathBuf {
    data_dir_in(env, home)
}

#[cfg(not(windows))]
fn data_dir_in(env: Env, home: Home) -> PathBuf {
    xdg_home(env, home, "XDG_DATA_HOME", &[".local", "share"]).join(APP_DIR)
}

#[cfg(not(windows))]
fn config_dir_in(env: Env, home: Home) -> PathBuf {
    xdg_home(env, home, "XDG_CONFIG_HOME", &[".config"]).join(APP_DIR)
}

/// C# `LbmPaths.XdgHome`: the variable when it is set and not empty — taken as is,
/// even relative — else `fallback` under the home directory (C#
/// `SpecialFolder.UserProfile`: `$HOME`, else the password database; a relative path
/// when neither is known).
#[cfg(not(windows))]
fn xdg_home(env: Env, home: Home, variable: &str, fallback: &[&str]) -> PathBuf {
    match env(variable) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let mut dir = home().unwrap_or_default();
            dir.extend(fallback);
            dir
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    fn home() -> Option<PathBuf> {
        Some(PathBuf::from("/home/u"))
    }

    #[cfg(not(windows))]
    #[test]
    fn linux_follows_xdg() {
        let set = env(&[("XDG_CONFIG_HOME", "/cfg"), ("XDG_DATA_HOME", "rel/data")]);
        assert_eq!(config_dir_in(&set, &home), Path::new("/cfg/LittleBigMouse"));
        assert_eq!(
            data_dir_in(&set, &home),
            Path::new("rel/data/LittleBigMouse")
        );

        for vars in [
            &[][..],
            &[("XDG_CONFIG_HOME", ""), ("XDG_DATA_HOME", "")][..],
        ] {
            let unset = env(vars);
            assert_eq!(
                config_dir_in(&unset, &home),
                Path::new("/home/u/.config/LittleBigMouse")
            );
            assert_eq!(
                data_dir_in(&unset, &home),
                Path::new("/home/u/.local/share/LittleBigMouse")
            );
        }

        let homeless = || None;
        assert_eq!(
            config_dir_in(&env(&[]), &homeless),
            Path::new(".config/LittleBigMouse")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_keeps_everything_under_local_app_data() {
        let set = env(&[("LOCALAPPDATA", r"C:\Users\u\AppData\Local")]);
        let expected = Path::new(r"C:\Users\u\AppData\Local\Mgth\LittleBigMouse");
        assert_eq!(data_dir_in(&set, &home), expected);
        assert_eq!(config_dir_in(&set, &home), expected);
    }
}
