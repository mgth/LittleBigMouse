//! What this window means by "saved" — the state of the Save and Undo buttons.
//!
//! The plan settles it as "the current DTO is not the saved DTO", and that formula has a
//! trap in it. It holds for a document written by the current version; for an **older**
//! document the DTO differs with no edit at all, because saving would *migrate* it. That
//! is not a guess: `save_after_load_v56_reproduces_the_stored_files` (lbm-store) loads
//! the `v5.6-current` fixture, saves it untouched, and asserts the result against a
//! **different** committed golden — `v5.6-current-saved`, where the layout's priority has
//! moved up into `options.json` and every `+` of the rescue shortcut is a unicode escape.
//! A window comparing against the store would open offering to save a layout nobody had
//! touched, on every profile written by an older version.
//!
//! So the comparison is not against the store. A **reference** is taken when the document
//! is loaded and again each time a save lands, and "unsaved" is *current ≠ reference*.
//! Three things follow, and each is a thing the ported flag gets wrong:
//!
//! * a migration is not an edit — the reference is taken *after* the load, migrations
//!   included, so an old profile opens clean;
//! * **an edit undone by hand is saved again.** Drag a screen away and drag it back and
//!   there is nothing to save, which is the truth; the C# `Saved` flag is a latch that
//!   only a load or a save can reset (`MonitorsLayout.Saved`), so it stays lit;
//! * **a save that lands clears it.** The window used to send `SaveLayout` and never hear
//!   the answer in a way that touched the flag, so Save stayed lit over a layout already
//!   on disk.
//!
//! Nothing is re-read from the store: the reference is what this window loaded or last
//! sent, held in memory, which is also what makes it honest about a save the agent
//! refused — that save left no reference behind.

use lbm_layout::model::Layout;
use lbm_store::LayoutDocument;

/// The document this window last loaded or last saved.
#[derive(Clone, Debug)]
pub struct Reference(LayoutDocument);

impl Reference {
    /// Takes the reference: at a load, at an undo, and when a save comes back accepted.
    pub fn of(layout: &Layout) -> Reference {
        Reference(crate::settings::document(layout))
    }

    /// Whether `layout` would save to the same document — nothing to save, nothing to
    /// undo.
    ///
    /// The comparison is on the document [`crate::settings::save_layout`] sends, not on
    /// the layout: two layouts that write the same file are the same layout as far as a
    /// Save button is concerned.
    pub fn holds(&self, layout: &Layout) -> bool {
        self.0 == crate::settings::document(layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_layout::geo::Point;
    use lbm_layout::model::LayoutOptions;

    /// The screens from the real pipeline, as the window gets them — no store, so what
    /// the system reports is the whole of it.
    fn layout() -> Layout {
        let mut layout = Layout::new(LayoutOptions::default());
        lbm_layout::linux::populate(&mut layout, &[], |_| Ok::<(), std::io::Error>(())).unwrap();
        layout
    }

    /// The first monitor's id, and a place to put it back.
    fn somewhere(layout: &mut Layout) -> (String, Point) {
        let monitor = layout.monitors()[0].id.clone();
        let home = Point::new(0.0, 0.0);
        layout.set_location(&monitor, home);
        (monitor, home)
    }

    #[test]
    fn a_layout_nobody_has_touched_has_nothing_to_save() {
        let mut layout = layout();
        somewhere(&mut layout);
        assert!(Reference::of(&layout).holds(&layout));
    }

    #[test]
    fn an_edit_is_unsaved_and_undoing_it_by_hand_is_saved_again() {
        let mut layout = layout();
        let (monitor, home) = somewhere(&mut layout);
        let reference = Reference::of(&layout);

        layout.set_location(&monitor, Point::new(40.0, 20.0));
        assert!(
            !reference.holds(&layout),
            "a screen was moved: there is something to save"
        );

        // Put back exactly. The ported `Saved` flag cannot come back from this — it is a
        // latch — and that is the whole point of comparing documents instead.
        layout.set_location(&monitor, home);
        assert!(
            reference.holds(&layout),
            "the screen is where it was: there is nothing left to save"
        );
        assert!(
            !layout.saved(),
            "the ported flag still says unsaved, which is what this replaces"
        );
    }

    #[test]
    fn an_option_that_a_save_carries_counts_and_one_it_does_not_carry_does_not() {
        let mut layout = layout();
        somewhere(&mut layout);
        let reference = Reference::of(&layout);

        // In the document a save sends.
        layout.edit_options(|o| o.loop_x = !o.loop_x);
        assert!(!reference.holds(&layout), "LoopX is written by a save");
        layout.edit_options(|o| o.loop_x = !o.loop_x);
        assert!(reference.holds(&layout));

        // Stripped from it: the window holds no excluded list, so `save_layout` leaves
        // the field out rather than erasing the user's. It must not light Save either.
        layout.edit_options(|o| o.excluded_list = vec!["/usr/bin/steam".to_owned()]);
        assert!(
            reference.holds(&layout),
            "the excluded list is not part of what Save sends"
        );
    }

    #[test]
    fn a_reference_taken_again_after_a_save_clears_the_buttons() {
        let mut layout = layout();
        let (monitor, _) = somewhere(&mut layout);
        let reference = Reference::of(&layout);
        layout.set_location(&monitor, Point::new(500.0, 500.0));
        assert!(!reference.holds(&layout));

        // What the window does when `SaveLayout` comes back accepted.
        let reference = Reference::of(&layout);
        assert!(reference.holds(&layout), "saved: nothing left to save");
    }
}
