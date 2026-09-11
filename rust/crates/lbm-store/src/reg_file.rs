//! Registry trees read from `.reg` files, the format regedit exports.
//!
//! The registry reader ([`registry_layout_store`](crate::registry_layout_store)) needs
//! a registry to read; this is one that exists on every platform. It makes the reader
//! and the import testable on fixtures, and a user's export of
//! `HKEY_CURRENT_USER\SOFTWARE\Mgth\LittleBigMouse` attached to an issue can be
//! replayed as is.
//!
//! Supported: both headers (`Windows Registry Editor Version 5.00`, `REGEDIT4`),
//! UTF-16LE (what regedit writes) and UTF-8 text, `[key]` and `[-key]` sections,
//! `"name"` and `@` value names, `"string"`, `dword:`, `hex:` and `hex(n):` data with
//! line continuations, `-` value deletions and `;` comments. Only string data
//! (`REG_SZ`, and `REG_EXPAND_SZ` as `hex(2):`) is readable through [`RegistryKey`], as
//! with C#'s `GetValue(name) as string`; the other kinds are kept but read as absent.

use std::cmp::Ordering;
use std::fmt;

use crate::registry_layout_store::RegistryKey;

/// Data of one registry value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegValue {
    /// `REG_SZ`.
    String(String),
    /// `REG_EXPAND_SZ`, unexpanded.
    ExpandString(String),
    /// `REG_DWORD`.
    Dword(u32),
    /// Any other kind: its type number and raw bytes.
    Other(u32, Vec<u8>),
}

/// A registry key held in memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryKey {
    name: String,
    values: Vec<(String, RegValue)>,
    subkeys: Vec<MemoryKey>,
}

/// The registry compares names case-insensitively (on their upper-case forms).
fn same_name(a: &str, b: &str) -> bool {
    a.to_uppercase() == b.to_uppercase()
}

fn compare_names(a: &str, b: &str) -> Ordering {
    a.to_uppercase().cmp(&b.to_uppercase())
}

impl MemoryKey {
    /// An empty key named `name`.
    pub fn new(name: impl Into<String>) -> Self {
        MemoryKey {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The values, in the order they were set.
    pub fn values(&self) -> &[(String, RegValue)] {
        &self.values
    }

    /// The direct subkeys, in the order they were created.
    pub fn subkeys(&self) -> &[MemoryKey] {
        &self.subkeys
    }

    /// The direct subkey `name`.
    pub fn subkey(&self, name: &str) -> Option<&MemoryKey> {
        self.subkeys.iter().find(|k| same_name(&k.name, name))
    }

    /// The key at `path` (`\`-separated) under this one, created as needed.
    pub fn create(&mut self, path: &str) -> &mut MemoryKey {
        let mut key = self;
        for part in path.split('\\').filter(|p| !p.is_empty()) {
            let index = match key.subkeys.iter().position(|k| same_name(&k.name, part)) {
                Some(i) => i,
                None => {
                    key.subkeys.push(MemoryKey::new(part));
                    key.subkeys.len() - 1
                }
            };
            key = &mut key.subkeys[index];
        }
        key
    }

    /// Removes the key at `path` and everything under it.
    pub fn delete(&mut self, path: &str) {
        let (parent, name) = match path.rfind('\\') {
            Some(i) => (&path[..i], &path[i + 1..]),
            None => ("", path),
        };
        let mut key = self;
        for part in parent.split('\\').filter(|p| !p.is_empty()) {
            match key.subkeys.iter().position(|k| same_name(&k.name, part)) {
                Some(i) => key = &mut key.subkeys[i],
                None => return,
            }
        }
        key.subkeys.retain(|k| !same_name(&k.name, name));
    }

    /// The value `name` (`""` for the default value).
    pub fn value(&self, name: &str) -> Option<&RegValue> {
        self.values
            .iter()
            .find(|(n, _)| same_name(n, name))
            .map(|(_, v)| v)
    }

    /// Sets the value `name`, replacing one of the same name.
    pub fn set_value(&mut self, name: impl Into<String>, value: RegValue) {
        let name = name.into();
        match self.values.iter_mut().find(|(n, _)| same_name(n, &name)) {
            Some(slot) => slot.1 = value,
            None => self.values.push((name, value)),
        }
    }

    /// Sets a `REG_SZ` value — what the C# writer stores for every value.
    pub fn set_string(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.set_value(name, RegValue::String(value.into()));
    }

    pub fn delete_value(&mut self, name: &str) {
        self.values.retain(|(n, _)| !same_name(n, name));
    }
}

impl<'a> RegistryKey for &'a MemoryKey {
    fn open(&self, path: &str) -> Option<&'a MemoryKey> {
        let mut key: &'a MemoryKey = self;
        for part in path.split('\\').filter(|p| !p.is_empty()) {
            key = key.subkey(part)?;
        }
        Some(key)
    }

    /// In the registry's order: sorted on the upper-case names.
    fn subkey_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.subkeys.iter().map(|k| k.name.clone()).collect();
        names.sort_by(|a, b| compare_names(a, b));
        names
    }

