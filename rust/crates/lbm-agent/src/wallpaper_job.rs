//! A captured layout and settings, prepared off the resident event loop.
use crate::desktop::ScreenWallpaper;
use lbm_layout::model::Layout;
use lbm_store::wallpaper_settings::{self, LayoutWallpaperSettings};
use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
pub struct Job {
    pub layout: Layout,
    pub settings: LayoutWallpaperSettings,
    pub store: PathBuf,
    pub images: PathBuf,
}
pub fn image_file(dir: &Path, pixels: &image::RgbaImage) -> Result<PathBuf, String> {
    let _guard = crate::wallpaper_cache::FILES
        .lock()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    pixels.dimensions().hash(&mut hash);
    pixels.as_raw().hash(&mut hash);
    let path = dir.join(format!("editor_{:016x}.png", hash.finish()));
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
            // A reused image can be queued for application. Restart its grace period
            // under the same lock used by the collector's final age check.
            std::fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|f| f.set_modified(std::time::SystemTime::now()))
                .map_err(|e| e.to_string())?;
        }
        Ok(_) => return Err("The wallpaper cache entry is not a regular file.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let tmp = path.with_extension(format!(
                "{}.{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let result = pixels
                .save_with_format(&tmp, image::ImageFormat::Png)
                .map_err(|e| e.to_string())
                .and_then(|()| std::fs::rename(&tmp, &path).map_err(|e| e.to_string()));
            if result.is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            result?;
        }
        Err(e) => return Err(e.to_string()),
    }
    Ok(path)
}
pub fn screens(
    layout: &Layout,
    settings: &LayoutWallpaperSettings,
    images: &Path,
) -> Result<Vec<ScreenWallpaper>, String> {
    let painted = lbm_wallpaper::render::paint(layout, settings, false)?;
    let mut screens = Vec::new();
    for p in painted {
        let path = image_file(images, &p.pixels)?;
        screens.push(ScreenWallpaper {
            x: p.x,
            y: p.y,
            expected_size: Some(p.pixels.dimensions()),
            image: Some(path),
            style: wallpaper_settings::WallpaperStyle::Stretch,
            color: "#000000".into(),
        });
    }
    Ok(screens)
}
impl Job {
    pub fn prepare(
        self,
        generation: Arc<AtomicU64>,
        expected: u64,
    ) -> Result<Vec<ScreenWallpaper>, String> {
        if generation.load(Ordering::SeqCst) != expected {
            return Err("A newer wallpaper request replaced this one.".into());
        }
        let screens = screens(&self.layout, &self.settings, &self.images)?;
        if generation.load(Ordering::SeqCst) != expected {
            return Err("The displays or wallpaper request changed. Reload and try again.".into());
        }
        crate::wallpaper_cache::save_settings(&self.store, &self.layout.id, self.settings)?;
        Ok(screens)
    }
}
