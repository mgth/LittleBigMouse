//! The excluded-processes section, through the accessibility tree.
//!
//! The rule this pins above all: **a window that has not read the list does not offer it
//! for editing.** Every other section of the panel works from values it has; this one can
//! be in a third state — not read — and an empty list drawn for that state would let the
//! user "remove" exclusions that are merely absent, and then save the removal.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use lbm_ui::excluded::{self, Did};

fn panel<'a>(
    list: Option<&'a [String]>,
    pattern: &'a std::cell::RefCell<String>,
    did: &'a std::cell::RefCell<Option<Did>>,
    seen: &'a [String],
) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(520.0, 900.0))
        .build_ui(move |ui| {
            if let Some(what) = excluded::panel(ui, list, &mut pattern.borrow_mut(), seen) {
                *did.borrow_mut() = Some(what);
            }
        })
}

#[test]
fn a_list_that_was_never_read_is_not_offered_for_editing() {
    let pattern = std::cell::RefCell::new(String::new());
    let did = std::cell::RefCell::new(None);
    let mut harness = panel(None, &pattern, &did, &[]);
    harness.run();

    assert!(
        harness.query_by_label("Add defaults").is_none(),
        "an unread list was offered for editing"
    );
    assert!(
        harness.query_by_label("Exclude").is_none(),
        "an unread list was offered for editing"
    );
}

/// An empty list is **not** the same state: it was read and it is empty, so editing it is
/// meaningful.
#[test]
fn a_list_that_is_read_and_empty_is_still_editable() {
    let pattern = std::cell::RefCell::new(String::new());
    let did = std::cell::RefCell::new(None);
    let empty: Vec<String> = Vec::new();
    let mut harness = panel(Some(&empty), &pattern, &did, &[]);
    harness.run();

    assert!(harness.query_by_label("Add defaults").is_some());
}

#[test]
fn typing_a_pattern_and_pressing_exclude_adds_it() {
    let pattern = std::cell::RefCell::new(String::new());
    let did = std::cell::RefCell::new(None);
    let list = vec!["game.exe".to_owned()];
    let mut harness = panel(Some(&list), &pattern, &did, &[]);
    harness.run();

    // Nothing typed: the button means nothing and cannot be pressed.
    harness.get_by_label("Exclude").click();
    harness.run();
    assert_eq!(*did.borrow(), None, "an empty pattern was added");

    pattern.replace("other.exe".to_owned());
    harness.run();
    harness.get_by_label("Exclude").click();
    harness.run();
    assert_eq!(*did.borrow(), Some(Did::Add("other.exe".to_owned())));
}

/// Each Remove button carries its entry. A row of buttons all called "Remove" is a row
/// nobody can tell apart — by ear, or from here.
#[test]
fn every_entry_has_its_own_remove() {
    let pattern = std::cell::RefCell::new(String::new());
    let did = std::cell::RefCell::new(None);
    let list = vec!["game.exe".to_owned(), "other.exe".to_owned()];
    let mut harness = panel(Some(&list), &pattern, &did, &[]);
    harness.run();

    harness.get_by_label("Remove other.exe").click();
    harness.run();
    assert_eq!(*did.borrow(), Some(Did::Remove("other.exe".to_owned())));
}

#[test]
fn a_seen_process_becomes_the_pattern_without_being_added() {
    let pattern = std::cell::RefCell::new(String::new());
    let did = std::cell::RefCell::new(None);
    let list = vec!["game.exe".to_owned()];
    let seen = vec!["/usr/bin/somegame".to_owned()];
    let mut harness = panel(Some(&list), &pattern, &did, &seen);
    harness.run();

    harness.get_by_label("/usr/bin/somegame").click();
    harness.run();
    assert_eq!(*pattern.borrow(), "/usr/bin/somegame");
    assert_eq!(
        *did.borrow(),
        None,
        "picking a seen process excluded it outright instead of proposing it"
    );
}
