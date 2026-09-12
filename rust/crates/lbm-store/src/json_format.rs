//! The serializer settings of the JSON store — port of `JsonLayoutStore.JsonOptions`
//! and of the text handling around it in `JsonLayoutStore.ReadJson`/`WriteJson`.
//!
//! ```csharp
//! static readonly JsonSerializerOptions JsonOptions = new()
//! {
//!     WriteIndented = true,
//!     DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
//! };
//! ```
//!
//! Every other setting is the .NET 10 default. How each one is mirrored:
//!
//! | `System.Text.Json` | here |
//! |---|---|
//! | `WriteIndented = true`: two spaces, `"Name": value`, `{}`/`[]` when empty, no final newline | serde_json's `PrettyFormatter` with two spaces, which lays out identically |
//! | `NewLine` = `Environment.NewLine` | always `\n`: the only C# writer of these files is the Linux one |
//! | `DefaultIgnoreCondition = WhenWritingNull` | `skip_serializing_if = "Option::is_none"` on the DTOs |
//! | no naming policy, case-sensitive matching | `rename_all = "PascalCase"` over snake_case fields; serde matches case-sensitively |
//! | unmapped members skipped on read | kept in `extra` and written back — the one deliberate difference |
//! | `Encoder = null`: the default escaping | everything but printable ASCII, plus `"` `&` `'` `+` `<` `>` `\` and the backtick, as `\uXXXX` in uppercase hex, except the short `\b` `\t` `\n` `\f` `\r` `\\` |
//! | `double`: shortest round-trip digits, .NET "R" layout | [`format_double`] |
//! | `NumberHandling = Strict`: NaN and infinities throw on write | the DTOs refuse them the same way instead of writing `null` |
//! | `AllowTrailingCommas = false`, `ReadCommentHandling = Disallow` | serde_json rejects both as well |
//! | a repeated member: the last one wins | documents are parsed into a [`Value`] first, whose map keeps the last one |
//! | `File.ReadAllText`: byte-order-mark detection, invalid UTF-8 replaced by U+FFFD | the same before parsing |
//!
//! The result is byte-identical to what the C# store writes for the same document,
//! the committed `*-saved` fixtures included. The one reading difference left is the
//! nesting limit: 64 levels for `System.Text.Json`, 128 for serde_json — far beyond
//! what a store document holds.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::io;

use serde::de::DeserializeOwned;
use serde::ser::{Error as _, Serialize, Serializer};
use serde_json::ser::{CharEscape, Formatter, PrettyFormatter};
use serde_json::Value;

/// Parse a stored document — C# `JsonSerializer.Deserialize<T>(File.ReadAllText(path),
/// JsonOptions)`, the body of `JsonLayoutStore.ReadJson`, minus the swallowing: the
/// store turns an error into "absent", callers that want to know why get it here.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> serde_json::Result<T> {
    let text = decode_text(bytes);
    // Through a Value so that a repeated member resolves like System.Text.Json
    // (last one wins) instead of failing as "duplicate field".
    let document: Value = serde_json::from_str(&text)?;
    T::deserialize(document)
}

/// Write a document — C# `JsonSerializer.Serialize(value, JsonOptions)`, with the
/// C# serializer's layout, escaping and number format.
///
/// Fails on a NaN or infinite value in a DTO, as the C# serializer throws on one.
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<String> {
    let mut out = Vec::with_capacity(1024);
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut out, DotNetFormatter::default());
    value.serialize(&mut serializer)?;
    Ok(String::from_utf8(out).expect("the formatter escapes everything outside ASCII"))
}

