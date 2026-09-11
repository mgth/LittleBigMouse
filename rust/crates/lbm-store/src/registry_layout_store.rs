//! The Windows registry store, read side — port of
//! `LittleBigMouse.Platform.Windows/RegistryLayoutStore.cs` (`Read` and its helpers)
//! and of the `TryGet*` helpers of `RegistryExt.cs`.
//!
//! Up to v5, Windows kept everything under `HKCU\SOFTWARE\Mgth\LittleBigMouse`. v6
//! stores JSON on every platform (decision D2) and imports the registry once, at the
//! first launch ([`registry_import`](crate::registry_import)); the registry itself is
//! left intact so that going back to 5.x loses nothing. Nothing here writes.
//!
//! The format, as the C# writer left it: every value is a `REG_SZ` string — numbers in
//! the invariant culture, bools as `"1"`/`"0"` — and a name holding a backslash
//! addresses a value in a subkey (`Borders\Left`). Legacy locations are honored as in
//! C#: options that once lived in the layout key, the old `ShowAttachDetachWarning`
//! name, and the pre-split whole-edge resistance VALUE.
//!
//! The registry is reached through [`RegistryKey`], so the reader runs on fixtures
//! everywhere ([`reg_file`](crate::reg_file)) and on the real registry on Windows
//! ([`windows_registry`](crate::windows_registry)).

use crate::layout_dtos::{
    BorderResistanceDto, BorderSectionDto, BorderSideDto, BordersDto, GlobalOptionsDto, LayoutDto,
    LayoutOptionsDto, ModelDto, MonitorDto, SourceDto,
};
use crate::layout_store::LayoutStoreData;
use crate::layout_store_key::key_for;

/// C# `RegistryLayoutStore.DefaultRootKey`, under `HKEY_CURRENT_USER`.
pub const DEFAULT_ROOT_KEY: &str = r"SOFTWARE\Mgth\LittleBigMouse";

/// A registry key, as much of it as the reader needs. Key and value names are
/// case-insensitive, as in the registry.
pub trait RegistryKey: Sized {
    /// `OpenSubKey(path)`: a subkey, `path` possibly several levels deep
    /// (`\`-separated); `None` when absent.
    fn open(&self, path: &str) -> Option<Self>;

    /// `GetSubKeyNames()`: the direct subkeys' names, in the registry's order.
    fn subkey_names(&self) -> Vec<String>;

    /// `GetValue(name) as string`: a string value (`REG_SZ`, or `REG_EXPAND_SZ`
    /// expanded); `None` when absent or of another kind.
    fn string_value(&self, name: &str) -> Option<String>;
}

//==================//
// RegistryExt      //
//==================//

/// C# `RegistryExt.TryGetString`: a stored string, `name` possibly addressing a value
/// in a subkey (`Borders\Left`); `None` when absent.
pub fn try_get_string<K: RegistryKey>(key: &K, name: &str) -> Option<String> {
    match name.rfind('\\') {
        Some(i) => key.open(&name[..i])?.string_value(&name[i + 1..]),
        None => key.string_value(name),
    }
}

/// C# `RegistryExt.TryGet`: a stored double, `None` when absent or unparsable
/// (`double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture)`).
pub fn try_get<K: RegistryKey>(key: &K, name: &str) -> Option<f64> {
    parse_double(&try_get_string(key, name)?)
}

/// C# `RegistryExt.TryGetBool`: a stored bool, `"1"` being true and anything else
/// false; `None` when absent.
pub fn try_get_bool<K: RegistryKey>(key: &K, name: &str) -> Option<bool> {
    try_get_string(key, name).map(|s| s == "1")
}

/// C# `RegistryExt.TryGetInt`: a stored int, `None` when absent or unparsable
/// (`int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture)`).
pub fn try_get_int<K: RegistryKey>(key: &K, name: &str) -> Option<i32> {
    parse_int(&try_get_string(key, name)?)
}

/// .NET's white space for number parsing: U+0009 to U+000D and U+0020. Trailing NULs
/// are accepted as well.
fn trim_dotnet(s: &str) -> &str {
    let white = |c: char| c == ' ' || ('\t'..='\r').contains(&c);
    s.trim_start_matches(white)
        .trim_end_matches(|c: char| white(c) || c == '\0')
}

