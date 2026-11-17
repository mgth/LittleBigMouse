//! Native layered windows, owned and updated on egui's UI thread.
use crate::{
    raster,
    scene::{Band, Pixels},
    Backend,
};
use std::{
    marker::PhantomData,
    mem::size_of,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
};
use windows::{
    core::{w, Error},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};

static INVALIDATED: AtomicBool = AtomicBool::new(false);
const CLASS: windows::core::PCWSTR = w!("LittleBigMouse.ScreenOverlay.v7");
fn fail(context: &str) -> String {
    format!("{context}: {}", Error::from_win32())
}
fn rect(p: Pixels) -> RECT {
    RECT {
        left: p.x,
        top: p.y,
        right: p.x + p.width,
        bottom: p.y + p.height,
    }
}

struct Dpi(DPI_AWARENESS_CONTEXT);
impl Dpi {
    fn enter() -> Result<Self, String> {
        // SAFETY: changes only this thread; Drop restores the previous context.
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            Err(fail("Cannot enter per-monitor DPI awareness"))
        } else {
            Ok(Self(previous))
        }
    }
}
impl Drop for Dpi {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match message {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            // Never keep a ruler visible in coordinates Windows just invalidated.
            INVALIDATED.store(true, Ordering::Release);
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, w, l),
    }
}
fn register() -> Result<(), String> {
    static CLASS_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    CLASS_RESULT
        .get_or_init(|| {
            // SAFETY: static class name and callback outlive all windows; no background brush.
            unsafe {
                let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
                let class = WNDCLASSW {
                    hInstance: instance.into(),
                    lpszClassName: CLASS,
                    lpfnWndProc: Some(procedure),
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 {
                    Err(fail("Cannot register overlay windows"))
                } else {
                    Ok(())
                }
            }
        })
        .clone()
}

struct Bitmap {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
}
impl Bitmap {
    fn new(image: &raster::Image) -> Result<Self, String> {
        // SAFETY: dimensions and buffer length come from the bounded raster renderer.
        // A top-down 32-bit DIB has exactly width*height*4 bytes and no row padding.
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err(fail("Cannot allocate overlay DC"));
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: image.width as i32,
                    biHeight: -(image.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bitmap) => bitmap,
                Err(e) => {
                    let _ = DeleteDC(dc);
                    return Err(e.to_string());
                }
            };
            let old = SelectObject(dc, bitmap);
            if bits.is_null() || old.is_invalid() {
                let _ = DeleteObject(bitmap);
                let _ = DeleteDC(dc);
                return Err("Cannot select overlay bitmap.".into());
            }
            std::ptr::copy_nonoverlapping(image.bgra.as_ptr(), bits.cast::<u8>(), image.bgra.len());
            Ok(Self { dc, bitmap, old })
        }
    }
}
impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: all handles are owned here, and the old object is restored first.
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.dc);
        }
    }
}
struct Window {
    hwnd: HWND,
    shown: Option<Band>,
}
impl Window {
    fn new(band: &Band, owner: HWND) -> Result<Self, String> {
        register()?;
        // SAFETY: the registered callback and static strings remain valid; no borrowed userdata.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                CLASS,
                w!("LittleBigMouse screen overlay"),
                WS_POPUP,
                band.rect.x,
                band.rect.y,
                band.rect.width,
                band.rect.height,
                owner,
                None,
                GetModuleHandleW(None).map_err(|e| e.to_string())?,
                None,
            )
            .map_err(|e| e.to_string())?
        };
        Ok(Self { hwnd, shown: None })
    }
    fn present(&mut self, band: &Band) -> Result<(), String> {
        if self.shown.as_ref() == Some(band) {
            return Ok(());
        }
        let image = raster::render(band)?;
        let bitmap = Bitmap::new(&image)?;
        // SAFETY: all handles/buffers remain valid for these synchronous Win32 calls.
        unsafe {
            let screen = rect(band.screen);
            let monitor = MonitorFromRect(&screen, MONITOR_DEFAULTTONULL);
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if monitor.is_invalid()
                || !GetMonitorInfoW(monitor, &mut info).as_bool()
                || info.rcMonitor != screen
            {
                return Err(
                    "Screen bounds changed. Reload the displays before showing overlays.".into(),
                );
            }
            let pos = POINT {
                x: band.rect.x,
                y: band.rect.y,
            };
            let size = SIZE {
                cx: band.rect.width,
                cy: band.rect.height,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&pos),
                Some(&size),
                bitmap.dc,
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
            .map_err(|e| e.to_string())?;
            if (GetDpiForWindow(self.hwnd) as f64 - band.scale * 96.).abs() > 1. {
                return Err(
                    "Screen DPI changed. Reload the displays before showing overlays.".into(),
                );
            }
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                pos.x,
                pos.y,
                size.cx,
                size.cy,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .map_err(|e| e.to_string())?;
            let mut actual = RECT::default();
            GetWindowRect(self.hwnd, &mut actual).map_err(|e| e.to_string())?;
            if actual != rect(band.rect) {
                return Err("Windows did not grant the requested overlay pixel bounds.".into());
            }
        }
        self.shown = Some(band.clone());
        Ok(())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[derive(Default)]
pub struct Native {
    windows: Vec<Window>,
    owner: HWND,
    _ui_thread: PhantomData<Rc<()>>,
}
impl Backend for Native {
    fn present(&mut self, bands: &[Band]) -> Result<(), String> {
        if self.owner.is_invalid() {
            return Err("No frontend window to own the screen overlays.".into());
        }
        let _dpi = Dpi::enter()?;
        self.windows.truncate(bands.len());
        for (i, band) in bands.iter().enumerate() {
            if i == self.windows.len() {
                self.windows.push(Window::new(band, self.owner)?);
            }
            self.windows[i].present(band)?;
        }
        if self.invalidated() {
            return Err("Displays changed while showing overlays. Reload the screens.".into());
        }
        Ok(())
    }
    fn clear(&mut self) {
        self.windows.clear();
        INVALIDATED.store(false, Ordering::Release);
    }
    fn invalidated(&mut self) -> bool {
        INVALIDATED.swap(false, Ordering::AcqRel)
    }
}

impl crate::Manager<Native> {
    /// The live HWND supplied by eframe. Owned popups follow its minimize/close lifetime.
    pub fn set_owner(&mut self, owner: isize) {
        let owner = HWND(owner as *mut std::ffi::c_void);
        if self.backend.owner != owner {
            self.clear();
            self.backend.owner = owner;
        }
    }
}
