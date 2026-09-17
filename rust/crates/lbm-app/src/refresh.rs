//! Invalidating an asynchronous display read without discarding an unsaved document.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub layout_id: Option<String>,
    pub revision: Option<u64>,
    pub signature: Option<String>,
}

impl Stamp {
    pub fn from_snapshot(snapshot: &Value) -> Self {
        Self {
            layout_id: snapshot["LayoutId"].as_str().map(str::to_owned),
            revision: snapshot["LayoutRevision"].as_u64(),
            signature: snapshot["DisplaySignature"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        }
    }
}

#[derive(Default)]
pub struct Refresh {
    stamp: Option<Stamp>,
    generation: u64,
    pending: bool,
}

impl Refresh {
    /// A reconnect also invalidates the old read: revision counters restart with the
    /// agent, and the display configuration may have changed while it was absent.
    pub fn observe(&mut self, stamp: Stamp, reconnected: bool, dirty: bool) -> bool {
        if !reconnected && self.stamp.as_ref() == Some(&stamp) {
            return false;
        }
        // An agent restart on the same physical desktop need not strand an edit.
        // Older agents do not publish a signature, so cannot establish this equality.
        if reconnected
            && dirty
            && self.stamp.as_ref().is_some_and(|old| {
                old.layout_id == stamp.layout_id
                    && old.signature.is_some()
                    && old.signature == stamp.signature
            })
        {
            self.stamp = Some(stamp);
            return false;
        }
        self.stamp = Some(stamp);
        self.invalidate();
        true
    }

    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.pending = true;
    }

    pub fn pending(&self) -> bool {
        self.pending
    }

    /// Dirty documents wait for an explicit reload. No background read earns the
    /// right to throw away an edit just because a dock or an agent came back.
    pub fn begin(&self, dirty: bool) -> Option<u64> {
        (self.pending && !dirty).then_some(self.generation)
    }

    pub fn accepts(&self, generation: u64) -> bool {
        self.generation == generation
    }

    pub fn complete(&mut self, generation: u64, layout_id: &str) -> bool {
        if !self.accepts(generation)
            || self.stamp.as_ref().is_some_and(|stamp| {
                stamp
                    .layout_id
                    .as_deref()
                    .is_some_and(|expected| expected != layout_id)
            })
        {
            return false;
        }
        self.pending = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_resolution_change_invalidates_a_read_even_with_the_same_monitors() {
        let mut refresh = Refresh::default();
        refresh.observe(
            Stamp::from_snapshot(&json!({"LayoutId":"A", "LayoutRevision":1})),
            false,
            false,
        );
        let old = refresh.begin(false).unwrap();
        refresh.observe(
            Stamp::from_snapshot(&json!({"LayoutId":"A", "LayoutRevision":2})),
            false,
            false,
        );
        assert!(!refresh.complete(old, "A"));
        assert!(refresh.complete(refresh.begin(false).unwrap(), "A"));
    }

    #[test]
    fn reconnect_and_hotplug_preserve_edits_until_reload_is_chosen() {
        let mut refresh = Refresh::default();
        let stamp = Stamp::from_snapshot(&json!({"LayoutId":"A", "LayoutRevision":1}));
        refresh.observe(stamp.clone(), false, false);
        assert!(refresh.complete(refresh.begin(false).unwrap(), "A"));
        assert!(!refresh.observe(stamp.clone(), false, false));
        assert!(refresh.observe(stamp, true, true));
        assert_eq!(refresh.begin(true), None);
        assert!(refresh.pending());
        assert!(refresh.complete(refresh.begin(false).unwrap(), "A"));
    }

    #[test]
    fn a_detection_of_the_wrong_desktop_is_not_adopted() {
        let mut refresh = Refresh::default();
        refresh.observe(Stamp::from_snapshot(&json!({"LayoutId":"B"})), false, false);
        assert!(!refresh.complete(refresh.begin(false).unwrap(), "A"));
        assert!(refresh.pending());
    }

    #[test]
    fn restarting_on_the_same_desktop_does_not_strand_an_unsaved_edit() {
        let mut refresh = Refresh::default();
        let stamp = |revision, signature| {
            Stamp::from_snapshot(&json!({
                "LayoutId":"A", "LayoutRevision":revision, "DisplaySignature":signature
            }))
        };
        refresh.observe(stamp(8, "1920x1080"), false, false);
        refresh.complete(refresh.begin(false).unwrap(), "A");
        assert!(!refresh.observe(stamp(1, "1920x1080"), true, true));
        assert!(!refresh.pending());
        assert!(refresh.observe(stamp(1, "3840x2160"), true, true));
        assert!(refresh.pending());
        assert_eq!(refresh.begin(true), None);
    }
}