    fn string_value(&self, name: &str) -> Option<String> {
        match self.value(name)? {
            RegValue::String(s) => Some(s.clone()),
            RegValue::ExpandString(s) => Some(expand_environment(s)),
            _ => None,
        }
    }
}

/// `ExpandEnvironmentStrings`, as `GetValue` applies it to `REG_EXPAND_SZ`: every
/// `%NAME%` of a defined variable replaced by its value, the others left as they are.
fn expand_environment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => match std::env::var(&after[..end]) {
                Ok(value) if end > 0 => {
                    out.push_str(&value);
                    rest = &after[end + 1..];
                }
                _ => {
                    out.push('%');
                    rest = after;
                }
            },
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

//==================//
// Parsing          //
//==================//

/// Why a `.reg` file could not be read, with its 1-based line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegFileError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for RegFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ".reg line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for RegFileError {}

/// The text of a `.reg` file: UTF-16LE after its byte-order mark (regedit), UTF-8
/// otherwise (a BOM skipped, invalid bytes replaced).
fn decode(bytes: &[u8]) -> String {
    if let Some(utf16) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = utf16
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Parses a `.reg` file into a tree whose root holds the hives by name
/// (`HKEY_CURRENT_USER`, ...).
pub fn parse(bytes: &[u8]) -> Result<MemoryKey, RegFileError> {
    let text = decode(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let error = |line: usize, message: &str| RegFileError {
        line: line + 1,
        message: message.to_owned(),
    };

    let mut root = MemoryKey::new("");
    let mut i = 0;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    let unicode = match lines.get(i).map(|l| l.trim()) {
        Some("Windows Registry Editor Version 5.00") => true,
        Some("REGEDIT4") => false,
        _ => return Err(error(i, "not a .reg file (no header)")),
    };
    i += 1;

    // The key values go to; None after a [-key] section.
    let mut current: Option<String> = None;
    while i < lines.len() {
        let number = i;
        let line = lines[i].trim();
        i += 1;
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if let Some(section) = line.strip_prefix('[') {
            let path = section
                .strip_suffix(']')
                .ok_or_else(|| error(number, "unterminated key"))?;
            if let Some(deleted) = path.strip_prefix('-') {
                root.delete(deleted);
                current = None;
            } else {
                root.create(path);
                current = Some(path.to_owned());
            }
            continue;
        }

        let (name, data) = split_value(line).ok_or_else(|| error(number, "bad value line"))?;
        let Some(path) = &current else { continue };

        // Hex data wraps over lines ending with a backslash.
        let mut data = data.to_owned();
        while data.ends_with('\\') {
            data.pop();
            match lines.get(i) {
                Some(next) => {
                    data.push_str(next.trim());
                    i += 1;
                }
                None => return Err(error(number, "continuation past the end")),
            }
        }

        let key = root.create(path);
        if data == "-" {
            key.delete_value(&name);
            continue;
        }
        let value = parse_data(&data, unicode).ok_or_else(|| error(number, "bad value data"))?;
        key.set_value(name, value);
    }
    Ok(root)
}

/// `"name"=data` or `@=data`: the unescaped name and the raw data.
fn split_value(line: &str) -> Option<(String, &str)> {
    if let Some(data) = line.strip_prefix("@=") {
        return Some((String::new(), data.trim()));
    }
    let (name, rest) = quoted(line)?;
    let data = rest.trim_start().strip_prefix('=')?;
    Some((name, data.trim()))
}

/// A `"..."` string at the start of `s` (escapes `\\` and `\"`), and what follows.
fn quoted(s: &str) -> Option<(String, &str)> {
    let body = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &body[i + 1..])),
            '\\' => out.push(chars.next()?.1),
            c => out.push(c),
        }
    }
    None
}

