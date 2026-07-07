//! Locations of the daemon's data files.

use std::path::PathBuf;

/// Path to `%LOCALAPPDATA%\Mgth\LittleBigMouse\<name>`.
///
/// The C# UI writes both `Current.xml` and `Excluded.txt` under
/// `LocalApplicationData` (`LittleBigMouseClientService`), so the daemon reads
/// them from there. This deliberately differs from the C++ daemon, which read
/// them from `%ProgramData%` — a path nothing writes to, so its standalone load
/// and process exclusion never saw the UI's files (a latent C++ bug). See the
/// port plan's open decision (Pièges & décisions #6).
pub fn lbm_data_file(name: &str) -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let mut path = PathBuf::from(base);
    path.push("Mgth");
    path.push("LittleBigMouse");
    path.push(name);
    Some(path)
}
