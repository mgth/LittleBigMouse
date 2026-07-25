//! Process helpers — /proc-based counterpart of the Win32 ToolHelp walk.

/// Executable path of the parent process, used to tell "launched by the UI"
/// (path contains "LittleBigMouse") from standalone/autostart.
pub fn parent_process_path() -> Option<String> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // /proc/<pid>/stat: "pid (comm) state ppid ..." — comm may contain spaces
    // and parentheses, so split after the LAST ')'.
    let after_comm = &stat[stat.rfind(')')? + 1..];
    let ppid = after_comm.split_whitespace().nth(1)?;

    std::fs::read_link(format!("/proc/{ppid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Identity string of a foreground process, for exclusion matching and the UI's
/// seen-processes list (Windows reports the exe full path there).
///
/// For a native app `/proc/<pid>/exe` is the equivalent. For a Wine/Proton game
/// it is not: every Wine process shares the same exe (`wine64-preloader`), and
/// the game's identity lives in the command line — Wine rewrites argv to the
/// Windows command (`Z:\...\steamapps\common\...\EscapeFromTarkov.exe`), which
/// conveniently also carries the `\steamapps\` default-exclusion component. So:
/// prefer the first `.exe` argument when the exe is Wine, and fall back
/// exe → argv[0] → comm as readability degrades (`exe` needs same-uid).
pub fn foreground_path(pid: u32) -> Option<String> {
    let exe = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    let argv: Vec<String> = std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|bytes| {
            bytes
                .split(|b| *b == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect()
        })
        .unwrap_or_default();
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string());
    compose_identity(exe.as_deref(), &argv, comm.as_deref())
}

/// See [`foreground_path`]; separated from the /proc reads for testability.
fn compose_identity(exe: Option<&str>, argv: &[String], comm: Option<&str>) -> Option<String> {
    // Unreadable exe (permissions) is treated like Wine: identify via argv.
    let wine = exe.is_none_or(|e| {
        let base = e.rsplit('/').next().unwrap_or(e);
        base.starts_with("wine") // wine, wine64, wine-preloader, wine64-preloader
    });
    if wine {
        if let Some(arg) = argv
            .iter()
            .find(|a| a.len() >= 4 && a[a.len() - 4..].eq_ignore_ascii_case(".exe"))
        {
            return Some(arg.clone());
        }
    }
    exe.map(str::to_owned)
        .or_else(|| argv.first().cloned())
        .or_else(|| comm.map(str::to_owned))
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_path_resolves() {
        // The test runner's parent is something real (cargo, a shell...): the
        // walk itself must succeed and return a non-empty absolute path.
        let path = parent_process_path().expect("parent path");
        assert!(path.starts_with('/'));
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn native_process_identity_is_the_exe_path() {
        let id = compose_identity(
            Some("/home/u/Steam/steamapps/common/Game/game.x86_64"),
            &args(&["./game.x86_64", "-nolauncher"]),
            Some("game.x86_64"),
        );
        assert_eq!(id.as_deref(), Some("/home/u/Steam/steamapps/common/Game/game.x86_64"));
    }

    #[test]
    fn wine_process_identity_is_the_windows_exe_argument() {
        // The #515 case: comm is truncated (EscapeFromTarko), exe is the shared
        // preloader — the .exe argument is the only useful identity.
        let id = compose_identity(
            Some("/home/u/Steam/steamapps/common/Proton 9.0/files/bin/wine64-preloader"),
            &args(&[r"Z:\home\u\Steam\steamapps\common\EFT\EscapeFromTarkov.exe", "-something"]),
            Some("EscapeFromTarko"),
        );
        assert_eq!(
            id.as_deref(),
            Some(r"Z:\home\u\Steam\steamapps\common\EFT\EscapeFromTarkov.exe")
        );
    }

    #[test]
    fn wine_without_exe_argument_falls_back_to_the_exe_path() {
        let id = compose_identity(
            Some("/usr/bin/wine64-preloader"),
            &args(&["winecfg"]),
            Some("winecfg"),
        );
        assert_eq!(id.as_deref(), Some("/usr/bin/wine64-preloader"));
    }

    #[test]
    fn unreadable_proc_degrades_argv_then_comm() {
        let id = compose_identity(None, &args(&["/opt/app/bin/app"]), Some("app"));
        assert_eq!(id.as_deref(), Some("/opt/app/bin/app"));
        let id = compose_identity(None, &[], Some("app"));
        assert_eq!(id.as_deref(), Some("app"));
        assert_eq!(compose_identity(None, &[], None), None);
    }
}
