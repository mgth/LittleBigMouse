//! The monitor ids, from `MonitorDeviceHelper`: `GetPnpCodeFromId`, `GetPhysicalId`,
//! `GetSourceId`, and the duplicate scan of `GetDisplayDevices`. The source id is the
//! key a user's layouts are stored under: every character here is C#'s.

use crate::edid::Edid;

/// C# `GetPnpCodeFromId`: the device id's second `\`-separated segment
/// (`MONITOR\DEL4065\{...}\0003` gives `DEL4065`), the whole id when it has one.
pub fn pnp_code_from_id(device_id: &str) -> &str {
    let mut segments = device_id.split('\\');
    let first = segments.next().unwrap_or("");
    segments.next().unwrap_or(first)
}

/// `deviceId.Split('\\').Last()`: the device instance (`0003`).
pub fn last_segment(device_id: &str) -> &str {
    device_id.rsplit('\\').next().unwrap_or("")
}

/// C# `GetPhysicalId`: `{pnp}{serial string}_{week:X2}_{year:X4}` with an EDID,
/// `NOEDID_{pnp}_{instance}` without. The enumeration sorts monitors by it.
pub fn physical_id(device_id: &str, edid: Option<&Edid>) -> String {
    let pnp_code = pnp_code_from_id(device_id);
    match edid {
        None => format!("NOEDID_{pnp_code}_{}", last_segment(device_id)),
        Some(e) => format!(
            "{pnp_code}{}_{:02X}_{:04X}",
            e.serial_number.as_deref().unwrap_or(""),
            e.week,
            e.year
        ),
    }
}

/// C# `GetSourceId`: the physical id and `_{checksum:X2}` with an EDID, the same
/// `NOEDID_{pnp}_{instance}` without. Before [`disambiguate_source_ids`], two
/// monitors of a model reporting no serial string, made the same week, share it.
pub fn source_id(device_id: &str, edid: Option<&Edid>) -> String {
    let pnp_code = pnp_code_from_id(device_id);
    match edid {
        None => format!("NOEDID_{pnp_code}_{}", last_segment(device_id)),
        Some(e) => format!(
            "{pnp_code}{}_{:02X}_{:04X}_{:02X}",
            e.serial_number.as_deref().unwrap_or(""),
            e.week,
            e.year,
            e.checksum
        ),
    }
}

/// The duplicate scan of `GetDisplayDevices`, over the monitors in the enumeration's
/// order (by physical id), `device_ids[i]` and `source_ids[i]` being monitor `i`'s: a
/// source id equal to the one before gets `_{instance}` appended, and so does the one
/// before when it still has it. Only neighbours are compared, the way C# does: `A`,
/// `B`, `A` keep their ids, and the layout then takes the second `A` for another
/// source of the first (see `lbm_layout::windows::add_or_update_monitor_device`).
pub fn disambiguate_source_ids(device_ids: &[&str], source_ids: &mut [String]) {
    let mut last_source_id = String::new();
    for i in 0..source_ids.len().min(device_ids.len()) {
        if i > 0 && source_ids[i] == last_source_id {
            if source_ids[i - 1] == last_source_id {
                source_ids[i - 1] = format!("{last_source_id}_{}", last_segment(device_ids[i - 1]));
            }
            // Always the case here; C# tests it all the same.
            if source_ids[i] == last_source_id {
                source_ids[i] = format!("{last_source_id}_{}", last_segment(device_ids[i]));
            }
        } else {
            last_source_id = source_ids[i].clone();
        }
    }
}
