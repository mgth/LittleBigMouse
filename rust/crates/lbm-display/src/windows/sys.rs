//! The Win32 calls of the Windows discovery, and nothing else: each fills a piece of
//! [`RawDisplays`] the way the C# call it replaces does, what a failure leaves
//! included. The rules applied to the answers are in `tree` and `registry`.

use std::mem::size_of;
use std::ptr::addr_of_mut;

use windows::core::{w, PCWSTR};
use windows::Wdk::System::Registry::{KeyNameInformation, NtQueryKey};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsExW,
    SetupDiOpenDevRegKey, DICS_FLAG_GLOBAL, DIGCF_PRESENT, DIGCF_PROFILE, DIREG_DEV,
    GUID_DEVCLASS_MONITOR, HDEVINFO, SP_DEVINFO_DATA,
};
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_MONITOR_SPECIALIZATION,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_GET_MONITOR_SPECIALIZATION, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ALL_PATHS,
};
use windows::Win32::Foundation::{
    BOOL, ERROR_ACCESS_DENIED, ERROR_BAD_IMPERSONATION_LEVEL, ERROR_MORE_DATA, ERROR_SUCCESS,
    HANDLE, HWND, LPARAM, LUID, RECT, STATUS_BUFFER_TOO_SMALL, TRUE,
};
use windows::Win32::Graphics::Gdi::{
    CreateDCW, DeleteDC, EnumDisplayDevicesW, EnumDisplayMonitors, EnumDisplaySettingsExW,
    GetDeviceCaps, GetMonitorInfoW, DEVMODEW, DISPLAY_DEVICEW, ENUM_CURRENT_SETTINGS,
    ENUM_DISPLAY_SETTINGS_FLAGS, HDC, HMONITOR, HORZRES, HORZSIZE, LOGPIXELSX, LOGPIXELSY,
    MONITORINFO, MONITORINFOEXW, VERTRES, VERTSIZE,
};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_CONFIG, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY, REG_DWORD, REG_DWORD_BIG_ENDIAN, REG_EXPAND_SZ,
    REG_MULTI_SZ, REG_NONE, REG_QWORD, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetDpiForMonitor, GetThreadDpiAwarenessContext,
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_ANGULAR_DPI,
    MDT_EFFECTIVE_DPI, MDT_RAW_DPI, MONITOR_DPI_TYPE,
};

use lbm_layout::model::DpiAwareness;

use super::registry::{
    decode_key_name_information, multi_string, registry_path, single_string, EdidEntry, EdidRead,
    Hive, InvalidKeyName, RegistryValue, MAX_KEY_LENGTH,
};
use super::tree::{
    DeviceCaps, DeviceEntry, DpiReading, Luid, RawAdapter, RawDevMode, RawDisplays, RawMonitor,
    RawMonitorInfo, RawRect, Target,
};

/// `EDD_GET_DEVICE_INTERFACE_NAME`: `DeviceID` receives the device interface path.
const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;

/// `DM_SPECVERSION` as C# passes it for the current mode (`SpecVersionEnum.Win8`).
const SPEC_VERSION_WIN8: u16 = 0x0602;

