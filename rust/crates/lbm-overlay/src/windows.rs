//! Native layered windows, owned and updated on egui's UI thread.
use crate::{
    input::{Capture, Input, Kind},
    raster,
    scene::{self, Band, Binding, Pixels},
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
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
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
thread_local! {
    static BANDS: std::cell::RefCell<std::collections::HashMap<isize,Band>> = Default::default();
    static INPUT: std::cell::RefCell<Vec<Input>> = Default::default();
    static CAPTURE: std::cell::Cell<Capture> = Default::default();
}
fn capture<T>(change: impl FnOnce(&mut Capture) -> T) -> T {
    CAPTURE.with(|cell| {
        let mut capture = cell.get();
        let result = change(&mut capture);
        cell.set(capture);
        result
    })
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let band = BANDS.with(|b| b.borrow().get(&(hwnd.0 as isize)).cloned());
    let interactive = band.as_ref().is_some_and(Band::interactive);
    match message {
        WM_NCHITTEST => LRESULT(if interactive {
            HTCLIENT as isize
        } else {
            HTTRANSPARENT as isize
        }),
        WM_MOUSEACTIVATE => LRESULT(if interactive {
            MA_ACTIVATE
        } else {
            MA_NOACTIVATE
        } as isize),
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            INVALIDATED.store(true, Ordering::Release);
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEMOVE | WM_MOUSEWHEEL | WM_KEYDOWN
        | WM_CAPTURECHANGED
            if interactive =>
        {
            let mut point = POINT {
                x: (l.0 as u16 as i16).into(),
                y: ((l.0 >> 16) as u16 as i16).into(),
            };
            let kind = match message {
                WM_LBUTTONDOWN => {
                    SetCapture(hwnd);
                    capture(Capture::press);
                    Some(Kind::Press)
                }
                WM_LBUTTONUP => {
                    capture(Capture::release);
                    Some(Kind::Release)
                }
                WM_MOUSEMOVE if GetCapture() == hwnd => Some(Kind::Move),
                WM_MOUSEWHEEL => {
                    let _ = ScreenToClient(hwnd, &mut point);
                    Some(Kind::Wheel(f64::from((w.0 >> 16) as u16 as i16) / 120.))
                }
                WM_KEYDOWN if w.0 == usize::from(VK_ESCAPE.0) => Some(Kind::Escape),
                WM_KEYDOWN if w.0 == usize::from(VK_DELETE.0) => Some(Kind::Delete),
                WM_CAPTURECHANGED => capture(Capture::lost),
                _ => None,
            };
            if let (Some(kind), Some(band)) = (kind, band) {
                INPUT.with(|q| {
                    q.borrow_mut().push(Input::new(
                        &band,
                        kind,
                        (point.x.into(), point.y.into()),
                        band.scale,
                        GetMessageTime() as u32,
                        GetKeyState(i32::from(VK_CONTROL.0)) < 0,
                        GetKeyState(i32::from(VK_SHIFT.0)) < 0,
                    ))
                });
            }
            if message == WM_LBUTTONUP {
                let _ = ReleaseCapture();
            }
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
    binding: Binding,
    shown: Option<Band>,
}
impl Window {
    fn new(band: &Band, owner: HWND) -> Result<Self, String> {
        register()?;
        // SAFETY: the registered callback and static strings remain valid; no borrowed userdata.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TOOLWINDOW
                    | if band.interactive() {
                        WINDOW_EX_STYLE::default()
                    } else {
                        WS_EX_TRANSPARENT | WS_EX_NOACTIVATE
                    },
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
        Ok(Self {
            hwnd,
            binding: band.binding(),
            shown: None,
        })
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
        BANDS.with(|b| b.borrow_mut().insert(self.hwnd.0 as isize, band.clone()));
        self.shown = Some(band.clone());
        Ok(())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            BANDS.with(|b| b.borrow_mut().remove(&(self.hwnd.0 as isize)));
        }
    }
}

#[derive(Default)]
pub struct Native {
    windows: Vec<Window>,
    owner: HWND,
    _ui_thread: PhantomData<Rc<()>>,
    /// The owner this backend made itself ([`crate::Manager::standalone`]), destroyed
    /// after the windows it owns: last field, dropped last.
    _hidden_owner: Option<HiddenOwner>,
}

/// A window never shown, for a process without one to own the overlays: owned popups
/// stay off the taskbar and out of Alt+Tab, and go with their owner.
struct HiddenOwner(HWND);
impl HiddenOwner {
    fn new() -> Result<Self, String> {
        static OWNER_CLASS: OnceLock<Result<(), String>> = OnceLock::new();
        const NAME: windows::core::PCWSTR = w!("LittleBigMouse.ScreenOverlay.Owner");
        unsafe extern "system" fn owner_procedure(
            hwnd: HWND,
            message: u32,
            w: WPARAM,
            l: LPARAM,
        ) -> LRESULT {
            DefWindowProcW(hwnd, message, w, l)
        }
        OWNER_CLASS
            .get_or_init(|| {
                // SAFETY: static class name and callback outlive every window of the class.
                unsafe {
                    let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
                    let class = WNDCLASSW {
                        hInstance: instance.into(),
                        lpszClassName: NAME,
                        lpfnWndProc: Some(owner_procedure),
                        ..Default::default()
                    };
                    if RegisterClassW(&class) == 0 {
                        Err(fail("Cannot register the overlays' owner"))
                    } else {
                        Ok(())
                    }
                }
            })
            .clone()?;
        // SAFETY: as for the overlay windows; this one is never shown.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                NAME,
                w!("LittleBigMouse screen overlays"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                GetModuleHandleW(None).map_err(|e| e.to_string())?,
                None,
            )
            .map_err(|e| e.to_string())?
        };
        Ok(Self(hwnd))
    }
}
impl Drop for HiddenOwner {
    fn drop(&mut self) {
        // SAFETY: created by this thread, destroyed once.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
impl Backend for Native {
    fn take_input(&mut self) -> Vec<Input> {
        INPUT.with(|q| std::mem::take(&mut *q.borrow_mut()))
    }
    fn present(&mut self, bands: &[Band]) -> Result<(), String> {
        if self.owner.is_invalid() {
            return Err("No frontend window to own the screen overlays.".into());
        }
        let _dpi = Dpi::enter()?;
        // Reconcile by screen and role, never by index: a guide that stops crossing one
        // screen must not hand its window to the next screen's guide (WM_DPICHANGED).
        let previous: Vec<_> = self.windows.iter().map(|w| w.binding.clone()).collect();
        let plan = scene::reuse(&previous, bands);
        let mut old: Vec<_> = std::mem::take(&mut self.windows)
            .into_iter()
            .map(Some)
            .collect();
        for (band, index) in bands.iter().zip(plan) {
            let window = match index.and_then(|i| old[i].take()) {
                Some(window) => window,
                None => Window::new(band, self.owner)?,
            };
            self.windows.push(window);
            if let Some(window) = self.windows.last_mut() {
                window.present(band)?;
            }
        }
        // Windows no band kept are destroyed here.
        drop(old);
        if self.invalidated() {
            return Err("Displays changed while showing overlays. Reload the screens.".into());
        }
        Ok(())
    }
    fn clear(&mut self) {
        self.windows.clear();
        INPUT.with(|q| q.borrow_mut().clear());
        INVALIDATED.store(false, Ordering::Release);
    }
    fn invalidated(&mut self) -> bool {
        INVALIDATED.swap(false, Ordering::AcqRel)
    }
}

impl crate::Manager<Native> {
    /// Overlays for a process with no window of its own, owned by a hidden window made
    /// for them on the calling thread, which must pump its messages.
    pub fn standalone() -> Result<Self, String> {
        let owner = HiddenOwner::new()?;
        Ok(Self::new(Native {
            owner: owner.0,
            _hidden_owner: Some(owner),
            ..Default::default()
        }))
    }

    /// The live HWND supplied by eframe. Owned popups follow its minimize/close lifetime.
    pub fn set_owner(&mut self, owner: isize) {
        let owner = HWND(owner as *mut std::ffi::c_void);
        if self.backend.owner != owner {
            self.clear();
            self.backend.owner = owner;
        }
    }
}
