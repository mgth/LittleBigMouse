//! Files on the shared clipboard (palier 3): what was copied here, listed for the offer
//! and read range by range for the other machine; what was copied there, fetched into
//! the reception folder before an application here is handed local paths.
//!
//! Both systems' file clipboards hold paths, not content (`text/uri-list`, `CF_HDROP`):
//! an application that pastes copies from those paths. So the files are fetched whole
//! first, into a folder of their own under the reception folder, and the paste copies
//! them from there. The disk is only ever touched in blocking tasks: the agent's runtime
//! has one thread, and the input relay runs on it.

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
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
    /// The file as it was listed: a read of it checks it is still that one.
    /// The file as it was listed, `None` for a folder: a read of it checks it is still
    /// that one.
    pub stamp: Option<Stamp>,
}

/// What tells one version of a file from another: its size and time of change, and the
/// file itself — device and inode on Unix, volume and file index on Windows — so that a
/// file replaced by another under the same name is told apart even with the same size
/// and a time kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    file: (u64, u64),
}

impl Stamp {
    /// The stamp of an open file, read from its handle, not its path.
    pub fn of(file: &std::fs::File) -> std::io::Result<Stamp> {
        let metadata = file.metadata()?;
        Ok(Stamp {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            file: identity(file, &metadata)?,
        })
    }
}

#[cfg(unix)]
fn identity(_: &std::fs::File, metadata: &std::fs::Metadata) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn identity(file: &std::fs::File, _: &std::fs::Metadata) -> std::io::Result<(u64, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle of an open file, for as long as the call.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(std::io::Error::other)?;
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

#[cfg(not(any(unix, windows)))]
fn identity(_: &std::fs::File, _: &std::fs::Metadata) -> std::io::Result<(u64, u64)> {
    Ok((0, 0))
}

/// The paths a clipboard holds, as the backends hand them over: separated by NUL, the
/// one byte no path holds on either system, each kept exactly as the system has it (no
/// line splitting, no trimming, no character lost).
pub fn unpack_paths(packed: &[u8]) -> Vec<PathBuf> {
    packed
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        // SAFETY: the bytes come from `pack_paths` in this process, split only at the
        // ASCII NUL it put between them, which `from_encoded_bytes_unchecked` allows.
        .map(|path| PathBuf::from(unsafe { OsStr::from_encoded_bytes_unchecked(path) }))
        .collect()
}

/// The paths as the backends take them: see [`unpack_paths`].
pub fn pack_paths(paths: &[PathBuf]) -> Vec<u8> {
    let mut packed = Vec::new();
    for path in paths {
        if !packed.is_empty() {
            packed.push(0);
        }
        packed.extend_from_slice(path.as_os_str().as_encoded_bytes());
    }
    packed
}

/// `name`, if it can cross: an offer carries names as UTF-8, and a Linux name that is not
/// is refused rather than sent mangled.
fn crossing_name(name: &OsStr, path: &Path) -> Result<String, String> {
    name.to_str().map(str::to_owned).ok_or_else(|| {
        format!(
            "{} has a name that cannot be sent to the other machine",
            path.display()
        )
    })
}

/// What `roots` are, folders walked: a folder before what is in it, links not followed
/// (a link to a folder could loop, and one to a file elsewhere is not what was copied),
/// nothing but files and folders. An error says why the files are not offered.
pub fn walk(roots: &[PathBuf], max_bytes: u64) -> Result<Vec<Listed>, String> {
    walk_within(roots, max_bytes, MAX_ENTRIES)
}

/// [`walk`], at most `max_entries` entries.
pub fn walk_within(
    roots: &[PathBuf],
    max_bytes: u64,
    max_entries: usize,
) -> Result<Vec<Listed>, String> {
    let mut walk = Walk {
        listed: Vec::new(),
        total: 0,
        max_bytes,
        max_entries,
    };
    for root in roots {
        let name = root
            .file_name()
            .ok_or_else(|| format!("{} has no name to paste it under", root.display()))?;
        let name = crossing_name(name, root)?;
        walk.visit(root, name)?;
    }
    Ok(walk.listed)
}

struct Walk {
    listed: Vec<Listed>,
    total: u64,
    max_bytes: u64,
    max_entries: usize,
}

impl Walk {
    fn too_many(&self) -> String {
        format!("more than {} files were copied", self.max_entries)
    }

    fn visit(&mut self, path: &Path, relative: String) -> Result<(), String> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
        if self.listed.len() >= self.max_entries {
            return Err(self.too_many());
        }
        if metadata.is_dir() {
            self.listed.push(Listed {
                entry: ClipFile {
                    path: relative.clone(),
                    size: 0,
                    folder: true,
                },
                source: path.to_owned(),
                stamp: None,
            });
            // Within what is left of the budget, before anything is sorted: a folder of
            // a million files is given up after the first entries over the bound.
            let room = self.max_entries - self.listed.len();
            let mut children = Vec::new();
            for child in std::fs::read_dir(path)
                .map_err(|error| format!("{} cannot be read: {error}", path.display()))?
            {
                let Ok(child) = child else { continue };
                if children.len() >= room {
                    return Err(self.too_many());
                }
                children.push(child);
            }
            children.sort_by_key(|child| child.file_name());
            for child in children {
                let name = crossing_name(&child.file_name(), &child.path())?;
                self.visit(&child.path(), format!("{relative}/{name}"))?;
            }
        } else if metadata.is_file() {
            // The size announced is the one of the very file whose identity is kept: the
            // path's, read before it was opened, may be another version's.
            let stamp = std::fs::File::open(path)
                .and_then(|file| Stamp::of(&file))
                .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
            self.total += stamp.len;
            if self.total > self.max_bytes {
                return Err(super::super::clipboard::too_large(
                    usize::try_from(self.max_bytes).unwrap_or(usize::MAX),
                ));
            }
            self.listed.push(Listed {
                entry: ClipFile {
                    path: relative,
                    size: stamp.len,
                    folder: false,
                },
                source: path.to_owned(),
                stamp: Some(stamp),
            });
        }
        Ok(())
    }
}