/// Everything `GetDisplayDevices` asks Windows, in one go.
pub(super) fn read() -> RawDisplays {
    RawDisplays {
        adapters: adapters(),
        monitor_infos: monitor_infos(),
        targets: targets(),
        edids: edid_table(),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// A fixed UTF-16 buffer up to its first NUL, as .NET's `ByValTStr` reads it.
fn text(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

//=====================//
// EnumDisplayDevices  //
//=====================//

/// `EnumDisplayDevices(device, index, entry, flags)`. C# reuses `entry` from one call
/// to the next: a member a call leaves unwritten keeps the previous device's.
fn enum_display_device(
    device: Option<&[u16]>,
    index: u32,
    flags: u32,
    entry: &mut DISPLAY_DEVICEW,
) -> bool {
    entry.cb = size_of::<DISPLAY_DEVICEW>() as u32;
    let name = device.map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr()));
    // SAFETY: `name` is null or a NUL-terminated buffer that outlives the call, and
    // `entry` is a DISPLAY_DEVICEW whose `cb` says so.
    unsafe { EnumDisplayDevicesW(name, index, entry, flags) }.as_bool()
}

fn device_entry(d: &DISPLAY_DEVICEW) -> DeviceEntry {
    DeviceEntry {
        device_name: text(&d.DeviceName),
        device_string: text(&d.DeviceString),
        state_flags: d.StateFlags,
        device_id: text(&d.DeviceID),
    }
}

/// `BuildDisplayDeviceAndChildren` from the root: the adapter sources, and below each
/// its monitors. C#'s recursion also asks for the devices below each monitor; Windows
/// has no level there.
fn adapters() -> Vec<RawAdapter> {
    let mut adapters = Vec::new();
    let mut child = DISPLAY_DEVICEW::default();
    for i in 0..=u32::MAX {
        if !enum_display_device(None, i, 0, &mut child) {
            break;
        }
        let device = device_entry(&child);
        let name = wide(&device.device_name);
        adapters.push(RawAdapter {
            capabilities: device_caps(&name),
            current_mode: current_mode(&name),
            monitors: monitors_of(&name),
            device,
        });
    }
    adapters
}

/// The monitors `EnumDisplayDevices(source, j)` lists, each with its device interface
/// path.
fn monitors_of(adapter: &[u16]) -> Vec<RawMonitor> {
    let mut monitors = Vec::new();
    let mut child = DISPLAY_DEVICEW::default();
    for j in 0..=u32::MAX {
        if !enum_display_device(Some(adapter), j, 0, &mut child) {
            break;
        }
        let mut interface = DISPLAY_DEVICEW::default();
        let interface_path = if enum_display_device(
            Some(adapter),
            j,
            EDD_GET_DEVICE_INTERFACE_NAME,
            &mut interface,
        ) {
            text(&interface.DeviceID)
        } else {
            String::new()
        };
        monitors.push(RawMonitor {
            device: device_entry(&child),
            interface_path,
        });
    }
    monitors
}

/// C# `BuildDeviceCaps`: `GetDeviceCaps` on `CreateDC("DISPLAY", source)`. Without a
/// DC, C# asks anyway and gets 0s.
fn device_caps(name: &[u16]) -> DeviceCaps {
    // SAFETY: both strings are NUL-terminated and outlive the call; no DEVMODE is passed.
    let hdc = unsafe { CreateDCW(w!("DISPLAY"), PCWSTR(name.as_ptr()), PCWSTR::null(), None) };
    // SAFETY: GetDeviceCaps takes any DC value, a null one included (it answers 0).
    let cap = |index| unsafe { GetDeviceCaps(hdc, index) };
    let caps = DeviceCaps {
        horz_size: cap(HORZSIZE),
        vert_size: cap(VERTSIZE),
        horz_res: cap(HORZRES),
        vert_res: cap(VERTRES),
        log_pixels_x: cap(LOGPIXELSX),
        log_pixels_y: cap(LOGPIXELSY),
    };
    if !hdc.is_invalid() {
        // SAFETY: the DC was created above and is deleted once.
        unsafe {
            let _ = DeleteDC(hdc);
        }
    }
    caps
}

/// C# `GetCurrentMode`: `EnumDisplaySettingsEx(source, ENUM_CURRENT_SETTINGS)`,
/// `None` when it fails.
fn current_mode(name: &[u16]) -> Option<RawDevMode> {
    let mut dm = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        dmSpecVersion: SPEC_VERSION_WIN8,
        ..Default::default()
    };
    // SAFETY: `name` is NUL-terminated and outlives the call; `dm` is a DEVMODEW whose
    // size says so, with no driver data.
    let found = unsafe {
        EnumDisplaySettingsExW(
            PCWSTR(name.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut dm,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        )
    };
    if !found.as_bool() {
        return None;
    }
    // SAFETY: both members of the union are plain integers, any bits are valid; a
    // display's DEVMODE uses the display one.
    let display = unsafe { dm.Anonymous1.Anonymous2 };
    Some(RawDevMode {
        fields: dm.dmFields.0,
        position_x: display.dmPosition.x,
        position_y: display.dmPosition.y,
        display_orientation: display.dmDisplayOrientation.0,
        pels_width: dm.dmPelsWidth,
        pels_height: dm.dmPelsHeight,
        display_frequency: dm.dmDisplayFrequency,
    })
}

//=====================//
// EnumDisplayMonitors //
//=====================//

/// `EnumDisplayMonitors`: each `HMONITOR` whose `GetMonitorInfo` succeeds, with its
/// three DPIs.
pub(super) fn monitor_infos() -> Vec<RawMonitorInfo> {
    let mut infos: Vec<RawMonitorInfo> = Vec::new();
    // SAFETY: the callback runs only during the call and gets back the pointer to
    // `infos`, which outlives it.
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(monitor_callback),
            LPARAM(addr_of_mut!(infos) as isize),
        );
    }
    infos
}

