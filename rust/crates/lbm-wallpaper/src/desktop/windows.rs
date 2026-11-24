use super::{Entry, Repeat};
use crate::{
    slots::{self, SlotRead, SlotRect},
    Style,
};
use lbm_layout::geo::Rect;
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::E_FAIL,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
            COINIT_APARTMENTTHREADED,
        },
        UI::{
            Shell::{DesktopWallpaper, IDesktopWallpaper},
            WindowsAndMessaging::{GetSystemMetrics, SM_CMONITORS},
        },
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
        // The shell paints slot k on another monitor than the one its path names
        // (crate::slots): read every slot in place, failures kept as holes, then give each
        // monitor the image of the slot painted on it.
        let mut reads = Vec::new();
        for index in 0..desktop.GetMonitorDevicePathCount()? {
            // The list holds every monitor Windows remembers, attached or not, and the
            // calls below fail (E_FAIL) for one that is not on the desktop right now.
            // That monitor has no wallpaper to report; the others still do. Seen on a
            // machine behind a KVM: one stale path made the whole reading fail. The
            // monitor count tells such a stale entry from an active monitor we failed to
            // read (crate::slots::plan).
            let Ok(id) = desktop.GetMonitorDevicePathAt(index).and_then(string) else {
                reads.push(SlotRead::Unknown);
                continue;
            };
            let wide: Vec<_> = id.encode_utf16().chain(Some(0)).collect();
            let pid = PCWSTR(wide.as_ptr());
            let Ok(bounds) = desktop.GetMonitorRECT(pid) else {
                reads.push(SlotRead::Unknown);
                continue;
            };
            // No picture readable for this monitor (a slideshow between two images, for
            // one) is the background colour, not a failure of the whole desktop.
            let path = desktop
                .GetWallpaper(pid)
                .and_then(string)
                .unwrap_or_default();
            let rect = SlotRect {
                left: bounds.left,
                top: bounds.top,
                right: bounds.right,
                bottom: bounds.bottom,
            };
            reads.push(SlotRead::from_parts(Some(id), Some(rect), Some(path)));
        }
        let active = usize::try_from(GetSystemMetrics(SM_CMONITORS)).unwrap_or(0);
        let Some(plan) = slots::plan(&reads, active) else {
            return Err(windows::core::Error::new(
                E_FAIL,
                "The desktop's monitor list does not match its monitor count.",
            ));
        };
        let entries = plan
            .pairs
            .into_iter()
            .map(|(filed, painted)| {
                let r = painted.rect;
                let path = filed.image.unwrap_or_default();
                Entry {
                    bounds: Rect::new(
                        r.left.into(),
                        r.top.into(),
                        (r.right - r.left).into(),
                        (r.bottom - r.top).into(),
                    ),
                    path: (!path.is_empty()).then(|| path.into()),
                    style,
                    repeat: Repeat::Both,
                    color,
                }
            })
            .collect();
        Ok(entries)
    }
}