/// `length` bytes of the file `listed` from `offset`, fewer at its end — provided it is
/// still the file that was listed: not a link now, the same file, size and time. A file
/// changed between two ranges would otherwise paste as the start of one version and the
/// end of another.
pub fn read_range(listed: &Listed, offset: u64, length: u64) -> Result<Vec<u8>, String> {
    let path = &listed.source;
    let changed = || format!("{} changed since it was copied", path.display());
    let link = std::fs::symlink_metadata(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    if !link.is_file() {
        return Err(changed());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let same = |file: &std::fs::File| {
        Stamp::of(file).ok().as_ref() == listed.stamp.as_ref() && listed.stamp.is_some()
    };
    if !same(&file) {
        return Err(changed());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let mut data = Vec::new();
    (&mut file)
        .take(length)
        .read_to_end(&mut data)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    // And still that one once read, at that path: a change under the read, or another
    // file renamed over it meanwhile, is a change too.
    let still = std::fs::File::open(path).is_ok_and(|again| same(&again));
    if same(&file) && still {
        Ok(data)
    } else {
        Err(changed())
    }
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
    // Windows and macOS file systems ignore case: `Photo.jpg` and `photo.jpg` are one
    // file there.
    acceptable_on(files, max_bytes, cfg!(any(windows, target_os = "macos")))
}

/// [`acceptable`] on a file system that ignores case or not. Two entries that would land
/// on the same path refuse the whole offer: the second would overwrite the first and the
/// paste would succeed with a file lost.
pub fn acceptable_on(files: &[ClipFile], max_bytes: u64, fold_case: bool) -> Result<(), String> {
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
    let mut taken = HashSet::new();
    for file in files {
        let path = local_path(root, &file.path).expect("checked above");
        let key = path.to_string_lossy().into_owned();
        let key = if fold_case { key.to_lowercase() } else { key };
        if !taken.insert(key) {
            return Err(format!(
                "two files would have the same name here: {}",
                file.path
            ));
        }
    }
    let total: u64 = files.iter().map(|f| f.size).fold(0, u64::saturating_add);
    if total > max_bytes {
        return Err(super::super::clipboard::too_large(
            usize::try_from(max_bytes).unwrap_or(usize::MAX),
        ));
    }
    Ok(())
}

/// A new folder under `reception` for the files of the offer held under `local`,
/// created here and now: a name already taken — by an earlier paste, or another agent
/// sharing the reception folder — is never reused, so that a paste only ever writes,
/// and on failure removes, a folder of its own. Blocking.
pub fn reserve(reception: &Path, local: u64) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    std::fs::create_dir_all(reception)
        .map_err(|error| format!("{} cannot be made: {error}", reception.display()))?;
    let base = format!("clipboard-{stamp}-{}-{local}", std::process::id());
    for n in 0..1000 {
        let name = if n == 0 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let folder = reception.join(name);
        match std::fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("{} cannot be made: {error}", folder.display())),
        }
    }
    Err(format!("no free folder under {}", reception.display()))
}

