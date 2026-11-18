use super::{Entry, Rect, Style};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{DesktopWallpaper, IDesktopWallpaper},
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
fn string(p: PWSTR) -> windows::core::Result<String> {
    if p.is_null() {
        return Ok(String::new());
    }
    unsafe {
        let value = p.to_string();
        CoTaskMemFree(Some(p.0.cast()));
        Ok(value?)
    }
}
pub(super) fn query() -> Result<Vec<Entry>, String> {
    read().map_err(|e| e.to_string())
}
fn read() -> windows::core::Result<Vec<Entry>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let _apartment = Apartment;
        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
        let style = match desktop.GetPosition()?.0 {
            0 => Style::Center,
            1 => Style::Tile,
            2 => Style::Stretch,
            3 => Style::Fit,
            5 => Style::Span,
            _ => Style::Fill,
        };
        let color = desktop.GetBackgroundColor()?.0;
        let color = [color as u8, (color >> 8) as u8, (color >> 16) as u8, 255];
        let mut entries = Vec::new();
        for index in 0..desktop.GetMonitorDevicePathCount()? {
            let id = string(desktop.GetMonitorDevicePathAt(index)?)?;
            let id: Vec<_> = id.encode_utf16().chain(Some(0)).collect();
            let id = PCWSTR(id.as_ptr());
            let bounds = desktop.GetMonitorRECT(id)?;
            if bounds.right <= bounds.left || bounds.bottom <= bounds.top {
                continue;
            }
            let path = string(desktop.GetWallpaper(id)?)?;
            entries.push(Entry {
                bounds: Rect::new(
                    bounds.left.into(),
                    bounds.top.into(),
                    (bounds.right - bounds.left).into(),
                    (bounds.bottom - bounds.top).into(),
                ),
                path: (!path.is_empty()).then(|| path.into()),
                style,
                color,
            });
        }
        Ok(entries)
    }
}
