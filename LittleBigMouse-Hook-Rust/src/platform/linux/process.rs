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
}
