//! Windows has one global positioning mode. Exact-size images preserve independent styles.
use crate::{desktop::ScreenWallpaper, wallpaper_job::image_file};
use std::path::Path;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub wallpaper: String,
}
pub trait Shell {
    fn outputs(&self) -> Result<Vec<Output>, String>;
    fn position(&self) -> Result<i32, String>;
    fn set_position(&self, position: i32) -> Result<(), String>;
    fn set_wallpaper(&self, id: &str, path: &str) -> Result<(), String>;
}
pub fn apply_with(
    shell: &impl Shell,
    screens: &[ScreenWallpaper],
    dir: &Path,
) -> Result<(), String> {
    if screens.is_empty() {
        return Ok(());
    }
    let outputs = shell.outputs()?;
    let position = shell.position()?;
    let mut planned = Vec::new();
    for screen in screens {
        let matches: Vec<_> = outputs
            .iter()
            .filter(|o| f64::from(o.x) == screen.x && f64::from(o.y) == screen.y)
            .collect();
        if matches.len() != 1 {
            return Err("Wallpaper output is missing or ambiguous; reload the displays.".into());
        }
        let output = matches[0];
        if planned
            .iter()
            .any(|(o, _): &(&Output, String)| o.id == output.id)
            || screen
                .expected_size
                .is_some_and(|size| size != (output.width, output.height))
        {
            return Err("Wallpaper output geometry changed; reload the displays.".into());
        }
        let fill = lbm_wallpaper::render::color(if screen.color.is_empty() {
            "#000000"
        } else {
            &screen.color
        })?;
        let image = if let Some(path) = &screen.image {
            lbm_wallpaper::render::decode(path)?
        } else {
            image::RgbaImage::from_pixel(1, 1, image::Rgba(fill))
        };
        let style = if screen.image.is_none() {
            lbm_store::wallpaper_settings::WallpaperStyle::Stretch
        } else {
            screen.style
        };
        let pixels = lbm_wallpaper::render::one(image, style, output.width, output.height, fill)?;
        planned.push((
            output,
            image_file(dir, &pixels)?.to_string_lossy().into_owned(),
        ));
    }
    // Tile and Span affect the whole desktop. A partial edit cannot silently alter others.
    let global = matches!(position, 1 | 5);
    if global && planned.len() != outputs.len() {
        return Err("Windows currently tiles or spans across the desktop. Configure every screen before applying independent wallpapers.".into());
    }
    if shell.outputs()? != outputs || shell.position()? != position {
        return Err("The desktop changed while preparing wallpapers. Try again.".into());
    }
    let result: Result<(), String> = (|| {
        if global {
            shell.set_position(2)?;
        }
        for (output, path) in &planned {
            shell.set_wallpaper(&output.id, path)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut restored = true;
        for (output, _) in &planned {
            restored &= shell.set_wallpaper(&output.id, &output.wallpaper).is_ok();
        }
        if global {
            restored &= shell.set_position(position).is_ok();
        }
        return Err(format!(
            "{error} {}",
            if restored {
                "Previous Windows wallpapers were restored."
            } else {
                "Some screens could not be restored; check the desktop."
            }
        ));
    }
    Ok(())
}
#[cfg(windows)]
mod native {
    use super::*;
    use windows::{
        core::PCWSTR,
        Win32::{
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
                COINIT_APARTMENTTHREADED,
            },
            UI::Shell::{DesktopWallpaper, IDesktopWallpaper, DESKTOP_WALLPAPER_POSITION},
        },
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    struct Native(IDesktopWallpaper);
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn string(p: windows::core::PWSTR) -> Result<String, String> {
        if p.is_null() {
            return Ok(String::new());
        }
        unsafe {
            let result = p.to_string().map_err(|e| e.to_string());
            CoTaskMemFree(Some(p.0.cast()));
            result
        }
    }
    impl Shell for Native {
        fn outputs(&self) -> Result<Vec<Output>, String> {
            unsafe {
                let mut outputs = Vec::new();
                for i in 0..self
                    .0
                    .GetMonitorDevicePathCount()
                    .map_err(|e| e.to_string())?
                {
                    let id = string(
                        self.0
                            .GetMonitorDevicePathAt(i)
                            .map_err(|e| e.to_string())?,
                    )?;
                    let w = wide(&id);
                    let r = self
                        .0
                        .GetMonitorRECT(PCWSTR(w.as_ptr()))
                        .map_err(|e| e.to_string())?;
                    if r.right <= r.left || r.bottom <= r.top {
                        continue;
                    }
                    let wallpaper = string(
                        self.0
                            .GetWallpaper(PCWSTR(w.as_ptr()))
                            .map_err(|e| e.to_string())?,
                    )?;
                    outputs.push(Output {
                        id,
                        x: r.left,
                        y: r.top,
                        width: (r.right - r.left) as u32,
                        height: (r.bottom - r.top) as u32,
                        wallpaper,
                    });
                }
                Ok(outputs)
            }
        }
        fn position(&self) -> Result<i32, String> {
            unsafe { self.0.GetPosition().map(|p| p.0).map_err(|e| e.to_string()) }
        }
        fn set_position(&self, p: i32) -> Result<(), String> {
            unsafe {
                self.0
                    .SetPosition(DESKTOP_WALLPAPER_POSITION(p))
                    .map_err(|e| e.to_string())
            }
        }
        fn set_wallpaper(&self, id: &str, path: &str) -> Result<(), String> {
            if id.contains('\0') || path.contains('\0') {
                return Err("A wallpaper path contains a NUL character.".into());
            }
            let id = wide(id);
            let path = wide(path);
            unsafe {
                self.0
                    .SetWallpaper(PCWSTR(id.as_ptr()), PCWSTR(path.as_ptr()))
                    .map_err(|e| e.to_string())
            }
        }
    }
    fn connect() -> Result<(Apartment, Native), String> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|e| e.to_string())?;
            let guard = Apartment;
            let shell =
                CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            Ok((guard, Native(shell)))
        }
    }
    pub fn supported() -> bool {
        connect().is_ok()
    }
    pub fn apply(screens: &[ScreenWallpaper]) -> Result<(), String> {
        let (_guard, shell) = connect()?;
        super::apply_with(&shell, screens, &crate::wallpaper::output_dir())
    }
}
#[cfg(windows)]
pub use native::{apply, supported};
