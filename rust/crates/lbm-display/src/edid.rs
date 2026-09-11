//! EDID parsing — port of `HLab.Sys.Monitors.Edid/Edid.cs` (`EdidParser.Parse`).
//!
//! Everything that goes into a monitor id must come out bit for bit as in C#: the
//! manufacturer letters, the product code, the binary serial, the descriptor strings,
//! and on Windows the week, the year and the checksum. So the C# parser is followed
//! step by step, stopping where it stops on a short block, and its quirks are kept:
//! the physical size first read in centimetres from the basic block, then replaced by
//! the first detailed timing's millimetres; descriptor strings read as Latin-1 up to a
//! line feed or a NUL.
//!
//! One deliberate difference: on a block of 69 to 125 bytes whose descriptor runs past
//! the end, C# throws `IndexOutOfRangeException`; here the string stops at the end.
//! The sysfs reader only accepts 128 bytes or more, where this cannot happen.

use lbm_layout::linux::LinuxEdid;

/// A parsed EDID block: C#'s `Edid`. `None` strings are members C# leaves null
/// because the block stops before them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edid {
    /// C# `HKeyName`: where the block was read from (a registry key, a sysfs path).
    pub key: String,
    /// Three letters, from bytes 8 and 9.
    pub manufacturer_code: Option<String>,
    /// Four upper-case hex digits, from bytes 10 and 11 (little-endian).
    pub product_code: Option<String>,
    /// The binary serial, bytes 15 down to 12 as eight upper-case hex digits.
    pub serial: Option<String>,
    pub week: i32,
    pub year: i32,
    /// `"<major>.<minor>"`.
    pub version: Option<String>,
    pub digital: bool,
    pub bit_depth: i32,
    pub video_interface: Option<String>,
    /// In mm: the basic block's centimetres times ten, then the first detailed
    /// timing's image size when the block reaches it.
    pub physical_width: f64,
    pub physical_height: f64,
    pub gamma: f64,
    pub dpms_standby_supported: bool,
    pub dpms_suspend_supported: bool,
    pub dpms_active_off_supported: bool,
    pub ycrcb444_support: bool,
    pub ycrcb422_support: bool,
    pub red_x: f64,
    pub red_y: f64,
    pub green_x: f64,
    pub green_y: f64,
    pub blue_x: f64,
    pub blue_y: f64,
    pub white_x: f64,
    pub white_y: f64,
    /// The 0xFC descriptor (monitor name), `""` when absent.
    pub model: Option<String>,
    /// The 0xFF descriptor (serial number string), `""` when absent.
    pub serial_number: Option<String>,
    pub checksum: i32,
}

/// C# `EdidParser.Parse(key, edid)`.
pub fn parse(key: impl Into<String>, edid: &[u8]) -> Edid {
    let mut e = Edid {
        key: key.into(),
        ..Default::default()
    };
    let len = edid.len();
    let b = |i: usize| i32::from(edid[i]);

    if len <= 9 {
        return e;
    }
    let letter = |v: i32| char::from(64 + (v & 0x1F) as u8);
    e.manufacturer_code = Some(
        [
            letter(b(8) >> 2),
            letter((b(8) << 3) | (b(9) >> 5)),
            letter(b(9)),
        ]
        .iter()
        .collect(),
    );

    if len <= 11 {
        return e;
    }
    e.product_code = Some(format!("{:04X}", b(10) + (b(11) << 8)));

    if len <= 15 {
        return e;
    }
    e.serial = Some(format!(
        "{:02X}{:02X}{:02X}{:02X}",
        edid[15], edid[14], edid[13], edid[12]
    ));

    if len <= 16 {
        return e;
    }
    e.week = b(16);

    if len < 18 {
        return e;
    }
    e.year = b(17) + 1990;

    if len <= 19 {
        return e;
    }
    e.version = Some(format!("{}.{}", edid[18], edid[19]));

    if len <= 20 {
        return e;
    }
    e.digital = (b(20) >> 7) == 1;
    if e.digital {
        e.bit_depth = 4 + ((b(20) & 0b0111_0000) >> 3);
        e.video_interface = Some(match b(20) & 0b1111 {
            0 => "undefined".to_owned(),
            2 => "HDMIa".to_owned(),
            3 => "HDMIb".to_owned(),
            4 => "MDDI".to_owned(),
            5 => "DisplayPort".to_owned(),
            other => format!("{other:X}"),
        });
    }

    if len <= 21 {
        return e;
    }
    e.physical_width = f64::from(b(21) * 10);

    if len <= 22 {
        return e;
    }
    e.physical_height = f64::from(b(22) * 10);

    if len <= 23 {
        return e;
    }
    if b(23) < 255 {
        e.gamma = 1.0 + f64::from(b(23)) / 100.0;
    }

    if len <= 24 {
        return e;
    }
    e.dpms_standby_supported = (b(24) & (1 << 7)) > 0;
    e.dpms_suspend_supported = (b(24) & (1 << 6)) > 0;
    e.dpms_active_off_supported = (b(24) & (1 << 5)) > 0;
    if e.digital {
        e.ycrcb422_support = (b(24) & (1 << 4)) > 0;
        e.ycrcb444_support = (b(24) & (1 << 3)) > 0;
    }

    if len <= 34 {
        return e;
    }
    if (b(24) & (1 << 2)) > 0 {
        let low = |byte: usize, shift: u32| (b(byte) >> shift) & 0b11;
        let chroma = |low: i32, high: usize| f64::from(low | (b(high) << 2)) / 1024.0;
        e.red_x = chroma(low(25, 6), 27);
        e.red_y = chroma(low(25, 4), 28);
        e.green_x = chroma(low(25, 2), 29);
        e.green_y = chroma(low(25, 0), 30);
        e.blue_x = chroma(low(26, 6), 31);
        e.blue_y = chroma(low(26, 4), 32);
        e.white_x = chroma(low(26, 2), 33);
        e.white_y = chroma(low(26, 0), 34);
    }

    if len <= 68 {
        return e;
    }
    e.physical_width = f64::from(((b(68) & 0xF0) << 4) + b(66));
    e.physical_height = f64::from(((b(68) & 0x0F) << 8) + b(67));

    e.model = Some(block(0xFC, edid));
    e.serial_number = Some(block(0xFF, edid));

    if len <= 127 {
        return e;
    }
    e.checksum = b(127);
    e
}

/// C# `EdidParser.Block`: the text of the first display descriptor tagged `code`
/// (`00 00 00 <code>`), in the four 18-byte slots from byte 54, as Latin-1 up to a
/// line feed or a NUL; `""` when there is none.
fn block(code: u8, edid: &[u8]) -> String {
    for i in (54..=108).step_by(18) {
        if i >= edid.len() || edid.get(i..i + 4) != Some(&[0, 0, 0, code][..]) {
            continue;
        }
        return edid[(i + 5).min(edid.len())..(i + 18).min(edid.len())]
            .iter()
            .take_while(|&&c| c != 0x0A && c != 0x00)
            .map(|&c| char::from(c))
            .collect();
    }
    String::new()
}

impl Edid {
    /// The members the Linux mapping reads (`LinuxLayoutMapping.AddMonitor`).
    pub fn to_linux(&self) -> LinuxEdid {
        LinuxEdid {
            manufacturer_code: self.manufacturer_code.clone(),
            product_code: self.product_code.clone(),
            serial: self.serial.clone(),
            serial_number: self.serial_number.clone(),
            model: self.model.clone(),
            physical_width: self.physical_width,
            physical_height: self.physical_height,
            video_interface: self.video_interface.clone(),
        }
    }
}