unsafe extern "system" fn monitor_callback(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: `data` is the `Vec` `monitor_infos` passed, alive and not otherwise
    // borrowed during the enumeration.
    let infos = unsafe { &mut *(data.0 as *mut Vec<RawMonitorInfo>) };
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: `mi` is a MONITORINFOEXW whose cbSize says so.
    if !unsafe { GetMonitorInfoW(hmonitor, addr_of_mut!(mi).cast::<MONITORINFO>()) }.as_bool() {
        return TRUE;
    }
    let rect = |r: RECT| RawRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    };
    infos.push(RawMonitorInfo {
        device_name: text(&mi.szDevice),
        flags: mi.monitorInfo.dwFlags,
        monitor: rect(mi.monitorInfo.rcMonitor),
        work: rect(mi.monitorInfo.rcWork),
        effective_dpi: dpi(hmonitor, MDT_EFFECTIVE_DPI),
        angular_dpi: dpi(hmonitor, MDT_ANGULAR_DPI),
        raw_dpi: dpi(hmonitor, MDT_RAW_DPI),
    });
    TRUE
}

/// `GetDpiForMonitor`: what it wrote over zeros, as C#'s `out` locals, and whether it
/// succeeded.
fn dpi(hmonitor: HMONITOR, kind: MONITOR_DPI_TYPE) -> DpiReading {
    let (mut x, mut y) = (0u32, 0u32);
    // SAFETY: two valid out pointers.
    let succeeded = unsafe { GetDpiForMonitor(hmonitor, kind, &mut x, &mut y) }.is_ok();
    DpiReading { x, y, succeeded }
}

//=====================//
// CCD                 //
//=====================//

