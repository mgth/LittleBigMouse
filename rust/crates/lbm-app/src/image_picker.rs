//! Native file selection runs on a dedicated worker, never on egui's thread.
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default)]
pub enum Parent {
    #[default]
    None,
    X11(usize),
    Windows(isize),
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
        runtime.block_on(async {
            // Wayland permits an empty parent identifier. Never send a borrowed wl_surface
            // to a worker after the frontend may have destroyed it.
            let parent=match parent {Parent::X11(id)=>Some(ashpd::WindowIdentifier::from_xid(id as _)),_=>None};
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
        })
    }
    #[cfg(test)]
    mod tests {
        use super::*;
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
