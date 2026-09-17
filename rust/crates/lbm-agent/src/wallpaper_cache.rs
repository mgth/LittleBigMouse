//! Best-effort housekeeping of editor images, only after a successful desktop update.
//! Unknown native state is a reason to retain the cache, never to guess references.
use lbm_store::wallpaper_settings::{self, LayoutWallpaperSettings, WallpaperSettings};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};

pub const GRACE: Duration = Duration::from_secs(24 * 60 * 60);
#[derive(Default)]
pub(crate) struct Files {
    pending: BTreeMap<PathBuf, usize>,
}
// Publish/reuse, settings updates, pending applications and deletion share this lock.
pub(crate) static FILES: Mutex<Files> = Mutex::new(Files {
    pending: BTreeMap::new(),
});

pub(crate) struct Pending(Vec<PathBuf>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Ok(mut files) = FILES.lock() {
            for path in &self.0 {
                if let Some(count) = files.pending.get_mut(path) {
                    *count -= 1;
                    if *count == 0 {
                        files.pending.remove(path);
                    }
                }
            }
        }
    }
}
pub(crate) fn pin(screens: &[crate::desktop::ScreenWallpaper]) -> Result<Pending, String> {
    let mut files = FILES.lock().map_err(|e| e.to_string())?;
    let paths: Vec<_> = screens
        .iter()
        .filter_map(|s| s.image.as_ref())
        .map(fs::canonicalize)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    for path in &paths {
        *files.pending.entry(path.clone()).or_default() += 1;
    }
    Ok(Pending(paths))
}
pub(crate) fn save_settings(
    path: &Path,
    id: &str,
    settings: LayoutWallpaperSettings,
) -> Result<(), String> {
    let _guard = FILES.lock().map_err(|e| e.to_string())?;
    wallpaper_settings::update(path, id, settings).map_err(|e| e.to_string())
}

fn owned(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("editor_") else {
        return false;
    };
    let Some((hash, suffix)) = rest.split_once('.') else {
        return false;
    };
    if hash.len() != 16
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return false;
    }
    if suffix == "png" {
        return true;
    }
    let parts: Vec<_> = suffix.split('.').collect();
    parts.len() == 3
        && parts[2] == "tmp"
        && parts[..2]
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}
fn old_file(path: &Path, now: SystemTime) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| {
        m.is_file()
            && !m.file_type().is_symlink()
            && m.modified()
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age > GRACE)
    })
}

