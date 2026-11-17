//! Process helpers — libproc counterpart of the Win32 ToolHelp walk and the
//! Linux /proc reads.

use std::ffi::c_void;

/// `PROC_PIDPATHINFO_MAXSIZE` (4 * MAXPATHLEN).
const PATH_MAX_SIZE: usize = 4096;

extern "C" {
    // libproc, part of libSystem: linked by default.
    fn proc_pidpath(pid: i32, buffer: *mut c_void, buffersize: u32) -> i32;
}

fn pid_path(pid: i32) -> Option<String> {
    if pid <= 0 {
        return None;
    }
    let mut buffer = vec![0u8; PATH_MAX_SIZE];
    let len = unsafe { proc_pidpath(pid, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32) };
    if len <= 0 {
        return None;
    }
    buffer.truncate(len as usize);
    String::from_utf8(buffer).ok().filter(|s| !s.is_empty())
}

/// Executable path of the parent process, used to tell "launched by the UI"
/// (path contains "LittleBigMouse") from standalone/autostart.
pub fn parent_process_path() -> Option<String> {
    pid_path(unsafe { libc::getppid() })
}

/// "Who is in front right now?" needs AppKit (`NSWorkspace.frontmostApplication`),
/// which this daemon does not link. `None` is "unknown", not "nobody": the caller
/// leaves the pause flag as it stands rather than guess (see `hook::adopt_foreground`).
pub fn foreground_path_now() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_path_resolves() {
        let path = parent_process_path().expect("parent path");
        assert!(path.starts_with('/'));
    }
}
