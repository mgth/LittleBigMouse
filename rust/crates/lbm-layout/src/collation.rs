//! .NET's `StringComparer.InvariantCulture`, for the strings the domain sorts.
//!
//! The C# domain sorts two kinds of keys with the culture-sensitive default
//! comparer, under the invariant culture: monitor ids when it builds a layout
//! id (`LayoutIdExtensions.ComputeId`, the id is the store key), and source
//! device ids when it orders zones (`MonitorsLayout.PhysicalSources`). An
//! ordinal sort gets both wrong — `eDP-1` sorts before `HDMI-A-1` for .NET — and
//! a wrong layout id means a user's saved layout is not found.
//!
//! .NET 5+ compares with ICU's root collation. For ASCII that is the DUCET
//! order with punctuation non-ignorable: every character has a primary weight
//! (whitespace, then punctuation and symbols in the order below, then digits,
//! then letters regardless of case), the primary keys of the two strings are
//! compared first, and case only breaks ties afterwards, lowercase first. Other
//! C0 controls and DEL are ignorable. This module implements exactly that and
//! is checked against 3,000 comparisons and a 419-string sort made by .NET 10
//! itself (`tests/data/invariant-collation.json`, produced by the program next
//! to it; inferred and first verified on 127,571 pairs).
//!
//! Non-ASCII characters are outside that verification: they sort after every
//! ASCII character, by code point. Monitor ids and connector names are ASCII
//! (EDID strings are printable ASCII by specification).

use std::cmp::Ordering;

/// The primary order of the printable ASCII characters that are not letters or
/// digits, and of the whitespace controls, as ICU's root collation ranks them.
const PUNCTUATION_ORDER: &str = "\t\n\u{0b}\u{0c}\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

/// Primary weight: position in the whitespace-punctuation-digits-letters order,
/// letters folded to one case. `None` for an ignorable character.
fn primary(c: char) -> Option<u32> {
    if let Some(i) = PUNCTUATION_ORDER.find(c) {
        return Some(i as u32);
    }
    let base = PUNCTUATION_ORDER.len() as u32;
    match c {
        '0'..='9' => Some(base + (c as u32 - '0' as u32)),
        'a'..='z' => Some(base + 10 + (c as u32 - 'a' as u32)),
        'A'..='Z' => Some(base + 10 + (c as u32 - 'A' as u32)),
        '\u{00}'..='\u{1f}' | '\u{7f}' => None,
        // Outside the verified range: after all of ASCII, by code point.
        _ => Some(base + 36 + c as u32),
    }
}

/// Tertiary (case) weight: lowercase and caseless before uppercase.
fn tertiary(c: char) -> u32 {
    u32::from(c.is_ascii_uppercase())
}

/// `StringComparer.InvariantCulture.Compare(a, b)` for the strings above.
pub fn invariant_compare(a: &str, b: &str) -> Ordering {
    let primaries = |s: &str| s.chars().filter_map(primary).collect::<Vec<_>>();
    match primaries(a).cmp(&primaries(b)) {
        Ordering::Equal => {}
        other => return other,
    }
    let tertiaries = |s: &str| {
        s.chars()
            .filter(|&c| primary(c).is_some())
            .map(tertiary)
            .collect::<Vec<_>>()
    };
    tertiaries(a).cmp(&tertiaries(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_only_breaks_ties() {
        assert_eq!(invariant_compare("a", "A"), Ordering::Less);
        assert_eq!(invariant_compare("ab", "Aa"), Ordering::Greater);
        assert_eq!(invariant_compare("eDP-1", "HDMI-A-1"), Ordering::Less);
        assert_eq!(invariant_compare("DP-1", "dp-2"), Ordering::Less);
    }

    #[test]
    fn punctuation_sorts_before_digits_before_letters() {
        assert_eq!(invariant_compare("_", "-"), Ordering::Less);
        assert_eq!(invariant_compare("-", "0"), Ordering::Less);
        assert_eq!(invariant_compare("9", "a"), Ordering::Less);
        // The separators of monitor ids: `_` before `@`.
        assert_eq!(invariant_compare("PHL0927_1", "PHL0927@1"), Ordering::Less);
    }
}
