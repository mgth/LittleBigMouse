//! Port of `LayoutStoreKeyTests.cs`, one test per C# test, same names.
//!
//! The store name of a layout (#589). Nine monitor ids join into a layout id longer
//! than a registry key name may be; the read threw and the UI never booted, which read
//! as an "8-monitor limit". A short id must keep its historical key; a long one must
//! fit, stay stable, and stay distinct from its neighbours.

mod common;

use common::{layout_id, monitor_id};
use lbm_store::layout_store_key::{key_for, key_for_capped, CapTooShort, MAX_LENGTH};

#[test]
fn eight_monitors_keep_their_historical_key() {
    let id = layout_id(8);

    assert!(id.len() <= MAX_LENGTH);
    assert_eq!(key_for(&id), id);
}

#[test]
fn nine_monitors_fit_the_limit() {
    // The reporter's configuration: the ninth monitor is the one that went over.
    let id = layout_id(9);
    assert!(id.len() > MAX_LENGTH);

    assert!(key_for(&id).len() <= MAX_LENGTH);
}

#[test]
fn long_id_keeps_a_readable_head() {
    let head = format!("{}+{}", monitor_id(1), monitor_id(2));
    assert!(key_for(&layout_id(9)).starts_with(&head));
}

#[test]
fn long_id_is_stable() {
    assert_eq!(key_for(&layout_id(9)), key_for(&layout_id(9)));
}

#[test]
fn long_ids_sharing_their_head_get_distinct_keys() {
    let a = layout_id(9);
    // Differs in its last character only, far past the readable head.
    let b = format!("{}F", &a[..a.len() - 1]);

    assert_ne!(key_for(&a), key_for(&b));
}

#[test]
fn key_uses_only_store_safe_characters() {
    // C#: Assert.Matches("^[A-Za-z0-9+_~]+$", ...)
    let key = key_for(&layout_id(12));
    assert!(!key.is_empty());
    assert!(
        key.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '_' | '~')),
        "{key}"
    );
}

#[test]
fn shorter_cap_is_honored() {
    // What the JSON store passes: the file name cap minus its extension.
    let key = key_for_capped(&layout_id(9), 250).unwrap();

    assert!(key.len() <= 250);
    assert_ne!(key_for(&layout_id(9)), key);
}

#[test]
fn cap_with_no_room_for_the_digest_is_refused() {
    assert_eq!(
        key_for_capped(&layout_id(9), 40),
        Err(CapTooShort { max_length: 40 })
    );
}