/// The folder a fetch reserved, once it has: what an abandon may remove, and nothing
/// else.
pub type Reserved = Arc<Mutex<Option<PathBuf>>>;

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
    /// Reserved already ([`reserve`]).
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
        // Created only if absent, folders as files: the folder of this paste is new, so
        // something there already is another entry that landed on the same name — a
        // volume that ignores case where this system does not, a share — and is never
        // written over.
        let clash = |path: &Path| {
            format!(
                "two files would have the same name here: {}",
                path.display()
            )
        };
        if file.folder {
            blocking(move || match std::fs::create_dir(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    Err(clash(&path))
                }
                Err(error) => Err(format!("{} cannot be made: {error}", path.display())),
            })
            .await?;
            continue;
        }
        let target = path.clone();
        blocking(move || {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
            {
                Ok(_) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    Err(clash(&target))
                }
                Err(error) => Err(format!("{} cannot be written: {error}", target.display())),
            }
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
        let listed = &walk(&[file], 100).unwrap()[0];
        assert_eq!(read_range(listed, 3, 4).unwrap(), b"3456");
        assert_eq!(read_range(listed, 8, 4).unwrap(), b"89");
    }

    #[test]
    fn a_file_changed_or_replaced_since_it_was_listed_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, b"first version").unwrap();
        let listed = walk(std::slice::from_ref(&file), 100).unwrap().remove(0);
        // Grown.
        std::fs::write(&file, b"second, longer version").unwrap();
        assert!(read_range(&listed, 0, 4).unwrap_err().contains("changed"));
        // Replaced by another file of the same size, renamed over it: told apart by
        // the file itself, which only Unix gives here.
        #[cfg(unix)]
        {
            std::fs::write(&file, b"first version").unwrap();
            let listed = walk(std::slice::from_ref(&file), 100).unwrap().remove(0);
            let other = dir.path().join("g");
            std::fs::write(&other, b"other version").unwrap();
            std::fs::rename(&other, &file).unwrap();
            assert!(read_range(&listed, 0, 4).unwrap_err().contains("changed"));
        }
        // A link where the file was.
        #[cfg(unix)]
        {
            let listed = walk(std::slice::from_ref(&file), 100).unwrap().remove(0);
            std::fs::remove_file(&file).unwrap();
            std::os::unix::fs::symlink(dir.path().join("elsewhere"), &file).unwrap();
            assert!(read_range(&listed, 0, 4).unwrap_err().contains("changed"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn paths_cross_between_the_clipboard_and_the_sharing_exactly() {
        use std::os::unix::ffi::OsStrExt;
        let paths = vec![
            PathBuf::from("/tmp/rapport "),
            PathBuf::from("/tmp/two\nlines"),
            PathBuf::from(OsStr::from_bytes(b"/tmp/not-utf8-\xff")),
        ];
        assert_eq!(unpack_paths(&pack_paths(&paths)), paths);
        // But a name that is not UTF-8 is not offered: the wire carries names as text.
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join(OsStr::from_bytes(b"bad-\xff"));
        std::fs::write(&bad, b"x").unwrap();
        assert!(walk(&[bad], 100).unwrap_err().contains("cannot be sent"));
    }

    #[test]
    fn a_folder_over_the_entry_bound_is_given_up_while_it_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("cache");
        std::fs::create_dir(&top).unwrap();
        for i in 0..50 {
            std::fs::write(top.join(format!("{i}")), b"").unwrap();
        }
        assert!(walk_within(std::slice::from_ref(&top), 1000, 10)
            .unwrap_err()
            .contains("more than 10"));
        assert_eq!(walk_within(&[top], 1000, 51).unwrap().len(), 51);
    }

    #[test]
    fn a_file_replaced_by_one_of_the_same_size_and_time_is_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, b"version one").unwrap();
        let listed = walk(std::slice::from_ref(&file), 100).unwrap().remove(0);
        let time = std::fs::metadata(&file).unwrap().modified().unwrap();
        let other = dir.path().join("g");
        std::fs::write(&other, b"version two").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&other)
            .unwrap()
            .set_modified(time)
            .unwrap();
        std::fs::rename(&other, &file).unwrap();
        assert!(read_range(&listed, 0, 4).unwrap_err().contains("changed"));
    }

    #[tokio::test]
    async fn an_entry_landing_on_one_already_written_fails_the_fetch_and_overwrites_nothing() {
        // What a volume that ignores case does to `Photo.jpg` then `photo.jpg`: the
        // second lands on the first. Played here by two entries of one name.
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("paste");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("x"), b"first").unwrap();
        let fetch = Fetch {
            link: LinkSender::closed(),
            machine_id: "m".into(),
            link_id: 1,
            serial: 1,
            local: 1,
            files: vec![ClipFile {
                path: "x".into(),
                size: 0,
                folder: false,
            }],
            folder: folder.clone(),
            routes: Routes::default(),
            transfers: Arc::new(AtomicU64::new(0)),
        };
        assert!(super::fetch(fetch).await.unwrap_err().contains("same name"));
        assert_eq!(std::fs::read(folder.join("x")).unwrap(), b"first");
    }

    #[test]
    fn each_paste_reserves_a_folder_of_its_own_and_never_takes_one_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        let reception = dir.path().join("in");
        let a = reserve(&reception, 1).unwrap();
        std::fs::write(a.join("theirs"), b"another agent's").unwrap();
        // The same paste number again in the same second, as an agent restarted (or a
        // second one) would ask for.
        let b = reserve(&reception, 1).unwrap();
        assert_ne!(a, b);
        assert!(b.read_dir().unwrap().next().is_none(), "new and empty");
        assert_eq!(std::fs::read(a.join("theirs")).unwrap(), b"another agent's");
    }

    #[test]
    fn two_entries_that_would_land_on_one_file_refuse_the_offer() {
        let file = |path: &str| ClipFile {
            path: path.into(),
            size: 1,
            folder: false,
        };
        let photos = [file("d/Photo.jpg"), file("d/photo.jpg")];
        assert!(acceptable_on(&photos, 10, false).is_ok());
        assert!(acceptable_on(&photos, 10, true)
            .unwrap_err()
            .contains("same name"));
        // Two copied roots of the same name, on any system.
        assert!(acceptable_on(&[file("x"), file("x")], 10, false).is_err());
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