/// A finite `f64` as .NET writes it (`double.ToString("R", InvariantCulture)`, the
/// format `System.Text.Json` uses): the shortest digits that round-trip, laid out by
/// `Number.Formatting.FormatGeneral` — plain notation while the decimal point sits
/// within 17 digits, or within the digits themselves (`10000000000000000`, `0.0001`),
/// scientific beyond (`1E+17`, `1E-05`, `1.7976931348623157E+308`), no `.0` on
/// integral values, `-0` for negative zero.
/// NaN and the infinities come out as .NET's invariant `NaN`, `Infinity`, `-Infinity`
/// (never in a document: the writer refuses them).
pub fn format_double(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.into();
    }

    // Rust's `{:e}` yields the shortest round-trip digits, as .NET does, save for the
    // tie-break; the layout is redone below.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').expect("`{:e}` has an exponent");
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().expect("`{:e}` writes a decimal exponent");
    break_tie_to_even(value.abs(), &mut digits, exponent);
    let count = digits.len() as i32;
    // .NET's NumberBuffer.Scale: the decimal point sits after `scale` digits.
    let scale = exponent + 1;

    let mut out = String::with_capacity(digits.len() + 8);
    if value < 0.0 {
        out.push('-');
    }
    // FormatGeneral with nMaxDigits = max(digit count, 17), what .NET passes for the
    // shortest round-trip form (double's MaxRoundTripDigits).
    if scale > count.max(17) || scale < -3 {
        out.push_str(&digits[..1]);
        if count > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let exponent = scale - 1;
        let sign = if exponent < 0 { '-' } else { '+' };
        let _ = write!(out, "E{sign}{:02}", exponent.unsigned_abs());
    } else if scale <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', scale.unsigned_abs() as usize));
        out.push_str(&digits);
    } else if count <= scale {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n('0', (scale - count) as usize));
    } else {
        let (integral, fractional) = digits.split_at(scale as usize);
        out.push_str(integral);
        out.push('.');
        out.push_str(fractional);
    }
    out
}

/// When the value lies exactly half-way between two shortest candidates, Rust's
/// shortest formatting takes the upper one and .NET's Dragon4 the even one ("round
/// towards the even digit", as long as it round-trips too). Both parse back to the
/// same double; this only makes the digits .NET's. Ties need an exact expansion one
/// digit longer than the shortest form, which in practice means 17-digit magnitudes
/// with a short binary fraction (`2049285864339844.25` → `…844.2`, not `…844.3`).
///
/// `positive` is the absolute value, `digits` its shortest digits, the first one
/// weighing `10^exponent`.
fn break_tie_to_even(positive: f64, digits: &mut String, exponent: i32) {
    // A tie puts two n-digit candidates, one unit of the n-th digit apart (over 10^-n
    // of the value), in the double's rounding interval (one ulp, at most 2^-52 of it):
    // n > 52·log10(2) ≈ 15.65. (A subnormal's exact expansion runs to hundreds of
    // digits, never a tie.) Skipping the shorter forms spares the expansion below.
    if digits.len() < 16 {
        return;
    }
    let last = digits.as_bytes()[digits.len() - 1];
    if (last - b'0').is_multiple_of(2) {
        return;
    }
    let mut lower = digits.clone();
    lower.pop();
    lower.push(char::from(last - 1));

    // The exact decimal expansion: a double never has more than 767 significant digits.
    let exact = format!("{positive:.800e}");
    let (mantissa, exact_exponent) = exact.split_once('e').expect("`{:e}` has an exponent");
    let exact_digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let tie = exact_exponent.parse() == Ok(exponent)
        && exact_digits
            .trim_end_matches('0')
            .strip_prefix(lower.as_str())
            == Some("5");
    if !tie {
        return;
    }

    let candidate = format!("{}.{}e{exponent}", &lower[..1], &lower[1..]);
    if candidate.parse::<f64>() == Ok(positive) {
        *digits = lower.trim_end_matches('0').to_owned();
    }
}

/// `System.Text.Json`'s output: the indented layout of serde_json's pretty printer,
/// with the C# escaping and number format.
struct DotNetFormatter {
    layout: PrettyFormatter<'static>,
}

impl Default for DotNetFormatter {
    fn default() -> Self {
        DotNetFormatter {
            layout: PrettyFormatter::with_indent(b"  "),
        }
    }
}

impl Formatter for DotNetFormatter {
    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.begin_array(writer)
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.layout.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.begin_object(writer)
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.layout.begin_object_key(writer, first)
    }

    fn end_object_key<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.end_object_key(writer)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.layout.end_object_value(writer)
    }

    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(format_double(value).as_bytes())
    }

    /// serde_json hands over the runs it would not escape itself; the C# encoder
    /// escapes more of them.
    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut start = 0;
        for (index, c) in fragment.char_indices() {
            if needs_escaping(c) {
                writer.write_all(&fragment.as_bytes()[start..index])?;
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    write!(writer, "\\u{unit:04X}")?;
                }
                start = index + c.len_utf8();
            }
        }
        writer.write_all(&fragment.as_bytes()[start..])
    }

    /// The characters serde_json escapes itself, in the C# encoder's spelling.
    fn write_char_escape<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        char_escape: CharEscape,
    ) -> io::Result<()> {
        let escaped: &[u8] = match char_escape {
            CharEscape::Quote => br"\u0022",
            CharEscape::ReverseSolidus => br"\\",
            // Never produced by serde_json's serializer, and not escaped by C#.
            CharEscape::Solidus => b"/",
            CharEscape::Backspace => br"\b",
            CharEscape::FormFeed => br"\f",
            CharEscape::LineFeed => br"\n",
            CharEscape::CarriageReturn => br"\r",
            CharEscape::Tab => br"\t",
            CharEscape::AsciiControl(byte) => return write!(writer, "\\u{byte:04X}"),
        };
        writer.write_all(escaped)
    }
}

