//! The EDID parser against the C# one: `tests/data/edid-vectors.json` holds blocks of
//! every length up to 256 bytes and what `EdidParser.Parse` made of each, written by
//! `tests/data/edid-generator` (.NET 10). Every member must come out the same, doubles
//! bit for bit.

use lbm_display::edid::{parse, Edid};
use serde_json::Value;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn check(e: &Edid, expected: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut string = |name: &str, actual: &Option<String>| {
        let expected = expected[name].as_str();
        if actual.as_deref() != expected {
            out.push(format!("{name}: {actual:?}, expected {expected:?}"));
        }
    };
    string("ManufacturerCode", &e.manufacturer_code);
    string("ProductCode", &e.product_code);
    string("Serial", &e.serial);
    string("Version", &e.version);
    string("VideoInterface", &e.video_interface);
    string("Model", &e.model);
    string("SerialNumber", &e.serial_number);
    if expected["HKeyName"].as_str() != Some(e.key.as_str()) {
        out.push(format!("HKeyName: {:?}", e.key));
    }

    let ints = [
        ("Week", e.week),
        ("Year", e.year),
        ("BitDepth", e.bit_depth),
        ("Checksum", e.checksum),
    ];
    for (name, actual) in ints {
        if expected[name].as_i64() != Some(i64::from(actual)) {
            out.push(format!("{name}: {actual}, expected {}", expected[name]));
        }
    }

    let bools = [
        ("Digital", e.digital),
        ("DpmsStandbySupported", e.dpms_standby_supported),
        ("DpmsSuspendSupported", e.dpms_suspend_supported),
        ("DpmsActiveOffSupported", e.dpms_active_off_supported),
        ("YCrCb444Support", e.ycrcb444_support),
        ("YCrCb422Support", e.ycrcb422_support),
    ];
    for (name, actual) in bools {
        if expected[name].as_bool() != Some(actual) {
            out.push(format!("{name}: {actual}, expected {}", expected[name]));
        }
    }

    let doubles = [
        ("PhysicalWidth", e.physical_width),
        ("PhysicalHeight", e.physical_height),
        ("Gamma", e.gamma),
        ("RedX", e.red_x),
        ("RedY", e.red_y),
        ("GreenX", e.green_x),
        ("GreenY", e.green_y),
        ("BlueX", e.blue_x),
        ("BlueY", e.blue_y),
        ("WhiteX", e.white_x),
        ("WhiteY", e.white_y),
    ];
    for (name, actual) in doubles {
        if expected[name].as_f64().map(f64::to_bits) != Some(actual.to_bits()) {
            out.push(format!("{name}: {actual}, expected {}", expected[name]));
        }
    }
    out
}

#[test]
fn every_vector_parses_as_in_csharp() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/edid-vectors.json");
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let vectors: Vec<Value> = serde_json::from_str(&text).unwrap();
    assert!(vectors.len() > 400, "{} vectors", vectors.len());

    let mut failures = Vec::new();
    let mut compared = 0;
    for vector in &vectors {
        let bytes = hex(vector["Edid"].as_str().unwrap());
        let parsed = parse("KEY", &bytes);
        // Where C# throws, the only requirement is not to panic (see the module docs).
        let Some(expected) = vector.get("Parsed") else {
            continue;
        };
        compared += 1;
        failures.extend(
            check(&parsed, expected)
                .into_iter()
                .map(|f| format!("{} bytes {}: {f}", bytes.len(), vector["Edid"])),
        );
    }
    assert!(compared > 400);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Where C# throws (a descriptor running past the end of a short block), the string
/// stops at the end.
#[test]
fn a_descriptor_past_the_end_stops_there() {
    let mut bytes = vec![0u8; 70];
    bytes[54..59].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
    bytes[59..70].copy_from_slice(b"ABCDEFGHIJK");
    let e = parse("", &bytes);
    assert_eq!(e.model.as_deref(), Some("ABCDEFGHIJK"));
    assert_eq!(e.serial_number.as_deref(), Some(""));
}
