//! Locations of the daemon's data files.

use std::path::PathBuf;

/// Path to `%LOCALAPPDATA%\Mgth\LittleBigMouse\<name>`.
///
/// `Excluded.txt` is written under `LocalApplicationData` (C#:
/// `LittleBigMouseClientService`, now the agent), so the daemon reads it from there.
/// This deliberately differs from the C++ daemon, which read from `%ProgramData%` — a
/// path nothing writes to, so its process exclusion never saw the UI's files (a latent
/// C++ bug). See the port plan's open decision (Pièges & décisions #6).
///
/// `Current.xml` used to live here too, for the daemon's standalone mode; both went
/// with phase 5.
pub fn lbm_data_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let mut path = PathBuf::from(base);
    path.push("Mgth");
    path.push("LittleBigMouse");
    path.push(name);
    Some(path)
}