/// Outside `System.Text.Json`'s default allow list: printable ASCII minus the
/// HTML-sensitive characters, the quote and the backslash.
fn needs_escaping(c: char) -> bool {
    !matches!(c, ' '..='~') || matches!(c, '"' | '&' | '\'' | '+' | '<' | '>' | '\\' | '`')
}

/// A file's text as .NET's `File.ReadAllText`/`File.ReadAllLines` decode it: a
/// byte-order mark selects UTF-8, UTF-16 or UTF-32 and is dropped, anything else is
/// UTF-8, and invalid sequences become U+FFFD instead of failing.
pub(crate) fn decode_text(bytes: &[u8]) -> Cow<'_, str> {
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest),
        [0xFF, 0xFE, 0x00, 0x00, rest @ ..] => Cow::Owned(decode_utf32(rest, u32::from_le_bytes)),
        [0x00, 0x00, 0xFE, 0xFF, rest @ ..] => Cow::Owned(decode_utf32(rest, u32::from_be_bytes)),
        [0xFF, 0xFE, rest @ ..] => Cow::Owned(decode_utf16(rest, u16::from_le_bytes)),
        [0xFE, 0xFF, rest @ ..] => Cow::Owned(decode_utf16(rest, u16::from_be_bytes)),
        _ => String::from_utf8_lossy(bytes),
    }
}

fn decode_utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let (units, rest) = bytes.as_chunks::<2>();
    let mut text: String = char::decode_utf16(units.iter().map(|&u| unit(u)))
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    if !rest.is_empty() {
        text.push(char::REPLACEMENT_CHARACTER);
    }
    text
}

fn decode_utf32(bytes: &[u8], unit: fn([u8; 4]) -> u32) -> String {
    let (units, rest) = bytes.as_chunks::<4>();
    let mut text: String = units
        .iter()
        .map(|&u| char::from_u32(unit(u)).unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    if !rest.is_empty() {
        text.push(char::REPLACEMENT_CHARACTER);
    }
    text
}

/// A number the C# serializer accepts: it throws on NaN and the infinities
/// (`JsonNumberHandling.Strict`), where serde_json would quietly write `null`.
pub(crate) struct Finite(pub(crate) f64);

impl Serialize for Finite {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_finite() {
            serializer.serialize_f64(self.0)
        } else {
            Err(S::Error::custom(format_args!(
                "{} is not a valid JSON number",
                format_double(self.0)
            )))
        }
    }
}

