//! The name a layout is stored under — port of `Persistence/LayoutStoreKey.cs`.
//!
//! A layout id is the "+"-joined list of its monitor ids (`LayoutIdExtensions.ComputeId`):
//! 26 to 31 characters per monitor on Windows, more on Linux. Both backends cap a name —
//! Windows refuses a registry key name over 255 characters, the common Linux filesystems
//! a file name over 255 bytes — and nine monitors are enough to go over. The registry
//! then threw at the first read and the UI never came up, which read as an "8-monitor
//! limit" (#589).
//!
//! An id that fits is stored as-is, so every existing configuration keeps its key. A
//! longer one keeps a readable head and ends with the SHA-256 of the whole id: two setups
//! sharing their first monitors still get distinct entries, and one setup always maps to
//! the same.
//!
//! Lengths are counted like C# `string.Length`, in UTF-16 code units, not in bytes or
//! characters: ids are ASCII in practice, and the key must be the one C# computes.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

/// C# `LayoutStoreKey.MaxLength`: longest name the backends accept, registry key names
/// and file names alike.
pub const MAX_LENGTH: usize = 255;

/// C# `LayoutStoreKey.Separator`: between the readable head and the digest; never part
/// of an id.
const SEPARATOR: char = '~';

/// Length of the upper-case hex SHA-256 digest.
const DIGEST_LENGTH: usize = 64;

/// C# `LayoutStoreKey.For(layoutId)`: the store name of a layout, under the default
/// [`MAX_LENGTH`] cap.
pub fn key_for(layout_id: &str) -> String {
    key_for_capped(layout_id, MAX_LENGTH).expect("the default cap leaves room for the digest")
}

/// C# `LayoutStoreKey.For(layoutId, maxLength)`: the store name of a layout under the
/// backend's cap. A file store passes the cap minus its extension.
///
/// Refused (C# `ArgumentOutOfRangeException`) when an id that does not fit meets a cap
/// too short to hold the digest and its separator.
pub fn key_for_capped(layout_id: &str, max_length: usize) -> Result<String, CapTooShort> {
    let units: Vec<u16> = layout_id.encode_utf16().collect();
    if units.len() <= max_length {
        return Ok(layout_id.to_owned());
    }

    let digest: String = Sha256::digest(layout_id.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();

    let mut head = max_length
        .checked_sub(DIGEST_LENGTH + 1)
        .ok_or(CapTooShort { max_length })?;

    // Ids are ASCII in practice; never cut a surrogate pair should one show up.
    if head > 0 && is_high_surrogate(units[head - 1]) {
        head -= 1;
    }

    let head = String::from_utf16(&units[..head]).expect("the head ends on a scalar boundary");
    Ok(format!("{head}{SEPARATOR}{digest}"))
}

fn is_high_surrogate(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

/// The C# `ArgumentOutOfRangeException` of `LayoutStoreKey.For`: the cap cannot hold
/// the digest and its separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapTooShort {
    /// The cap that was asked for.
    pub max_length: usize,
}

impl fmt::Display for CapTooShort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "A store key needs room for the {DIGEST_LENGTH}-character digest and its separator (cap: {}).",
            self.max_length
        )
    }
}

impl Error for CapTooShort {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_is_the_upper_case_sha256_of_the_utf8_id() {
        // SHA-256 of a million 'a', the FIPS 180-2 test vector; a cap of 65 leaves no
        // room for a head.
        let key = key_for_capped(&"a".repeat(1_000_000), 65).unwrap();
        assert_eq!(
            key,
            "~CDC76E5C9914FB9281A1C7E284D73E67F1809A48A497200E046D39CCC7112CD0"
        );
    }

    #[test]
    fn length_is_counted_in_utf16_units_and_pairs_are_never_split() {
        // "😀" is two UTF-16 units (four UTF-8 bytes); a head of three units would end
        // between them, so it gives up the whole character.
        let id = format!("ab😀{}", "x".repeat(80));
        let key = key_for_capped(&id, DIGEST_LENGTH + 1 + 3).unwrap();
        assert!(key.starts_with("ab~"), "{key}");

        // Four characters but five units: it fits a cap of five, not of four.
        assert_eq!(key_for_capped("abc😀", 5).unwrap(), "abc😀");
        assert_eq!(
            key_for_capped("abc😀", 4),
            Err(CapTooShort { max_length: 4 })
        );
    }
}