/// C# `QueryDisplayConfigPaths(QDC_ALL_PATHS)` and what `UpdateMonitorNumbers` and
/// `UpdateSpecializedMonitors` ask about its available targets: each target once, its
/// device path and specialization. `None` when the query fails, as in C# (no retry).
fn targets() -> Option<Vec<Target>> {
    let (mut path_count, mut mode_count) = (0u32, 0u32);
    // SAFETY: two valid out pointers.
    if unsafe { GetDisplayConfigBufferSizes(QDC_ALL_PATHS, &mut path_count, &mut mode_count) }
        != ERROR_SUCCESS
    {
        return None;
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count.max(1) as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count.max(1) as usize];
    // SAFETY: the arrays hold at least the counts passed; QDC_ALL_PATHS wants no
    // topology id.
    let status = unsafe {
        QueryDisplayConfig(
            QDC_ALL_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    paths.truncate(path_count as usize);

    let mut targets: Vec<Target> = Vec::new();
    for path in &paths {
        let target = &path.targetInfo;
        if !target.targetAvailable.as_bool() {
            continue;
        }
        let adapter = Luid {
            low_part: target.adapterId.LowPart,
            high_part: target.adapterId.HighPart,
        };
        if targets
            .iter()
            .any(|t| t.adapter == adapter && t.id == target.id)
        {
            continue;
        }
        targets.push(Target {
            adapter,
            id: target.id,
            device_path: target_device_path(target.adapterId, target.id),
            specialized: is_specialized(target.adapterId, target.id),
        });
    }
    Some(targets)
}

/// C# `GetTargetDevicePath`: the monitor's device interface path, `""` when the query
/// fails.
fn target_device_path(adapter: LUID, id: u32) -> String {
    let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: adapter,
            id,
        },
        ..Default::default()
    };
    // SAFETY: the pointer covers the whole packet, whose size the header gives.
    let status = unsafe { DisplayConfigGetDeviceInfo(addr_of_mut!(name).cast()) };
    if status == 0 {
        text(&name.monitorDevicePath)
    } else {
        String::new()
    }
}

/// C# `IsSpecializedTarget`: specialization enabled; false when the query fails, as it
/// does before Windows 11.
fn is_specialized(adapter: LUID, id: u32) -> bool {
    let mut request = DISPLAYCONFIG_GET_MONITOR_SPECIALIZATION {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_MONITOR_SPECIALIZATION,
            size: size_of::<DISPLAYCONFIG_GET_MONITOR_SPECIALIZATION>() as u32,
            adapterId: adapter,
            id,
        },
        ..Default::default()
    };
    // SAFETY: the pointer covers the whole packet, whose size the header gives.
    let status = unsafe { DisplayConfigGetDeviceInfo(addr_of_mut!(request).cast()) };
    // SAFETY: both members of the union are a plain u32.
    status == 0 && unsafe { request.Anonymous.value } & 1 != 0
}

//=====================//
// SetupAPI, registry  //
//=====================//

/// A registry key: one this module opened (closed when dropped) or a predefined root.
struct Key {
    handle: HKEY,
    owned: bool,
}

impl Drop for Key {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: the key was opened here and is closed once.
            unsafe {
                let _ = RegCloseKey(self.handle);
            }
        }
    }
}

/// What .NET's `OpenSubKey` gives: a key, `null`, or an exception.
enum Opened {
    Key(Key),
    Null,
    Throws,
}

/// `key.OpenSubKey(name)` for reading: a name over 255 characters throws, access
/// denied throws (`SecurityException`), any other failure is `null`.
fn open_subkey(parent: &Key, name: &str) -> Opened {
    if name.encode_utf16().count() > MAX_KEY_LENGTH {
        return Opened::Throws;
    }
    let name = wide(name);
    let mut handle = HKEY::default();
    // SAFETY: `name` is NUL-terminated and outlives the call; `handle` receives the key.
    let status = unsafe {
        RegOpenKeyExW(
            parent.handle,
            PCWSTR(name.as_ptr()),
            0,
            KEY_READ,
            &mut handle,
        )
    };
    if status == ERROR_SUCCESS && !handle.is_invalid() {
        Opened::Key(Key {
            handle,
            owned: true,
        })
    } else if status == ERROR_ACCESS_DENIED || status == ERROR_BAD_IMPERSONATION_LEVEL {
        Opened::Throws
    } else {
        Opened::Null
    }
}

/// C# `WinReg.RegistryKey(path, parent)`: the hive the kernel path names, then each
/// subkey opened in turn; `null` as soon as one is missing.
fn open_path(path: &str, parent: usize) -> Opened {
    let Some((hive, subkeys)) = registry_path(path, parent) else {
        return Opened::Throws;
    };
    let mut key = Key {
        handle: match hive {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::CurrentConfig => HKEY_CURRENT_CONFIG,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        },
        owned: false,
    };
    for name in subkeys {
        match open_subkey(&key, name) {
            Opened::Key(next) => key = next,
            other => return other,
        }
    }
    Opened::Key(key)
}

