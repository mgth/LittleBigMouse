//! The real registry behind [`RegistryKey`] (Windows only), read-only: the v6 import
//! copies the v5 registry tree and never changes it.

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    REG_EXPAND_SZ, REG_SZ, REG_VALUE_TYPE,
};

use crate::registry_layout_store::RegistryKey;

/// An open registry key, closed when dropped.
pub struct WindowsKey(HKEY);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn open(parent: HKEY, path: &str) -> Option<WindowsKey> {
    let path = wide(path);
    let mut key = HKEY::default();
    // SAFETY: `path` is NUL-terminated and outlives the call; `key` receives the handle.
    let status = unsafe { RegOpenKeyExW(parent, PCWSTR(path.as_ptr()), 0, KEY_READ, &mut key) };
    (status == ERROR_SUCCESS).then_some(WindowsKey(key))
}

impl WindowsKey {
    /// `Registry.CurrentUser.OpenSubKey(path)`: `HKEY_CURRENT_USER\<path>` for reading,
    /// `None` when it does not exist.
    pub fn open_current_user(path: &str) -> Option<WindowsKey> {
        open(HKEY_CURRENT_USER, path)
    }
}

impl Drop for WindowsKey {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `open` and is closed once.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// `ExpandEnvironmentStrings`, as `GetValue` applies it to a `REG_EXPAND_SZ`.
fn expand(s: &str) -> String {
    let source = wide(s);
    let mut buffer = vec![0u16; source.len().max(64)];
    loop {
        // SAFETY: `source` is NUL-terminated; the buffer's length is passed along.
        let needed =
            unsafe { ExpandEnvironmentStringsW(PCWSTR(source.as_ptr()), Some(&mut buffer)) }
                as usize;
        if needed == 0 {
            return s.to_owned();
        }
        if needed <= buffer.len() {
            return String::from_utf16_lossy(&buffer[..needed - 1]);
        }
        buffer.resize(needed, 0);
    }
}

impl RegistryKey for WindowsKey {
    fn open(&self, path: &str) -> Option<Self> {
        open(self.0, path)
    }

    fn subkey_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        // A key name is at most 255 characters.
        let mut buffer = [0u16; 256];
        for index in 0.. {
            let mut length = buffer.len() as u32;
            // SAFETY: the buffer and its length in characters are passed together.
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    PWSTR(buffer.as_mut_ptr()),
                    &mut length,
                    None,
                    PWSTR::null(),
                    None,
                    None,
                )
            };
            if status != ERROR_SUCCESS {
                // ERROR_NO_MORE_ITEMS, or a key that went away meanwhile.
                break;
            }
            names.push(String::from_utf16_lossy(&buffer[..length as usize]));
        }
        names
    }

    fn string_value(&self, name: &str) -> Option<String> {
        let name = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        // SAFETY: a size query: no data buffer, the type and size are written back.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
        };
        if status != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) {
            return None;
        }

        let mut data = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut bytes = (data.len() * 2) as u32;
        // SAFETY: `data` holds `bytes` bytes, as passed.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status != ERROR_SUCCESS || (kind != REG_SZ && kind != REG_EXPAND_SZ) {
            return None;
        }

        // .NET's GetValue drops one terminating NUL, and only one.
        let units = &data[..(bytes as usize / 2).min(data.len())];
        let units = match units.split_last() {
            Some((0, rest)) => rest,
            _ => units,
        };
        let value = String::from_utf16_lossy(units);
        Some(if kind == REG_EXPAND_SZ {
            expand(&value)
        } else {
            value
        })
    }
}
