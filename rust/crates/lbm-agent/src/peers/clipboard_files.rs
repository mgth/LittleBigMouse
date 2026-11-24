//! Files on the shared clipboard (palier 3): what was copied here, listed for the offer
//! and read range by range for the other machine; what was copied there, fetched into
//! the reception folder before an application here is handed local paths.
//!
//! Both systems' file clipboards hold paths, not content (`text/uri-list`, `CF_HDROP`):
//! an application that pastes copies from those paths. So the files are fetched whole
//! first, into a folder of their own under the reception folder, and the paste copies
//! them from there. The disk is only ever touched in blocking tasks: the agent's runtime
//! has one thread, and the input relay runs on it.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lbm_net::link::LinkSender;
use lbm_net::wire::{Bulk, ClipFile, ClipFormat, Control};
use tokio::sync::mpsc;

/// The most entries one copy may list: past it the files are not offered.
pub const MAX_ENTRIES: usize = 10_000;
/// The largest range asked for at once: what is held in memory per file being fetched.
pub const RANGE: u64 = 4 * 1024 * 1024;
/// How long a range may go without a piece.
const PATIENCE: Duration = Duration::from_secs(20);

/// An entry of an offer, and where it is read from on this machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub entry: ClipFile,
    pub source: PathBuf,
}

/// The paths a clipboard holds, one per line, as the backends hand them over.
pub fn paths_of(text: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(text)
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The paths as the backends take them, one per line.
pub fn lines_of(paths: &[PathBuf]) -> Vec<u8> {
    paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

/// What `roots` are, folders walked: a folder before what is in it, links not followed
/// (a link to a folder could loop, and one to a file elsewhere is not what was copied),
/// nothing but files and folders. An error says why the files are not offered.
pub fn walk(roots: &[PathBuf], max_bytes: u64) -> Result<Vec<Listed>, String> {
    let mut listed = Vec::new();
    let mut total = 0u64;
    for root in roots {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{} has no name to paste it under", root.display()))?;
        visit(root, name, &mut listed, &mut total, max_bytes)?;
    }
    Ok(listed)
}

fn visit(
    path: &Path,
    relative: String,
    listed: &mut Vec<Listed>,
    total: &mut u64,
    max_bytes: u64,
) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    if listed.len() >= MAX_ENTRIES {
        return Err(format!("more than {MAX_ENTRIES} files were copied"));
    }
    if metadata.is_dir() {
        listed.push(Listed {
            entry: ClipFile {
                path: relative.clone(),
                size: 0,
                folder: true,
            },
            source: path.to_owned(),
        });
        let mut children: Vec<_> = std::fs::read_dir(path)
            .map_err(|error| format!("{} cannot be read: {error}", path.display()))?
            .filter_map(Result::ok)
            .collect();
        children.sort_by_key(|child| child.file_name());
        for child in children {
            let name = child.file_name().to_string_lossy().into_owned();
            visit(
                &child.path(),
                format!("{relative}/{name}"),
                listed,
                total,
                max_bytes,
            )?;
        }
    } else if metadata.is_file() {
        *total += metadata.len();
        if *total > max_bytes {
            return Err(super::super::clipboard::too_large(
                usize::try_from(max_bytes).unwrap_or(usize::MAX),
            ));
        }
        listed.push(Listed {
            entry: ClipFile {
                path: relative,
                size: metadata.len(),
                folder: false,
            },
            source: path.to_owned(),
        });
    }
    Ok(())
}

/// `length` bytes of `path` from `offset`, fewer at its end.
pub fn read_range(path: &Path, offset: u64, length: u64) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let mut data = Vec::new();
    file.take(length)
        .read_to_end(&mut data)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    Ok(data)
}

/// Where the entry `relative` of an offer goes under `root`, or `None` when it would go
/// anywhere else: an absolute path, `..`, a drive, an empty name. Names Windows cannot
/// hold are made fit for it there, wherever the offer comes from.
pub fn local_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = root.to_owned();
    let mut any = false;
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', '\0', ':']) {
            return None;
        }
        let part = if cfg!(windows) {
            windows_name(part)
        } else {
            part.to_owned()
        };
        if Path::new(&part)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        path.push(part);
        any = true;
    }
    any.then_some(path)
}

