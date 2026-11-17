//! Physical-screen overlays. Geometry/raster/lifetime policy also run without a desktop.
#[cfg(target_os = "linux")]
pub mod linux;
pub mod raster;
pub mod scene;
#[cfg(windows)]
mod windows;
pub use scene::{Band, Mode};

pub trait Backend {
    fn present(&mut self, bands: &[Band]) -> Result<(), String>;
    fn clear(&mut self);
    fn invalidated(&mut self) -> bool {
        false
    }
    fn poll(&mut self) -> Result<(), String> {
        if self.invalidated() {
            Err("Displays changed. Reload the screens before showing overlays again.".into())
        } else {
            Ok(())
        }
    }
}

/// Owns all surfaces. Unchanged frames perform no drawing or native allocation.
#[derive(Default)]
pub struct Manager<B: Backend> {
    backend: B,
    shown: Vec<Band>,
}
impl<B: Backend> Manager<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            shown: Vec::new(),
        }
    }
    pub fn sync(&mut self, bands: &[Band]) -> Result<(), String> {
        if let Err(error) = self.backend.poll() {
            self.clear();
            return Err(error);
        }
        if bands == self.shown {
            return Ok(());
        }
        if bands.is_empty() {
            self.clear();
            return Ok(());
        }
        if let Err(error) = self.backend.present(bands) {
            self.clear();
            return Err(error);
        }
        self.shown = bands.to_vec();
        Ok(())
    }
    pub fn clear(&mut self) {
        self.backend.clear();
        self.shown.clear();
    }
}
impl<B: Backend> Drop for Manager<B> {
    fn drop(&mut self) {
        self.backend.clear();
    }
}
pub const SUPPORTED: bool = cfg!(any(windows, target_os = "linux"));
#[cfg(windows)]
pub type Overlay = Manager<windows::Native>;
#[cfg(target_os = "linux")]
pub type Overlay = Manager<linux::Native>;
#[cfg(not(any(windows, target_os = "linux")))]
pub type Overlay = Manager<Unsupported>;
#[derive(Default)]
pub struct Unsupported;
impl Backend for Unsupported {
    fn present(&mut self, _: &[Band]) -> Result<(), String> {
        Err("Screen overlays are currently available on Windows and Linux only.".into())
    }
    fn clear(&mut self) {}
}
