//! The registry side of C#'s `MonitorDeviceHelper.GetEdid`, without the registry:
//! what .NET makes of the values it reads (`RegistryKey.GetValue`), the key name the
//! kernel reports (`WinReg.GetHKeyName`) and how `WinReg.RegistryKey` walks it, and
//! the lookup of a monitor's EDID in the table of SetupAPI's monitor devices.

use crate::edid::{self, Edid};

/// What .NET's `RegistryKey.GetValue(name)` returns, by registry type.
#[derive(Clone, Debug, PartialEq)]
pub enum RegistryValue {
    /// Missing, unreadable, or of a type .NET does not convert (`REG_LINK`,
    /// resource lists): `null`.
    Null,
    /// `REG_SZ`, `REG_EXPAND_SZ` (expanded): a `string`, one terminating NUL dropped.
    String(String),
    /// `REG_MULTI_SZ`: a `string[]` (see [`multi_string`]).
    MultiString(Vec<String>),
    /// `REG_BINARY`, `REG_NONE`, `REG_DWORD_BIG_ENDIAN`, and a `REG_DWORD` or
    /// `REG_QWORD` too long for its type: a `byte[]`.
    Binary(Vec<u8>),
    /// `REG_DWORD`: an `int`.
    Dword(i32),
    /// `REG_QWORD`, and a `REG_DWORD` of five to eight bytes: a `long`.
    Qword(i64),
}

impl RegistryValue {
    /// The value in a C# string concatenation (`s + value`): its `ToString()`, `""`
    /// for null.
    pub fn concat_text(&self) -> String {
        match self {
            RegistryValue::Null => String::new(),
            RegistryValue::String(s) => s.clone(),
            RegistryValue::MultiString(_) => "System.String[]".to_owned(),
            RegistryValue::Binary(_) => "System.Byte[]".to_owned(),
            RegistryValue::Dword(v) => v.to_string(),
            RegistryValue::Qword(v) => v.to_string(),
        }
    }
}

/// .NET's reading of a `REG_MULTI_SZ`: a NUL is added if the data does not end with
/// one, then every NUL-terminated string is kept, empty ones included, except the
/// empty one the final terminator makes.
pub fn multi_string(data: &[u16]) -> Vec<String> {
    let mut blob = data.to_vec();
    if blob.last().is_some_and(|&c| c != 0) {
        blob.push(0);
    }
    let len = blob.len();
    let mut strings = Vec::new();
    let mut cur = 0;
    while cur < len {
        let next_null = blob[cur..]
            .iter()
            .position(|&c| c == 0)
            .map_or(len, |p| cur + p);
        if next_null < len {
            if next_null > cur {
                strings.push(String::from_utf16_lossy(&blob[cur..next_null]));
            } else if next_null != len - 1 {
                strings.push(String::new());
            }
        } else {
            strings.push(String::from_utf16_lossy(&blob[cur..len]));
        }
        cur = next_null + 1;
    }
    strings
}

/// .NET's reading of a `REG_SZ`: one terminating NUL dropped, and only one.
pub fn single_string(data: &[u16]) -> String {
    let data = match data.split_last() {
        Some((0, rest)) => rest,
        _ => data,
    };
    String::from_utf16_lossy(data)
}

/// A `KEY_NAME_INFORMATION` whose byte length is odd or runs past the buffer: C#'s
/// `InvalidDataException`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidKeyName;

/// C# `WinReg.DecodeKeyNameInformation`: a `KEY_NAME_INFORMATION` — a byte length,
/// then that many bytes of UTF-16 — as a string, reading nothing past the buffer. A
/// buffer shorter than the length field is `""`.
pub fn decode_key_name_information(buffer: &[u8]) -> Result<String, InvalidKeyName> {
    let [a, b, c, d, rest @ ..] = buffer else {
        return Ok(String::new());
    };
    let name_length = u32::from_le_bytes([*a, *b, *c, *d]) as usize;
    if name_length & 1 != 0 || name_length > rest.len() {
        return Err(InvalidKeyName);
    }
    let units: Vec<u16> = rest[..name_length]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&unit| u16::from_le_bytes(unit))
        .collect();
    Ok(String::from_utf16_lossy(&units))
}