/// `RegQueryValueEx` into a buffer of `size` bytes, as .NET's second query: the buffer
/// whatever the call wrote, and whether it succeeded.
fn query_value(key: &Key, name: &[u16], size: u32) -> (Vec<u8>, bool) {
    let mut data = vec![0u8; size as usize];
    let mut written = size;
    let mut kind = REG_VALUE_TYPE::default();
    // SAFETY: `data` holds `written` bytes, as passed; `name` is NUL-terminated.
    let status = unsafe {
        RegQueryValueExW(
            key.handle,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(data.as_mut_ptr()),
            Some(&mut written),
        )
    };
    (data, status == ERROR_SUCCESS)
}

fn utf16(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect()
}

/// .NET's `RegistryKey.GetValue(name)`: the value by its type, `null` when it is
/// missing or of a type .NET does not convert.
fn get_value(key: &Key, name: &str) -> RegistryValue {
    let name = wide(name);
    let mut kind = REG_VALUE_TYPE::default();
    let mut size = 0u32;
    // SAFETY: a size query: no buffer, the type and size are written back.
    let status = unsafe {
        RegQueryValueExW(
            key.handle,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS && status != ERROR_MORE_DATA {
        return RegistryValue::Null;
    }
    // Strings: .NET rounds an odd byte count up.
    let even = size.saturating_add(size & 1);
    match kind {
        REG_NONE | REG_BINARY | REG_DWORD_BIG_ENDIAN => {
            RegistryValue::Binary(query_value(key, &name, size).0)
        }
        REG_DWORD if size <= 4 => {
            let (bytes, _) = query_value(key, &name, 4);
            RegistryValue::Dword(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        }
        REG_DWORD | REG_QWORD if size <= 8 => {
            let (bytes, _) = query_value(key, &name, 8);
            let mut qword = [0u8; 8];
            qword.copy_from_slice(&bytes[..8]);
            RegistryValue::Qword(i64::from_le_bytes(qword))
        }
        REG_DWORD | REG_QWORD => RegistryValue::Binary(query_value(key, &name, size).0),
        REG_SZ => RegistryValue::String(single_string(&utf16(&query_value(key, &name, even).0))),
        REG_EXPAND_SZ => RegistryValue::String(expand(&single_string(&utf16(
            &query_value(key, &name, even).0,
        )))),
        REG_MULTI_SZ => {
            let (bytes, succeeded) = query_value(key, &name, even);
            RegistryValue::MultiString(if succeeded {
                multi_string(&utf16(&bytes))
            } else {
                Vec::new()
            })
        }
        _ => RegistryValue::Null,
    }
}

/// `Environment.ExpandEnvironmentVariables`, which `GetValue` applies to a
/// `REG_EXPAND_SZ`.
fn expand(s: &str) -> String {
    let source = wide(s);
    let mut buffer = vec![0u16; source.len().max(64)];
    loop {
        // SAFETY: `source` is NUL-terminated; the buffer is passed with its length.
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

/// C# `WinReg.GetHKeyName`: the kernel's name of an open key
/// (`\REGISTRY\MACHINE\SYSTEM\...`), `""` when `NtQueryKey` does not give one.
fn key_name(key: HKEY) -> Result<String, InvalidKeyName> {
    let handle = HANDLE(key.0);
    let mut needed = 0u32;
    // SAFETY: a size query: no buffer, the needed length is written back.
    let status = unsafe { NtQueryKey(handle, KeyNameInformation, None, 0, &mut needed) };
    if status != STATUS_BUFFER_TOO_SMALL || needed < 4 {
        return Ok(String::new());
    }
    // u32s: KEY_NAME_INFORMATION starts with a ULONG.
    let mut buffer = vec![0u32; (needed as usize).div_ceil(4)];
    let mut returned = 0u32;
    // SAFETY: `buffer` holds at least `needed` bytes, as passed.
    let status = unsafe {
        NtQueryKey(
            handle,
            KeyNameInformation,
            Some(buffer.as_mut_ptr().cast()),
            needed,
            &mut returned,
        )
    };
    if status.0 != 0 || returned < 4 || returned > needed {
        return Ok(String::new());
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .flat_map(|unit| unit.to_ne_bytes())
        .take(returned as usize)
        .collect();
    decode_key_name_information(&bytes)
}

/// One SetupAPI monitor device, as far as C#'s `GetEdid` loop goes with it: `None`
/// where it `continue`s (or catches) before comparing ids.
fn edid_entry(device_key: &Key) -> Option<EdidEntry> {
    let key_name = match key_name(device_key.handle) {
        Ok(name) if !name.is_empty() => name,
        _ => return None,
    };
    let Opened::Key(parent) = open_path(&key_name, 1) else {
        return None;
    };
    let RegistryValue::MultiString(hardware_ids) = get_value(&parent, "HardwareID") else {
        return None;
    };
    let first = hardware_ids.first()?;
    let id = format!("{first}\\{}", get_value(&parent, "Driver").concat_text());
    let edid = match open_path(&key_name, 0) {
        Opened::Throws => EdidRead::Failed,
        Opened::Null => EdidRead::Missing,
        Opened::Key(key) => match get_value(&key, "EDID") {
            RegistryValue::Binary(bytes) => EdidRead::Bytes(bytes),
            _ => EdidRead::Missing,
        },
    };
    Some(EdidEntry { key_name, id, edid })
}

/// SetupAPI's present monitor devices (`DIGCF_PRESENT | DIGCF_PROFILE`), in order: the
/// table [`super::registry::find_edid`] answers each monitor from.
fn edid_table() -> Vec<EdidEntry> {
    let mut entries = Vec::new();
    // SAFETY: plain call with a class GUID that outlives it; the set is destroyed below.
    let devinfo = unsafe {
        SetupDiGetClassDevsExW(
            Some(&GUID_DEVCLASS_MONITOR),
            PCWSTR::null(),
            HWND::default(),
            DIGCF_PRESENT | DIGCF_PROFILE,
            HDEVINFO::default(),
            PCWSTR::null(),
            None,
        )
    };
    let Ok(devinfo) = devinfo else {
        return entries;
    };
    for i in 0..=u32::MAX {
        let mut data = SP_DEVINFO_DATA {
            cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        // SAFETY: `data` is an SP_DEVINFO_DATA whose cbSize says so.
        if unsafe { SetupDiEnumDeviceInfo(devinfo, i, &mut data) }.is_err() {
            break;
        }
        // SAFETY: `data` was just filled for this set.
        let key = unsafe {
            SetupDiOpenDevRegKey(devinfo, &data, DICS_FLAG_GLOBAL.0, 0, DIREG_DEV, KEY_READ.0)
        };
        let Ok(handle) = key else {
            continue;
        };
        let device_key = Key {
            handle,
            owned: true,
        };
        if let Some(entry) = edid_entry(&device_key) {
            entries.push(entry);
        }
    }
    // SAFETY: the set was created above and is destroyed once.
    unsafe {
        let _ = SetupDiDestroyDeviceInfoList(devinfo);
    }
    entries
}

//=====================//
// DPI awareness       //
//=====================//

pub(super) fn thread_dpi_awareness() -> DpiAwareness {
    // SAFETY: plain queries on the calling thread's own context.
    let awareness = unsafe { GetAwarenessFromDpiAwarenessContext(GetThreadDpiAwarenessContext()) };
    super::dpi_awareness(awareness.0)
}

pub(super) fn set_process_per_monitor_dpi_aware() {
    // SAFETY: plain call; it fails, harmlessly, when the awareness is already set.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
