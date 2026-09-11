//! Where the hook listens: one definition for the daemon and its clients.
//!
//! Linux: a Unix socket named [`SOCKET_NAME`] in `$XDG_RUNTIME_DIR`, or in the
//! LittleBigMouse data directory when there is none. Windows: a named pipe per logon
//! session ([`pipe_name`]). [`ENDPOINT_VARIABLE`] overrides both, for tests and
//! side-by-side instances. The C# client (`LocalIpcClient`) spells the same names.

use std::path::{Path, PathBuf};

/// `LBM_HOOK_ENDPOINT`: the full endpoint (socket path, or `\\.\pipe\...` name) to
/// use instead of the per-session default.
pub const ENDPOINT_VARIABLE: &str = "LBM_HOOK_ENDPOINT";

/// The Linux socket's file name.
pub const SOCKET_NAME: &str = "littlebigmouse-v1.sock";

/// The Linux socket: in `runtime_dir` (`$XDG_RUNTIME_DIR`) when there is one and it
/// is not empty, else in `data_dir` (the LittleBigMouse data directory).
pub fn socket_path(runtime_dir: Option<&Path>, data_dir: Option<&Path>) -> Option<PathBuf> {
    runtime_dir
        .filter(|dir| !dir.as_os_str().is_empty())
        .or(data_dir)
        .map(|dir| dir.join(SOCKET_NAME))
}

/// The Windows pipe of a logon session.
pub fn pipe_name(session: u32) -> String {
    format!(r"\\.\pipe\LittleBigMouse-v1-session-{session}")
}

/// The endpoint [`ENDPOINT_VARIABLE`] names, if it is set.
pub fn from_environment() -> Option<String> {
    std::env::var(ENDPOINT_VARIABLE).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_the_ones_the_csharp_client_spells() {
        assert_eq!(
            socket_path(Some(Path::new("/run/user/1000")), None),
            Some(PathBuf::from("/run/user/1000/littlebigmouse-v1.sock"))
        );
        // An empty XDG_RUNTIME_DIR counts as none.
        assert_eq!(
            socket_path(
                Some(Path::new("")),
                Some(Path::new("/home/u/.local/share/LittleBigMouse"))
            ),
            Some(PathBuf::from(
                "/home/u/.local/share/LittleBigMouse/littlebigmouse-v1.sock"
            ))
        );
        assert_eq!(socket_path(None, None), None);
        assert_eq!(pipe_name(2), r"\\.\pipe\LittleBigMouse-v1-session-2");
    }
}