/// The hive a kernel key path names, as C#'s `WinReg.RegistryKey` maps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hive {
    /// `\REGISTRY\USER\...` (C# opens `HKEY_CURRENT_USER`, not `HKEY_USERS`).
    CurrentUser,
    /// `\REGISTRY\CONFIG\...`.
    CurrentConfig,
    /// `\REGISTRY\MACHINE\...`, and anything else.
    LocalMachine,
}

/// C# `WinReg.RegistryKey(path, parent)` before it opens anything: the hive of a
/// kernel key path (`\REGISTRY\MACHINE\SYSTEM\...`) and the subkeys to open from it one
/// at a time, the last `parent` of them left out. `None` where C# throws: a path of
/// fewer than three segments.
pub fn registry_path(path: &str, parent: usize) -> Option<(Hive, Vec<&str>)> {
    let keys: Vec<&str> = path.split('\\').collect();
    if keys.len() < 3 {
        return None;
    }
    let hive = match keys[2] {
        "USER" => Hive::CurrentUser,
        "CONFIG" => Hive::CurrentConfig,
        _ => Hive::LocalMachine,
    };
    let end = keys.len().saturating_sub(parent);
    Some((
        hive,
        keys.get(3..end).map(<[&str]>::to_vec).unwrap_or_default(),
    ))
}

/// Longest key name .NET's `OpenSubKey` accepts, in UTF-16 units: a longer one
/// throws `ArgumentException`.
pub const MAX_KEY_LENGTH: usize = 255;

/// One monitor device SetupAPI lists, as far as C#'s `GetEdid` gets with it before
/// comparing ids. The devices it drops earlier — no device key, a key name it cannot
/// read, a parent key it cannot open, a `HardwareID` that is not a non-empty
/// `REG_MULTI_SZ` — are not entries: they match no monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct EdidEntry {
    /// The device key as the kernel names it
    /// (`\REGISTRY\MACHINE\SYSTEM\ControlSet001\Enum\DISPLAY\DEL4065\...\Device Parameters`):
    /// C#'s `HKeyName`, which the parsed EDID keeps.
    pub key_name: String,
    /// `HardwareID[0] + "\" + Driver`, what C# compares with the monitor's device id:
    /// `MONITOR\DEL4065\{4d36e96e-e325-11ce-bfc1-08002be10318}\0003`.
    pub id: String,
    pub edid: EdidRead,
}

/// What reading a device's `EDID` value gave.
#[derive(Clone, Debug, PartialEq)]
pub enum EdidRead {
    /// The bytes of a binary value.
    Bytes(Vec<u8>),
    /// No device key to reopen, or no binary `EDID` value in it: C# returns null, and
    /// the monitor has no EDID even if a later device has the same id.
    Missing,
    /// Reopening the key threw (access denied, a name too long): C# catches it and
    /// goes on with the next device.
    Failed,
}

/// C# `MonitorDeviceHelper.GetEdid(deviceId)`, over the table of SetupAPI's monitor
/// devices in SetupAPI's order: the first entry with the id whose EDID reads without
/// an exception decides — parsed, or `None` when it has none. A block C#'s parser
/// would throw on counts as an exception ([`edid::csharp_throws`]).
pub fn find_edid(entries: &[EdidEntry], device_id: &str) -> Option<Edid> {
    for entry in entries.iter().filter(|e| e.id == device_id) {
        match &entry.edid {
            EdidRead::Failed => continue,
            EdidRead::Missing => return None,
            EdidRead::Bytes(bytes) if edid::csharp_throws(bytes) => continue,
            EdidRead::Bytes(bytes) => return Some(edid::parse(entry.key_name.clone(), bytes)),
        }
    }
    None
}