fn parse_data(data: &str, unicode: bool) -> Option<RegValue> {
    if data.starts_with('"') {
        let (s, rest) = quoted(data)?;
        return rest.trim().is_empty().then_some(RegValue::String(s));
    }
    if let Some(hex) = data.strip_prefix("dword:") {
        return u32::from_str_radix(hex.trim(), 16)
            .ok()
            .map(RegValue::Dword);
    }
    let (kind, bytes) = if let Some(rest) = data.strip_prefix("hex:") {
        (3, rest)
    } else {
        let rest = data.strip_prefix("hex(")?;
        let close = rest.find("):")?;
        (
            u32::from_str_radix(&rest[..close], 16).ok()?,
            &rest[close + 2..],
        )
    };
    let bytes = bytes
        .split(',')
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(|b| u8::from_str_radix(b, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    Some(match kind {
        1 | 2 => {
            // A string in hex: UTF-16LE in version 5 files, ANSI in REGEDIT4 ones,
            // NUL-terminated either way.
            let text = if unicode {
                let units: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&pair| u16::from_le_bytes(pair))
                    .take_while(|u| *u != 0)
                    .collect();
                String::from_utf16_lossy(&units)
            } else {
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                String::from_utf8_lossy(&bytes[..end]).into_owned()
            };
            if kind == 1 {
                RegValue::String(text)
            } else {
                RegValue::ExpandString(text)
            }
        }
        4 if bytes.len() == 4 => RegValue::Dword(u32::from_le_bytes(bytes.try_into().ok()?)),
        kind => RegValue::Other(kind, bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"Windows Registry Editor Version 5.00

; a comment
[HKEY_CURRENT_USER\SOFTWARE\Mgth\LittleBigMouse]
"Priority"="High"
"Quoted"="a \"b\" c:\\d"
@="default"
"Count"=dword:0000002a
"Path"=hex(2):25,00,54,00,48,00,49,00,53,00,5f,00,49,00,53,00,5f,00,55,00,4e,\
  00,53,00,45,00,54,00,25,00,00,00
"Blob"=hex:01,02,03

[HKEY_CURRENT_USER\SOFTWARE\Mgth\LittleBigMouse\b]
[HKEY_CURRENT_USER\SOFTWARE\Mgth\LittleBigMouse\A]
"Gone"="x"
"Gone"=-
[-HKEY_CURRENT_USER\SOFTWARE\Mgth\LittleBigMouse\Deleted]
"Ignored"="x"
"#;

    fn sample() -> MemoryKey {
        parse(SAMPLE.as_bytes()).unwrap()
    }

    #[test]
    fn values_read_like_getvalue_as_string() {
        let tree = sample();
        let root = (&tree)
            .open(r"hkey_current_user\software\mgth\littlebigmouse")
            .expect("names are case-insensitive");
        assert_eq!(root.string_value("priority").as_deref(), Some("High"));
        assert_eq!(
            root.string_value("Quoted").as_deref(),
            Some(r#"a "b" c:\d"#)
        );
        assert_eq!(root.string_value("").as_deref(), Some("default"));
        // Not strings: absent to the reader, as `GetValue(name) as string` is null.
        assert_eq!(root.string_value("Count"), None);
        assert_eq!(root.value("Count"), Some(&RegValue::Dword(42)));
        assert_eq!(root.string_value("Blob"), None);
        // An unset variable is left as it is.
        assert_eq!(
            root.string_value("Path").as_deref(),
            Some("%THIS_IS_UNSET%")
        );
    }

    #[test]
    fn subkeys_come_sorted_like_the_registry_enumerates_them() {
        let tree = sample();
        let root = (&tree)
            .open(r"HKEY_CURRENT_USER\SOFTWARE\Mgth\LittleBigMouse")
            .unwrap();
        assert_eq!(root.subkey_names(), ["A", "b"]);
        assert_eq!(root.open("A").unwrap().string_value("Gone"), None);
        assert!(root.open("Deleted").is_none());
    }

    #[test]
    fn utf16_exports_and_regedit4_read_alike() {
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(SAMPLE.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(parse(&utf16).unwrap(), sample());

        let regedit4 = "REGEDIT4\r\n\r\n[HKEY_CURRENT_USER\\X]\r\n\"S\"=hex(2):61,62,00\r\n";
        let tree = parse(regedit4.as_bytes()).unwrap();
        let x = (&tree).open(r"HKEY_CURRENT_USER\X").unwrap();
        assert_eq!(x.string_value("S").as_deref(), Some("ab"));
    }

    #[test]
    fn malformed_files_are_refused_with_their_line() {
        assert_eq!(parse(b"[HKEY_CURRENT_USER\\X]").unwrap_err().line, 1);
        let bad = "REGEDIT4\n[HKEY_CURRENT_USER\\X]\n\"S\"=nonsense\n";
        assert_eq!(parse(bad.as_bytes()).unwrap_err().line, 3);
    }

    #[test]
    fn environment_expansion_keeps_unknown_and_lone_percents() {
        assert_eq!(expand_environment("100%"), "100%");
        assert_eq!(expand_environment("%%"), "%%");
        assert_eq!(
            expand_environment("a%LBM_SURELY_UNSET%b"),
            "a%LBM_SURELY_UNSET%b"
        );
    }
}