/// `serialize_with` for the DTOs' `Option<f64>` members; see [`Finite`].
pub(crate) fn finite<S: Serializer>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&Finite(*value)),
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::layout_dtos::{GlobalOptionsDto, LayoutDto, MonitorDto};

    /// Every finite expectation was printed by `JsonSerializer.Serialize(double)` on
    /// .NET 10, the others by `double.ToString(CultureInfo.InvariantCulture)`.
    #[test]
    fn doubles_are_written_like_dotnet() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "-0"),
            (1.0, "1"),
            (-12.0, "-12"),
            (320.5, "320.5"),
            (0.1, "0.1"),
            (0.5, "0.5"),
            (1.0 / 3.0, "0.3333333333333333"),
            (0.1 + 0.2, "0.30000000000000004"),
            (0.0001, "0.0001"),
            (0.00012, "0.00012"),
            (0.000123456, "0.000123456"),
            (0.00001, "1E-05"),
            (2.5e-5, "2.5E-05"),
            (1.5e-7, "1.5E-07"),
            (100.0, "100"),
            (12345678.9, "12345678.9"),
            (1e14, "100000000000000"),
            (123456789012345.0, "123456789012345"),
            (1e15, "1000000000000000"),
            (1.5e15, "1500000000000000"),
            (9007199254740992.0, "9007199254740992"),
            (1e16, "10000000000000000"),
            (1234567890123456789.0, "1.2345678901234568E+18"),
            (1e21, "1E+21"),
            // Exact ties between two shortest candidates: .NET takes the even one.
            (2049285864339844.0 + 0.25, "2049285864339844.2"),
            (-(196972381475249.0 + 0.625), "-196972381475249.62"),
            (1249.75, "1249.75"),
            (f64::MAX, "1.7976931348623157E+308"),
            (f64::MIN_POSITIVE, "2.2250738585072014E-308"),
            (5e-324, "5E-324"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for &(value, expected) in cases {
            assert_eq!(format_double(value), expected, "{value:e}");
            if value.is_finite() {
                assert_eq!(expected.parse::<f64>().unwrap().to_bits(), value.to_bits());
            }
        }
    }

    #[test]
    fn strings_are_escaped_like_the_default_csharp_encoder() {
        let text = "Ctrl+Alt \"q\" \\x/<>&'`\u{7f}\u{1}\n\r\t\u{8}\u{c}é€😀~";
        assert_eq!(
            to_string(text).unwrap(),
            r#""Ctrl\u002BAlt \u0022q\u0022 \\x/\u003C\u003E\u0026\u0027\u0060\u007F\u0001\n\r\t\b\f\u00E9\u20AC\uD83D\uDE00~""#
        );
    }

    #[test]
    fn property_names_are_escaped_too() {
        let mut dto = LayoutDto::default();
        dto.monitors.insert("A+B".into(), MonitorDto::default());
        assert_eq!(
            to_string(&dto).unwrap(),
            "{\n  \"Monitors\": {\n    \"A\\u002BB\": {}\n  }\n}"
        );
    }

    #[test]
    fn layout_is_the_indented_csharp_one() {
        let value = json!({ "A": [], "B": {}, "C": [{ "D": 1 }, 2.5], "E": null });
        assert_eq!(
            to_string(&value).unwrap(),
            "{\n  \"A\": [],\n  \"B\": {},\n  \"C\": [\n    {\n      \"D\": 1\n    },\n    2.5\n  ],\n  \"E\": null\n}"
        );
    }

    #[test]
    fn a_repeated_member_keeps_the_last_value() {
        let dto: GlobalOptionsDto =
            from_slice(br#"{ "Pinned": true, "Future": 1, "Pinned": false, "Future": 2 }"#)
                .unwrap();
        assert_eq!(dto.pinned, Some(false));
        assert_eq!(
            dto.extra,
            json!({ "Future": 2 }).as_object().unwrap().clone()
        );
    }

    #[test]
    fn byte_order_marks_are_honoured() {
        let json = r#"{ "Priority": "Hé" }"#;
        let utf16le: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(json.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let utf16be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain(json.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        let utf32le: Vec<u8> = [0xFF, 0xFE, 0, 0]
            .into_iter()
            .chain(json.chars().flat_map(|c| u32::from(c).to_le_bytes()))
            .collect();
        let utf8 = [&[0xEF, 0xBB, 0xBF][..], json.as_bytes()].concat();
        for bytes in [utf8, utf16le, utf16be, utf32le] {
            let dto: GlobalOptionsDto = from_slice(&bytes).unwrap();
            assert_eq!(dto.priority.as_deref(), Some("Hé"));
        }
    }

    #[test]
    fn doubles_are_read_correctly_rounded() {
        // System.Text.Json reads the nearest double; serde_json's default
        // parser can land one ulp off, and a re-save would then change the
        // stored digits (this one came back as ...74005).
        let dto: MonitorDto = from_slice(br#"{ "XLocationInMm": 92.53889943074003 }"#).unwrap();
        assert_eq!(dto.x_location_in_mm, Some(92.53889943074003));
        assert_eq!(
            format_double(dto.x_location_in_mm.unwrap()),
            "92.53889943074003"
        );
    }

    #[test]
    fn invalid_utf8_is_replaced_not_fatal() {
        let dto: GlobalOptionsDto = from_slice(b"{ \"Priority\": \"a\xFFb\" }").unwrap();
        assert_eq!(dto.priority.as_deref(), Some("a\u{FFFD}b"));
    }

    #[test]
    fn comments_and_trailing_commas_are_refused() {
        assert!(from_slice::<GlobalOptionsDto>(b"{ \"Pinned\": true, }").is_err());
        assert!(from_slice::<GlobalOptionsDto>(b"{ /* x */ \"Pinned\": true }").is_err());
    }

    #[test]
    fn an_integer_member_refuses_a_fraction() {
        assert!(from_slice::<GlobalOptionsDto>(br#"{ "ExcludedDefaultsVersion": 1.0 }"#).is_err());
    }

    #[test]
    fn non_finite_numbers_fail_the_write() {
        let dto = MonitorDto {
            x_location_in_mm: Some(f64::NAN),
            ..MonitorDto::default()
        };
        assert!(to_string(&dto).is_err());
    }
}
