//! Native file selection runs on a dedicated worker, never on egui's thread.
use std::path::{Path, PathBuf};

/// Own the native window, not just its pointers, while a portal uses its surface.
#[cfg(target_os = "linux")]
pub trait WindowOwner:
    raw_window_handle::HasWindowHandle
    + raw_window_handle::HasDisplayHandle
    + Send
    + Sync
    + std::fmt::Debug
{
}
#[cfg(target_os = "linux")]
impl<T> WindowOwner for T where
    T: raw_window_handle::HasWindowHandle
        + raw_window_handle::HasDisplayHandle
        + Send
        + Sync
        + std::fmt::Debug
{
}

#[derive(Clone, Debug, Default)]
pub enum Parent {
    #[default]
    None,
    X11(usize),
    Windows(isize),
    #[cfg(target_os = "linux")]
    Window(std::sync::Arc<dyn WindowOwner>),
}
pub type Selection = Result<Option<String>, String>;

/// Validate through the renderer, including format and allocation limits. No writes.
pub fn validate(path: PathBuf) -> Result<String, String> {
    if !path.is_absolute() {
        return Err("Choose an absolute local image path.".into());
    }
    let text = path
        .to_str()
        .ok_or("This image path cannot be represented in wallpaper settings.")?
        .to_owned();
    lbm_wallpaper::render::decode(&path)?;
    Ok(text)
}
pub fn choose(parent: Parent, current: Option<&str>) -> Selection {
    platform::choose(parent, current)?.map(validate).transpose()
}
fn folder(current: Option<&str>) -> Option<&Path> {
    let path = Path::new(current?);
    let dir = if path.is_dir() { path } else { path.parent()? };
    (dir.is_absolute() && dir.is_dir()).then_some(dir)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use ashpd::desktop::{
        file_chooser::{FileFilter, SelectedFiles},
        ResponseError,
    };
    use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
    use wayland_client::{protocol::wl_surface::WlSurface, Connection, Proxy};

    /// Field order matters: unexport before dropping the borrowed connection, and
    /// destroy that connection before releasing the actual window/display owner.
    struct PortalParent {
        identifier: Option<ashpd::WindowIdentifier>,
        connection: Option<Connection>,
        _owner: Parent,
    }
    impl Drop for PortalParent {
        fn drop(&mut self) {
            drop(self.identifier.take());
            // Also runs after errors/unwinding, even if egui's loop has stopped.
            if let Some(connection) = &self.connection {
                let _ = connection.flush();
            }
        }
    }
    impl PortalParent {
        fn request_identifier(&self) -> Option<ashpd::WindowIdentifier> {
            let id = self.identifier.as_ref()?.to_string();
            if id == "wayland:" {
                return None;
            }
            // Only the opaque token crosses into the request builder; the original
            // export stays in this guard until the entire operation is over.
            id.parse::<ashpd::WindowIdentifierType>()
                .ok()
                .map(Into::into)
        }
        async fn prepare(owner: Parent) -> Self {
            let mut prepared = Self {
                identifier: None,
                connection: None,
                _owner: owner,
            };
            match &prepared._owner {
                Parent::X11(id) => {
                    prepared.identifier = Some(ashpd::WindowIdentifier::from_xid(*id as _))
                }
                Parent::Window(window) => {
                    let Ok(handle) = window.window_handle() else {
                        return prepared;
                    };
                    match handle.as_raw() {
                        RawWindowHandle::Xlib(h) => {
                            prepared.identifier = Some(ashpd::WindowIdentifier::from_xid(h.window))
                        }
                        RawWindowHandle::Xcb(h) => {
                            prepared.identifier =
                                Some(ashpd::WindowIdentifier::from_xid(h.window.get().into()))
                        }
                        RawWindowHandle::Wayland(surface) => {
                            let Ok(display) = window.display_handle() else {
                                return prepared;
                            };
                            let RawDisplayHandle::Wayland(display) = display.as_raw() else {
                                return prepared;
                            };
                            // SAFETY: the Arc in _owner keeps both native handles alive.
                            // This backend borrows the display; it never disconnects it.
                            // Keep it too: protocol proxies alone hold only a weak backend.
                            let backend = unsafe {
                                wayland_backend::sys::client::Backend::from_foreign_display(
                                    display.display.as_ptr().cast(),
                                )
                            };
                            let connection = Connection::from_backend(backend);
                            let id = unsafe {
                                wayland_backend::sys::client::ObjectId::from_ptr(
                                    WlSurface::interface(),
                                    surface.surface.as_ptr().cast(),
                                )
                            };
                            let surface = id
                                .ok()
                                .and_then(|id| WlSurface::from_id(&connection, id).ok());
                            if let Some(surface) = surface {
                                // Do not time out this future: ashpd's export thread must
                                // finish before the native window/connection can be dropped.
                                prepared.identifier =
                                    ashpd::WindowIdentifier::from_wayland(&surface).await;
                            }
                            prepared.connection = Some(connection);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            prepared
        }
    }
    async fn with_parent<F, R, T>(parent: Parent, run: F) -> T
    where
        F: FnOnce(Option<ashpd::WindowIdentifier>) -> R,
        R: std::future::Future<Output = T>,
    {
        let parent = PortalParent::prepare(parent).await;
        run(parent.request_identifier()).await
    }
    pub fn local_uri(value: &str) -> Result<PathBuf, String> {
        let uri = url::Url::parse(value).map_err(|e| e.to_string())?;
        if uri.query().is_some() || uri.fragment().is_some() {
            return Err("The file chooser returned an invalid local file URI.".into());
        }
        uri.to_file_path()
            .map_err(|_| "Choose a local image file, not a remote address.".into())
    }
    pub fn choose(parent: Parent, current: Option<&str>) -> Result<Option<PathBuf>, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        runtime.block_on(with_parent(parent, |parent| async move {
            let mut request=SelectedFiles::open_file().title("Choose wallpaper image")
                .identifier(parent).modal(false).multiple(false).directory(false)
                .filter(FileFilter::new("Images (PNG, JPEG, BMP, GIF)")
                    .mimetype("image/png").mimetype("image/jpeg").mimetype("image/bmp").mimetype("image/gif"));
            if let Some(folder)=folder(current){request=request.current_folder(folder).map_err(|e|e.to_string())?;}
            let response=match request.send().await.and_then(|r|r.response()) {
                Ok(response)=>response,
                Err(ashpd::Error::Response(ResponseError::Cancelled))=>return Ok(None),
                Err(e)=>return Err(format!("The desktop file chooser is unavailable: {e}. You can paste a path or drop an image.")),
            };
            let [uri]=response.uris() else{return Err("The file chooser did not return exactly one image.".into());};
            local_uri(uri.as_str()).map(Some)
        }))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use raw_window_handle::{
            DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
            XlibWindowHandle,
        };
        use std::{
            future::Future,
            sync::{
                atomic::{AtomicUsize, Ordering},
                Arc,
            },
            task::{Context, Poll, Waker},
        };

        #[derive(Debug)]
        struct TestWindow {
            xid: Option<u64>,
            dropped: Arc<AtomicUsize>,
        }
        impl HasWindowHandle for TestWindow {
            fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
                let xid = self.xid.ok_or(HandleError::Unavailable)?;
                // A numeric XID is serialized only: these tests never contact X11.
                Ok(unsafe {
                    WindowHandle::borrow_raw(RawWindowHandle::Xlib(XlibWindowHandle::new(xid as _)))
                })
            }
        }
        impl HasDisplayHandle for TestWindow {
            fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
                Err(HandleError::Unavailable)
            }
        }
        impl Drop for TestWindow {
            fn drop(&mut self) {
                self.dropped.fetch_add(1, Ordering::SeqCst);
            }
        }
        fn owner(xid: Option<u64>, dropped: &Arc<AtomicUsize>) -> Parent {
            Parent::Window(Arc::new(TestWindow {
                xid,
                dropped: dropped.clone(),
            }))
        }
        fn ready<T>(future: impl Future<Output = T>) -> T {
            let mut future = Box::pin(future);
            match future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                Poll::Ready(value) => value,
                Poll::Pending => panic!("fixture must not access any desktop service"),
            }
        }
        #[test]
        fn closing_the_frontend_does_not_release_a_pending_portal_parent() {
            let dropped = Arc::new(AtomicUsize::new(0));
            let frontend_parent = owner(Some(0x1234), &dropped);
            let mut pending = Box::pin(with_parent(frontend_parent.clone(), |id| async move {
                assert_eq!(id.unwrap().to_string(), "x11:1234");
                std::future::pending::<()>().await;
            }));
            assert!(pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
            drop(frontend_parent);
            assert_eq!(dropped.load(Ordering::SeqCst), 0);
            drop(pending);
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
        }
        #[test]
        fn success_cancel_and_portal_failure_release_the_window_after_the_response() {
            for result in [
                Ok(Some(PathBuf::from("/tmp/image.png"))),
                Ok(None),
                Err("portal failed".to_owned()),
            ] {
                let dropped = Arc::new(AtomicUsize::new(0));
                let actual = ready(with_parent(owner(Some(42), &dropped), |id| async {
                    assert_eq!(id.unwrap().to_string(), "x11:2a");
                    assert_eq!(dropped.load(Ordering::SeqCst), 0);
                    result.clone()
                }));
                assert_eq!(actual, result);
                assert_eq!(dropped.load(Ordering::SeqCst), 1);
            }
        }
        #[test]
        fn panic_in_the_request_releases_its_parent() {
            let dropped = Arc::new(AtomicUsize::new(0));
            let result = std::panic::catch_unwind(|| {
                ready(with_parent(owner(Some(42), &dropped), |_| async {
                    panic!("request failed")
                }))
            });
            assert!(result.is_err());
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
        }
        #[test]
        fn no_handle_keeps_the_standalone_dialog_fallback() {
            let dropped = Arc::new(AtomicUsize::new(0));
            for parent in [Parent::None, owner(None, &dropped)] {
                ready(with_parent(parent, |id| async move {
                    assert!(id.is_none());
                }));
            }
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
        }
        #[test]
        fn wayland_export_tokens_are_forwarded_without_mangling() {
            let mut parent = PortalParent {
                identifier: Some(ashpd::WindowIdentifier::from_xdg_foreign_exported(
                    "~12l9jdl.-a".into(),
                )),
                connection: None,
                _owner: Parent::None,
            };
            assert_eq!(
                parent.request_identifier().unwrap().to_string(),
                "wayland:~12l9jdl.-a"
            );
            assert!(
                parent.identifier.is_some(),
                "export guard must remain owned"
            );
            parent.identifier = Some(ashpd::WindowIdentifier::from_xdg_foreign_exported(
                String::new(),
            ));
            assert!(parent.request_identifier().is_none());
        }
        #[test]
        fn file_uris_preserve_spaces_unicode_and_literal_percent_characters() {
            assert_eq!(
                local_uri("file:///tmp/%C3%A9t%C3%A9%20%2520%23.png").unwrap(),
                PathBuf::from("/tmp/été %20#.png")
            );
            assert_eq!(
                local_uri("file://localhost/tmp/a.png").unwrap(),
                PathBuf::from("/tmp/a.png")
            );
            for uri in [
                "https://example.com/a.png",
                "file://other-host/a.png",
                "file:///tmp/a.png?query",
                "file:///tmp/a.png#fragment",
            ] {
                assert!(local_uri(uri).is_err(), "{uri}");
            }
        }
    }
}
#[cfg(windows)]
mod platform {
    use super::*;
    use windows::{
        core::{w, HRESULT, PCWSTR},
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND},
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            },
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FileOpenDialog, IFileOpenDialog, IShellItem,
                SHCreateItemFromParsingName, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
                FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
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
    pub fn choose(parent: Parent, current: Option<&str>) -> Result<Option<PathBuf>, String> {
        native(parent,current).map_err(|e|format!("Could not open or read the Windows file chooser: {e}. You can paste a path or drop an image."))
    }
    fn native(parent: Parent, current: Option<&str>) -> windows::core::Result<Option<PathBuf>> {
        // The worker owns its COM apartment, dialog and allocated strings throughout.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let _apartment = Apartment;
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetTitle(w!("Choose wallpaper image"))?;
            dialog.SetOptions(
                dialog.GetOptions()?
                    | FOS_FORCEFILESYSTEM
                    | FOS_FILEMUSTEXIST
                    | FOS_PATHMUSTEXIST
                    | FOS_NOCHANGEDIR,
            )?;
            dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("Images (PNG, JPEG, BMP, GIF)"),
                pszSpec: w!("*.png;*.jpg;*.jpeg;*.bmp;*.gif"),
            }])?;
            if let Some(folder) = folder(current)
                .and_then(|p| p.to_str())
                .filter(|s| !s.contains('\0'))
            {
                let wide: Vec<u16> = folder.encode_utf16().chain(Some(0)).collect();
                if let Ok(item) =
                    SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide.as_ptr()), None)
                {
                    let _ = dialog.SetDefaultFolder(&item);
                }
            }
            let owner = match parent {
                Parent::Windows(id) => id,
                _ => 0,
            };
            if let Err(e) = dialog.Show(HWND(owner as *mut _)) {
                if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(e);
            }
            let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            if path.is_null() {
                return Err(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::E_UNEXPECTED,
                ));
            }
            let value = path.to_string();
            CoTaskMemFree(Some(path.0.cast()));
            Ok(Some(PathBuf::from(value?)))
        }
    }
}
#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::*;
    pub fn choose(_: Parent, _: Option<&str>) -> Result<Option<PathBuf>, String> {
        Err("Native image selection is not available on this platform. Paste a path or drop an image.".into())
    }
}
