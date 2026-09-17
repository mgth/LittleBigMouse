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
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    pixels.dimensions().hash(&mut hash);
    pixels.as_raw().hash(&mut hash);
    let path = dir.join(format!("editor_{:016x}.png", hash.finish()));
    if !path.exists() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let tmp = path.with_extension(format!(
            "{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        pixels
            .save_with_format(&tmp, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
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
        wallpaper_settings::update(&self.store, &self.layout.id, self.settings)
            .map_err(|e| e.to_string())?;
        Ok(screens)
    }
}