struct Plan {
    dir: PathBuf,
    store: PathBuf,
    settings: WallpaperSettings,
    candidates: Vec<PathBuf>,
}
impl Plan {
    fn read(dir: &Path, store: &Path, now: SystemTime) -> Result<Self, String> {
        let meta = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("cache directory is not a regular directory".into());
        }
        let dir = fs::canonicalize(dir).map_err(|e| e.to_string())?;
        let settings = wallpaper_settings::load_checked(store).map_err(|e| e.to_string())?;
        let mut candidates = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name().to_str().is_some_and(owned) && old_file(&entry.path(), now) {
                candidates.push(entry.path());
            }
        }
        Ok(Self {
            dir,
            store: store.to_owned(),
            settings,
            candidates,
        })
    }
    fn finish(
        self,
        before: &BTreeSet<PathBuf>,
        after: &BTreeSet<PathBuf>,
        now: SystemTime,
    ) -> Result<usize, String> {
        if before != after {
            return Err("native wallpaper references changed".into());
        }
        let files = FILES.lock().map_err(|e| e.to_string())?;
        if wallpaper_settings::load_checked(&self.store).map_err(|e| e.to_string())?
            != self.settings
        {
            return Err("wallpaper settings changed".into());
        }
        let meta = fs::symlink_metadata(&self.dir).map_err(|e| e.to_string())?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || fs::canonicalize(&self.dir).map_err(|e| e.to_string())? != self.dir
        {
            return Err("cache directory changed".into());
        }
        let mut keep = before.clone();
        keep.extend(files.pending.keys().cloned());
        for settings in self.settings.values() {
            // Keep even inactive modes, screens and layouts: their sources can be reused.
            keep.extend(
                settings
                    .span_image_path
                    .iter()
                    .chain(
                        settings
                            .per_screen
                            .values()
                            .filter_map(|s| s.image_path.as_ref()),
                    )
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from),
            );
        }
        // Resolve aliases, case and directory references. Unresolvable paths postpone
        // the whole sweep; no partially verified reference set can delete files.
        let keep: Vec<_> = keep
            .iter()
            .map(fs::canonicalize)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("unresolved wallpaper reference: {e}"))?;
        let mut removed = 0;
        for path in self.candidates {
            if !keep.iter().any(|k| path.starts_with(k)) && old_file(&path, now) {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}

/// Called while the runtime's wallpaper application lock is held. Native reads have
/// a deadline; filesystem work runs off the resident event loop. Failures only log.
pub(crate) async fn cleanup(dir: PathBuf, store: PathBuf) {
    let result = collect_with(dir, store, references).await;
    match result {
        Ok(n) if n > 0 => eprintln!("[lbm-agent] wallpaper cache: removed {n} unused files"),
        Err(e) => eprintln!("[lbm-agent] wallpaper cache retained: {e}"),
        _ => {}
    }
}
async fn collect_with<F, R>(dir: PathBuf, store: PathBuf, mut read: F) -> Result<usize, String>
where
    F: FnMut() -> R,
    R: std::future::Future<Output = Result<BTreeSet<PathBuf>, String>>,
{
    let plan = tokio::task::spawn_blocking(move || Plan::read(&dir, &store, SystemTime::now()))
        .await
        .map_err(|e| e.to_string())??;
    if plan.candidates.is_empty() {
        return Ok(0);
    }
    let before = read().await?;
    let after = read().await?;
    tokio::task::spawn_blocking(move || plan.finish(&before, &after, SystemTime::now()))
        .await
        .map_err(|e| e.to_string())?
}
async fn references() -> Result<BTreeSet<PathBuf>, String> {
    tokio::time::timeout(Duration::from_secs(5), crate::desktop::references())
        .await
        .map_err(|_| "desktop reference query timed out".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_store::wallpaper_settings::ScreenWallpaperSettings;

    struct Cache {
        root: tempfile::TempDir,
        store: PathBuf,
        now: SystemTime,
    }
    impl Cache {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let store = root.path().join("wallpaper.json");
            Self {
                root,
                store,
                // Whole seconds keep the boundary fixture exact on filesystems with
                // coarser timestamp precision (notably Windows).
                now: SystemTime::UNIX_EPOCH
                    + Duration::from_secs(
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    ),
            }
        }
        fn old(&self, name: &str) -> PathBuf {
            let path = self.root.path().join(name);
            fs::write(&path, b"cached").unwrap();
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(self.now - GRACE - Duration::from_secs(60))
                .unwrap();
            path
        }
        fn plan(&self) -> Plan {
            Plan::read(self.root.path(), &self.store, self.now).unwrap()
        }
        fn collect(&self, refs: BTreeSet<PathBuf>) -> Result<usize, String> {
            self.plan().finish(&refs, &refs, self.now)
        }
    }
    const A: &str = "editor_0123456789abcdef.png";
    const B: &str = "editor_abcdef0123456789.png";

    #[tokio::test]
    async fn failed_or_changed_native_reads_leave_all_files() {
        let c = Cache::new();
        let a = c.old(A);
        for answers in [
            vec![Err("unavailable".into())],
            vec![Ok(BTreeSet::new()), Err("incomplete".into())],
            vec![Ok(BTreeSet::new()), Ok(BTreeSet::from([a.clone()]))],
        ] {
            let mut answers = answers.into_iter();
            let result = collect_with(c.root.path().to_owned(), c.store.clone(), || {
                std::future::ready(answers.next().unwrap())
            })
            .await;
            assert!(result.is_err());
            assert!(a.exists());
        }
    }
    #[tokio::test]
    async fn stable_native_reads_allow_collection() {
        let c = Cache::new();
        let a = c.old(A);
        let b = c.old(B);
        let result = collect_with(c.root.path().to_owned(), c.store.clone(), || {
            std::future::ready(Ok(BTreeSet::from([a.clone()])))
        })
        .await;
        assert_eq!(result.unwrap(), 1);
        assert!(a.exists() && !b.exists());
    }
    #[tokio::test]
    async fn no_native_calls_when_no_old_files_exist() {
        let c = Cache::new();
        let result = collect_with(c.root.path().to_owned(), c.store.clone(), || {
            panic!("no desktop query expected");
            #[allow(unreachable_code)]
            std::future::ready(Ok(BTreeSet::new()))
        })
        .await;
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn only_exact_editor_names_are_owned() {
        assert!(owned(A));
        assert!(owned("editor_0123456789abcdef.123.0.tmp"));
        for name in [
            "editor_a.png",
            "editor_0123456789ABCDEF.png",
            "editor_0123456789abcdef.png.tmp",
            "editor_0123456789abcdef..0.tmp",
            "editor_0123456789abcdef.1.x.tmp",
            "editor_0123456789abcdef.png.bak",
            "span_0123456789abcdef.png",
            "family.png",
        ] {
            assert!(!owned(name), "{name}");
        }
    }
    #[test]
    fn removes_old_generated_files_and_abandoned_temps_only() {
        let c = Cache::new();
        let a = c.old(A);
        let tmp = c.old("editor_0123456789abcdef.123.0.tmp");
        let unknown = c.old("family.png");
        let legacy = c.old("span_0123456789abcdef.png");
        fs::create_dir(c.root.path().join(B)).unwrap();
        let nested = c.root.path().join(B).join(A);
        fs::write(&nested, b"nested").unwrap();
        assert_eq!(c.collect(BTreeSet::new()).unwrap(), 2);
        assert!(!a.exists() && !tmp.exists());
        assert!(unknown.exists() && legacy.exists() && nested.exists());
    }
    #[test]
    fn recent_boundary_and_future_timestamps_survive() {
        let c = Cache::new();
        let a = c.old(A);
        for modified in [c.now, c.now - GRACE, c.now + GRACE] {
            fs::File::options()
                .write(true)
                .open(&a)
                .unwrap()
                .set_modified(modified)
                .unwrap();
            assert_eq!(c.collect(BTreeSet::new()).unwrap(), 0);
            assert!(a.exists());
        }
    }
    #[test]
    fn keeps_native_files_and_directory_references() {
        let c = Cache::new();
        let a = c.old(A);
        let b = c.old(B);
        assert_eq!(
            c.collect(BTreeSet::from([c.root.path().to_owned()]))
                .unwrap(),
            0
        );
        assert_eq!(c.collect(BTreeSet::from([a.clone()])).unwrap(), 1);
        assert!(a.exists() && !b.exists());
    }
    #[test]
    fn sources_in_other_layouts_and_inactive_modes_are_kept() {
        let c = Cache::new();
        let a = c.old(A);
        let b = c.old(B);
        let settings = WallpaperSettings::from([
            (
                "disconnected".into(),
                LayoutWallpaperSettings {
                    span_image_path: Some(a.to_string_lossy().into()),
                    ..Default::default()
                },
            ),
            (
                "other".into(),
                LayoutWallpaperSettings {
                    per_screen: BTreeMap::from([(
                        "off".into(),
                        ScreenWallpaperSettings {
                            kind: wallpaper_settings::ScreenWallpaperKind::Color,
                            image_path: Some(b.to_string_lossy().into()),
                            ..Default::default()
                        },
                    )]),
                    ..Default::default()
                },
            ),
        ]);
        wallpaper_settings::save(&c.store, &settings).unwrap();
        assert_eq!(c.collect(BTreeSet::new()).unwrap(), 0);
        assert!(a.exists() && b.exists());
    }
    #[test]
    fn corrupt_settings_or_unresolvable_references_prevent_deletion() {
        let c = Cache::new();
        let a = c.old(A);
        fs::write(&c.store, b"broken").unwrap();
        assert!(Plan::read(c.root.path(), &c.store, c.now).is_err());
        fs::remove_file(&c.store).unwrap();
        assert!(c
            .collect(BTreeSet::from([c.root.path().join("missing.png")]))
            .is_err());
        assert!(a.exists());
    }
    #[test]
    fn changed_native_state_or_settings_cancels_the_whole_plan() {
        let c = Cache::new();
        let a = c.old(A);
        assert!(c
            .plan()
            .finish(&BTreeSet::new(), &BTreeSet::from([a.clone()]), c.now)
            .is_err());
        let plan = c.plan();
        save_settings(&c.store, "new-layout", LayoutWallpaperSettings::default()).unwrap();
        assert!(plan
            .finish(&BTreeSet::new(), &BTreeSet::new(), c.now)
            .is_err());
        assert!(a.exists());
    }
    #[test]
    fn reusing_an_old_image_after_planning_restarts_its_grace_period() {
        let c = Cache::new();
        let pixels = image::RgbaImage::new(2, 2);
        let path = crate::wallpaper_job::image_file(c.root.path(), &pixels).unwrap();
        c.old(path.file_name().unwrap().to_str().unwrap());
        let plan = c.plan();
        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(
            crate::wallpaper_job::image_file(c.root.path(), &pixels).unwrap(),
            path
        );
        assert_eq!(
            plan.finish(&BTreeSet::new(), &BTreeSet::new(), SystemTime::now())
                .unwrap(),
            0
        );
        assert!(path.exists());
    }
    #[test]
    fn queued_applications_are_pinned_until_the_last_owner_finishes() {
        let c = Cache::new();
        let a = c.old(A);
        let screens = [crate::desktop::ScreenWallpaper {
            expected_size: None,
            x: 0.,
            y: 0.,
            image: Some(a.clone()),
            style: wallpaper_settings::WallpaperStyle::Stretch,
            color: "#000000".into(),
        }];
        let first = pin(&screens).unwrap();
        let second = pin(&screens).unwrap();
        assert_eq!(c.collect(BTreeSet::new()).unwrap(), 0);
        drop(first);
        assert_eq!(c.collect(BTreeSet::new()).unwrap(), 0);
        drop(second);
        assert_eq!(c.collect(BTreeSet::new()).unwrap(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_deleted_and_reference_aliases_are_resolved() {
        use std::os::unix::fs::symlink;
        let c = Cache::new();
        let a = c.old(A);
        let alias = c.root.path().join(B);
        symlink(&a, &alias).unwrap();
        assert_eq!(c.collect(BTreeSet::from([alias.clone()])).unwrap(), 0);
        assert_eq!(c.collect(BTreeSet::new()).unwrap(), 1);
        assert!(fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink());
        let root_alias = c.root.path().join("alias");
        symlink(c.root.path(), &root_alias).unwrap();
        assert!(Plan::read(&root_alias, &c.store, c.now).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn a_cache_symlink_is_never_reused_as_an_image() {
        let c = Cache::new();
        let pixels = image::RgbaImage::new(2, 2);
        let path = crate::wallpaper_job::image_file(c.root.path(), &pixels).unwrap();
        fs::remove_file(&path).unwrap();
        let outside = c.root.path().join("source.png");
        fs::write(&outside, b"source").unwrap();
        std::os::unix::fs::symlink(&outside, path).unwrap();
        assert!(crate::wallpaper_job::image_file(c.root.path(), &pixels).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"source");
    }
}