/// `double.TryParse(s, NumberStyles.Float, CultureInfo.InvariantCulture)`: white space
/// around, a sign, digits with a `.`, an exponent; `NaN` and `Infinity` (any case,
/// signed); correctly rounded. No thousands separator, no hexadecimal.
pub fn parse_double(s: &str) -> Option<f64> {
    let s = trim_dotnet(s);
    let (negative, unsigned) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if unsigned.eq_ignore_ascii_case("NaN") {
        return Some(f64::NAN);
    }
    if unsigned.eq_ignore_ascii_case("Infinity") {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }

    // Rust's parser takes the same numbers, but also "inf": check the shape first.
    let bytes = unsigned.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let start = *i;
        while bytes.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i - start
    };
    let mut mantissa = digits(&mut i);
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        mantissa += digits(&mut i);
    }
    if mantissa == 0 {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return None;
        }
    }
    if i != bytes.len() {
        return None;
    }
    s.parse().ok()
}

/// `int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture)`: white space
/// around, a sign, decimal digits, within `i32`.
pub fn parse_int(s: &str) -> Option<i32> {
    let s = trim_dotnet(s);
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

//==================//
// Read             //
//==================//

/// C# `RegistryLayoutStore.LayoutKeyPath`: a registry key name is capped at 255
/// characters and a nine-monitor id is longer (#589), so the key is derived from the
/// id, and is the id itself whenever it fits.
pub fn layout_key_path(layout_id: &str) -> String {
    format!(r"Layouts\{}", key_for(layout_id))
}

/// C# `RegistryLayoutStore.Read`: the global options, the layout document and the
/// models of the given PnP codes, from the tree under `root` (`None` when the root key
/// does not exist: nothing stored).
pub fn read<K: RegistryKey>(
    root: Option<&K>,
    layout_id: &str,
    pnp_codes: &[&str],
) -> LayoutStoreData {
    let mut data = LayoutStoreData::default();
    let Some(root) = root else { return data };

    let layout_key = root.open(&layout_key_path(layout_id));

    data.global_options = Some(read_global_options(root, layout_key.as_ref()));
    data.layout = layout_key.as_ref().map(read_layout);

    for pnp_code in pnp_codes {
        if let Some(model_key) = root.open(&format!(r"monitors\{pnp_code}")) {
            data.models
                .insert((*pnp_code).to_owned(), read_model(&model_key));
        }
    }
    data
}

/// C# `RegistryLayoutStore.ReadGlobalOptions`. Several options historically lived in
/// the layout key: `layout_key` is their fallback, so old configurations keep their
/// values (they reach the root key at the next save).
pub fn read_global_options<K: RegistryKey>(root: &K, layout_key: Option<&K>) -> GlobalOptionsDto {
    let legacy_string = |name| layout_key.and_then(|k| try_get_string(k, name));
    let legacy_bool = |name| layout_key.and_then(|k| try_get_bool(k, name));
    GlobalOptionsDto {
        priority: try_get_string(root, "Priority").or_else(|| legacy_string("Priority")),
        priority_unhooked: try_get_string(root, "PriorityUnhooked")
            .or_else(|| legacy_string("PriorityUnhooked")),
        home_cinema: try_get_bool(root, "HomeCinema").or_else(|| legacy_bool("HomeCinema")),
        pinned: try_get_bool(root, "Pinned").or_else(|| legacy_bool("Pinned")),
        auto_update: try_get_bool(root, "AutoUpdate").or_else(|| legacy_bool("AutoUpdate")),
        start_minimized: try_get_bool(root, "StartMinimized")
            .or_else(|| legacy_bool("StartMinimized")),
        start_elevated: try_get_bool(root, "StartElevated")
            .or_else(|| legacy_bool("StartElevated")),
        debug_tools: try_get_bool(root, "DebugTools"),
        experimental_features: try_get_bool(root, "ExperimentalFeatures"),
        vcp_control: try_get_bool(root, "VcpControl"),
        // "ShowAttachDetachWarning" is the former name of the option, read as fallback.
        show_monitor_action_warning: try_get_bool(root, "ShowMonitorActionWarning")
            .or_else(|| try_get_bool(root, "ShowAttachDetachWarning")),
        border_values: try_get_string(root, "BorderValues"),
        rescue_shortcut: try_get_string(root, "RescueShortcut"),
        hide_tray_icon: try_get_bool(root, "HideTrayIcon"),
        excluded_defaults_version: try_get_int(root, "ExcludedDefaultsVersion"),
        ..Default::default()
    }
}

/// C# `RegistryLayoutStore.ReadLayout`: a layout key's options and monitors.
pub fn read_layout<K: RegistryKey>(key: &K) -> LayoutDto {
    let mut dto = LayoutDto {
        options: Some(LayoutOptionsDto {
            allow_overlaps: try_get_bool(key, "AllowOverlaps"),
            allow_discontinuity: try_get_bool(key, "AllowDiscontinuity"),
            algorithm: try_get_string(key, "Algorithm"),
            minimal_edge_overlap: try_get(key, "MinimalEdgeOverlap"),
            max_travel_distance: try_get(key, "MaxTravelDistance"),
            freelook_check_interval: try_get(key, "FreelookCheckInterval"),
            freelook_enabled: try_get_bool(key, "FreelookEnabled"),
            loop_x: try_get_bool(key, "LoopX"),
            loop_y: try_get_bool(key, "LoopY"),
            enabled: try_get_bool(key, "Enabled"),
            adjust_pointer: try_get_bool(key, "AdjustPointer"),
            adjust_speed: try_get_bool(key, "AdjustSpeed"),
            priority: try_get_string(key, "Priority"),
            priority_unhooked: try_get_string(key, "PriorityUnhooked"),
            ..Default::default()
        }),
        ..Default::default()
    };

    let Some(monitors) = key.open("PhysicalMonitors") else {
        return dto;
    };
    for id in monitors.subkey_names() {
        if let Some(monitor_key) = monitors.open(&id) {
            dto.monitors.insert(id, read_monitor(&monitor_key));
        }
    }
    dto
}

/// C# `RegistryLayoutStore.ReadMonitor`: one monitor of a layout.
pub fn read_monitor<K: RegistryKey>(key: &K) -> MonitorDto {
    let mut dto = MonitorDto {
        x_location_in_mm: try_get(key, "XLocationInMm"),
        y_location_in_mm: try_get(key, "YLocationInMm"),
        physical_ratio_x: try_get(key, "PhysicalRatioX"),
        physical_ratio_y: try_get(key, "PhysicalRatioY"),
        border_resistance: read_border_resistance(key),
        // Presence of Borders\Left IS the "monitor owns its bezel borders" flag
        // (BordersCustomized) — a partial subkey without Left does not count.
        borders: try_get(key, r"Borders\Left").map(|left| BordersDto {
            left: Some(left),
            top: try_get(key, r"Borders\Top"),
            right: try_get(key, r"Borders\Right"),
            bottom: try_get(key, r"Borders\Bottom"),
            ..Default::default()
        }),
        active_source: try_get_string(key, "ActiveSource"),
        serial_number: try_get_string(key, "SerialNumber"),
        excluded_from_layout: try_get_bool(key, "ExcludedFromLayout"),
        sources: Some(Default::default()),
        ..Default::default()
    };

    // Sources are stored as sibling subkeys of the two border subkeys (compared as C#
    // does, case-sensitively).
    let sources = dto.sources.get_or_insert_with(Default::default);
    for id in key.subkey_names() {
        if id == "BorderResistance" || id == "Borders" {
            continue;
        }
        let Some(source_key) = key.open(&id) else {
            continue;
        };
        sources.insert(
            id,
            SourceDto {
                pixel_x: try_get(&source_key, "PixelX"),
                pixel_y: try_get(&source_key, "PixelY"),
                pixel_width: try_get(&source_key, "PixelWidth"),
                pixel_height: try_get(&source_key, "PixelHeight"),
                orientation: try_get_int(&source_key, "Orientation"),
                display_name: try_get_string(&source_key, "DisplayName"),
                primary: try_get_bool(&source_key, "Primary"),
                ..Default::default()
            },
        );
    }
    dto
}

/// C# `RegistryLayoutStore.ReadModel`: one monitor model (`monitors\<PnP code>`).
pub fn read_model<K: RegistryKey>(key: &K) -> ModelDto {
    ModelDto {
        width: try_get(key, r"Size\Width"),
        height: try_get(key, r"Size\Height"),
        borders: read_borders(key, "Borders"),
        pnp_name: try_get_string(key, "PnpName"),
        ..Default::default()
    }
}

fn read_borders<K: RegistryKey>(key: &K, name: &str) -> Option<BordersDto> {
    let sub = key.open(name)?;
    Some(BordersDto {
        left: try_get(&sub, "Left"),
        top: try_get(&sub, "Top"),
        right: try_get(&sub, "Right"),
        bottom: try_get(&sub, "Bottom"),
        ..Default::default()
    })
}

fn read_border_resistance<K: RegistryKey>(key: &K) -> Option<BorderResistanceDto> {
    let sub = key.open("BorderResistance")?;
    Some(BorderResistanceDto {
        left: read_side(&sub, "Left"),
        top: read_side(&sub, "Top"),
        right: read_side(&sub, "Right"),
        bottom: read_side(&sub, "Bottom"),
        ..Default::default()
    })
}

/// C# `RegistryLayoutStore.ReadSide`: one edge. Before the move/drag split each edge
/// was a single VALUE holding its resistance; it is now a SUBKEY. Reading the value
/// first is what migrates an existing installation — one number meant "resist any
/// crossing", so it maps to both modes (`border_side_json` is the JSON twin).
fn read_side<K: RegistryKey>(border_resistance: &K, name: &str) -> Option<BorderSideDto> {
    if let Some(legacy) = try_get(border_resistance, name) {
        return Some(BorderSideDto {
            r#move: Some(legacy),
            drag: Some(legacy),
            ..Default::default()
        });
    }

    let sub = border_resistance.open(name)?;
    Some(BorderSideDto {
        r#move: try_get(&sub, "Move"),
        move_block: try_get_bool(&sub, "MoveBlock"),
        drag: try_get(&sub, "Drag"),
        drag_block: try_get_bool(&sub, "DragBlock"),
        sections: read_sections(&sub),
        ..Default::default()
    })
}

/// C# `RegistryLayoutStore.ReadSections`. Subkeys are named by index; order matters for
/// nothing but stability, and the registry enumerates them alphabetically ("10" before
/// "2"), hence the (stable) sort, non-numbers last.
fn read_sections<K: RegistryKey>(side: &K) -> Option<Vec<BorderSectionDto>> {
    let container = side.open("Sections")?;
    let mut names = container.subkey_names();
    names.sort_by_key(|n| parse_int(n).unwrap_or(i32::MAX));

    let mut sections = Vec::new();
    for name in names {
        let Some(section) = container.open(&name) else {
            continue;
        };
        sections.push(BorderSectionDto {
            from: try_get(&section, "From"),
            to: try_get(&section, "To"),
            r#move: try_get(&section, "Move"),
            move_block: try_get_bool(&section, "MoveBlock"),
            drag: try_get(&section, "Drag"),
            drag_block: try_get_bool(&section, "DragBlock"),
            ..Default::default()
        });
    }
    Some(sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_parse_like_dotnet_float_invariant() {
        let accepted = [
            ("20", 20.0),
            ("-12", -12.0),
            ("+3", 3.0),
            ("320.5", 320.5),
            (" 1.5 ", 1.5),
            ("\t2\r\n", 2.0),
            ("1.", 1.0),
            (".5", 0.5),
            ("1E+17", 1e17),
            ("2.5e-5", 2.5e-5),
            ("1e400", f64::INFINITY),
            ("92.53889943074003", 92.53889943074003),
            ("7\0", 7.0),
            ("Infinity", f64::INFINITY),
            ("-infinity", f64::NEG_INFINITY),
            ("+Infinity", f64::INFINITY),
        ];
        for (s, expected) in accepted {
            assert_eq!(parse_double(s), Some(expected), "{s:?}");
        }
        assert!(parse_double("NaN").unwrap().is_nan());
        assert!(parse_double("-nan").unwrap().is_nan());
        assert!(parse_double("+NaN").unwrap().is_nan());

        // Refused: .NET's invariant culture has no thousands separator in Float, no
        // "inf", no comma decimal, no hexadecimal, nothing trailing.
        // (Checked against .NET 10, as the accepted ones.)
        for s in [
            "", " ", "1,5", "1 000", "inf", "0x10", "1e", "e5", ".", "-", "1.5x", "١", "∞",
            "\u{a0}1",
        ] {
            assert_eq!(parse_double(s), None, "{s:?}");
        }
    }

    #[test]
    fn ints_parse_like_dotnet_integer_invariant() {
        assert_eq!(parse_int("2"), Some(2));
        assert_eq!(parse_int(" -7 "), Some(-7));
        assert_eq!(parse_int("+0"), Some(0));
        assert_eq!(parse_int("2147483647"), Some(i32::MAX));
        assert_eq!(parse_int("7\0"), Some(7));
        for s in ["", "2147483648", "1.0", "1e3", "+", "0x1", "1_000"] {
            assert_eq!(parse_int(s), None, "{s:?}");
        }
    }
}