/// `name` as Windows can hold it: its forbidden characters replaced, no trailing dot or
/// space, and a device name (`CON`, `COM1`, …) prefixed.
pub fn windows_name(name: &str) -> String {
    let mut fit: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect();
    while fit.ends_with(['.', ' ']) {
        fit.pop();
    }
    if fit.is_empty() {
        fit.push('_');
    }
    let stem = fit
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if device {
        fit.insert(0, '_');
    }
    fit
}

/// Whether every entry of an offer has a place under any root, and their sum is within
/// `max_bytes`. Checked when the offer comes, so that a paste never starts what it
/// cannot finish.
pub fn acceptable(files: &[ClipFile], max_bytes: u64) -> Result<(), String> {
    if files.is_empty() {
        return Err("no file was listed".into());
    }
    if files.len() > MAX_ENTRIES {
        return Err(format!("more than {MAX_ENTRIES} files were copied"));
    }
    let root = Path::new("root");
    if let Some(bad) = files.iter().find(|f| local_path(root, &f.path).is_none()) {
        return Err(format!(
            "a file has a name that cannot be pasted: {}",
            bad.path
        ));
    }
    let total: u64 = files.iter().map(|f| f.size).fold(0, u64::saturating_add);
    if total > max_bytes {
        return Err(super::super::clipboard::too_large(
            usize::try_from(max_bytes).unwrap_or(usize::MAX),
        ));
    }
    Ok(())
}

/// The folder under `reception` the files of the offer held under `local` are fetched
/// to: its own, named by when and which, so that two pastes never mix.
pub fn folder_for(reception: &Path, local: u64) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    reception.join(format!("clipboard-{stamp}-{local}"))
}

/// What a fetch receives for one of its ranges.
#[derive(Debug)]
pub enum Piece {
    Data(Bulk),
    Abort(String),
}

/// A range under way: where its pieces go, and from whom they may come.
pub struct Route {
    pub machine_id: String,
    pub link: u64,
    /// The fetch it belongs to, by the offer's number here.
    pub fetch: u64,
    pub pieces: mpsc::UnboundedSender<Piece>,
}

/// The ranges under way, by transfer number: filled by the fetches, read by the sharing
/// as pieces arrive.
pub type Routes = Arc<Mutex<HashMap<u64, Route>>>;

/// What a fetch needs.
pub struct Fetch {
    pub link: LinkSender,
    pub machine_id: String,
    pub link_id: u64,
    /// The offer, by its number on the machine it comes from, and by its number here.
    pub serial: u64,
    pub local: u64,
    pub files: Vec<ClipFile>,
    pub folder: PathBuf,
    pub routes: Routes,
    pub transfers: Arc<AtomicU64>,
}

/// Fetches every file of the offer into `folder` (made already), and returns the paths
/// pasted: the entries at the top. On failure what was fetched is left to the caller to
/// remove.
pub async fn fetch(fetch: Fetch) -> Result<Vec<PathBuf>, String> {
    let mut tops = Vec::new();
    for (item, file) in fetch.files.iter().enumerate() {
        let path = local_path(&fetch.folder, &file.path)
            .ok_or_else(|| format!("a file has a name that cannot be pasted: {}", file.path))?;
        if !file.path.contains('/') {
            tops.push(path.clone());
        }
        if file.folder {
            blocking(move || {
                std::fs::create_dir_all(&path)
                    .map_err(|error| format!("{} cannot be made: {error}", path.display()))
            })
            .await?;
            continue;
        }
        let target = path.clone();
        blocking(move || {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("{} cannot be made: {error}", parent.display()))?;
            }
            std::fs::File::create(&target)
                .map(drop)
                .map_err(|error| format!("{} cannot be written: {error}", target.display()))
        })
        .await?;
        let mut offset = 0;
        while offset < file.size {
            let length = RANGE.min(file.size - offset);
            let data = range(&fetch, item as u32, offset, length).await?;
            if data.len() as u64 != length {
                return Err(format!("{} came short", file.path));
            }
            let target = path.clone();
            blocking(move || {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&target)
                    .and_then(|mut f| f.write_all(&data))
                    .map_err(|error| format!("{} cannot be written: {error}", target.display()))
            })
            .await?;
            offset += length;
        }
    }
    Ok(tops)
}

/// One range of one file.
async fn range(fetch: &Fetch, item: u32, offset: u64, length: u64) -> Result<Vec<u8>, String> {
    let transfer = fetch.transfers.fetch_add(1, Ordering::Relaxed) + 1;
    let (pieces, mut received) = mpsc::unbounded_channel();
    fetch.routes.lock().unwrap().insert(
        transfer,
        Route {
            machine_id: fetch.machine_id.clone(),
            link: fetch.link_id,
            fetch: fetch.local,
            pieces,
        },
    );
    let result = async {
        fetch
            .link
            .control(&Control::ClipboardRequest {
                serial: fetch.serial,
                format: ClipFormat::Files,
                transfer,
                item: Some(item),
                offset,
                length: Some(length),
            })
            .await
            .map_err(|_| "the other machine left".to_owned())?;
        let mut data = Vec::with_capacity(length as usize);
        loop {
            match tokio::time::timeout(PATIENCE, received.recv()).await {
                Err(_) => return Err("the other machine did not send the files in time".into()),
                Ok(None) => return Err("the other machine left".into()),
                Ok(Some(Piece::Abort(reason))) => return Err(reason),
                Ok(Some(Piece::Data(piece))) => {
                    if data.len() + piece.data.len() > length as usize {
                        return Err("the other machine sent more than asked".into());
                    }
                    data.extend_from_slice(&piece.data);
                    if piece.last {
                        return Ok(data);
                    }
                }
            }
        }
    }
    .await;
    fetch.routes.lock().unwrap().remove(&transfer);
    result
}

/// `work` on a thread that may block.
pub async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|_| Err("the work on the files stopped".into()))
}

/// The reception folder unless the user chose another: `LittleBigMouse` in the user's
/// downloads.
pub fn default_reception() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("Downloads").join("LittleBigMouse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_goes_under_the_root_and_nowhere_else() {
        let root = Path::new("/r");
        assert_eq!(
            local_path(root, "a/b.txt"),
            Some(PathBuf::from("/r/a/b.txt"))
        );
        for bad in [
            "",
            "/etc/passwd",
            "a//b",
            "../x",
            "a/../../x",
            "a/./b",
            "C:/x",
            "a\\..\\x",
            "nul\0",
        ] {
            assert_eq!(local_path(root, bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_name_windows_cannot_hold_is_made_fit() {
        assert_eq!(windows_name("a<b>?.txt"), "a_b__.txt");
        assert_eq!(windows_name("trailing. "), "trailing");
        assert_eq!(windows_name("CON"), "_CON");
        assert_eq!(windows_name("com1.txt"), "_com1.txt");
        assert_eq!(windows_name("COMPUTER"), "COMPUTER");
        assert_eq!(windows_name("..."), "_");
    }

    #[test]
    fn copied_folders_are_listed_before_their_content_and_links_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("top");
        std::fs::create_dir_all(top.join("sub")).unwrap();
        std::fs::write(top.join("b.txt"), b"bb").unwrap();
        std::fs::write(top.join("sub/a.bin"), [0u8; 10]).unwrap();
        std::fs::write(dir.path().join("lone.txt"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path(), top.join("loop")).unwrap();
        let listed = walk(&[top.clone(), dir.path().join("lone.txt")], 1000).unwrap();
        let entries: Vec<(&str, u64, bool)> = listed
            .iter()
            .map(|l| (l.entry.path.as_str(), l.entry.size, l.entry.folder))
            .collect();
        assert_eq!(
            entries,
            [
                ("top", 0, true),
                ("top/b.txt", 2, false),
                ("top/sub", 0, true),
                ("top/sub/a.bin", 10, false),
                ("lone.txt", 1, false),
            ]
        );
        assert!(walk(&[top], 5).unwrap_err().contains("bound"));
    }

    #[test]
    fn a_range_is_read_where_asked_and_short_at_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, b"0123456789").unwrap();
        assert_eq!(read_range(&file, 3, 4).unwrap(), b"3456");
        assert_eq!(read_range(&file, 8, 4).unwrap(), b"89");
    }

    #[test]
    fn an_offer_is_refused_whole_for_one_bad_name_or_its_size() {
        let file = |path: &str, size| ClipFile {
            path: path.into(),
            size,
            folder: false,
        };
        assert!(acceptable(&[file("a", 1), file("b/c", 2)], 10).is_ok());
        assert!(acceptable(&[file("a", 1), file("../c", 2)], 10).is_err());
        assert!(acceptable(&[file("a", 6), file("b", 6)], 10).is_err());
        assert!(acceptable(&[], 10).is_err());
    }
}
