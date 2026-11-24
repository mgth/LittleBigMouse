//! Which zones the hook holds, as far as the agent can tell — and which are on their way.
//!
//! The multi-PC mode opens a border to another machine only once the hook holds zones that
//! lead nowhere else (`peers::Service::hook_ready`), so this is a safety question, not a
//! courtesy: believing the hook holds the zones of the selected machine while it still
//! holds another's is a cursor sent to a machine nobody chose.
//!
//! Every `Load` goes out with an id of its own, and the hook names it again in the
//! `Loaded` or `LoadFailed` that answers it. Matching answers by their rank instead does
//! not survive a reconnection: a frame the agent had already counted lost can still be
//! written to the new connection — it sat in the client's queue while the old one
//! dropped — and its answer would then be taken for the next `Load`'s.

use std::collections::{BTreeSet, VecDeque};

/// The other machines a set of zones leads to, by host.
pub(crate) type Hosts = BTreeSet<String>;

#[derive(Debug)]
pub(crate) struct HookZones {
    /// What the hook holds; `None` when that is not known (no hook, a hook holding a
    /// layout this agent cannot name, an answer to a `Load` this agent no longer counts).
    holds: Option<Hosts>,
    /// Every `Load` sent and not yet answered, oldest first, by its id.
    in_flight: VecDeque<(u64, Hosts)>,
    /// The id the next `Load` goes out with. Never reset, not even with the connection:
    /// an id must not name two `Load`s of the same agent.
    next_id: u64,
}

impl HookZones {
    /// Ids start somewhere no other agent's are likely to: the hook tells every listener
    /// how a `Load` went, and a previous life of this agent may have left one in its queue.
    pub(crate) fn new() -> Self {
        use std::hash::{BuildHasher, Hasher};
        let seed = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        Self::starting_at(seed)
    }

    pub(crate) fn starting_at(first_id: u64) -> Self {
        HookZones {
            holds: None,
            in_flight: VecDeque::new(),
            next_id: first_id,
        }
    }

    pub(crate) fn holds(&self) -> Option<&Hosts> {
        self.holds.as_ref()
    }

    pub(crate) fn in_flight(&self) -> impl Iterator<Item = &Hosts> {
        self.in_flight.iter().map(|(_, hosts)| hosts)
    }

    /// Nothing is known any more: a new connection, or none.
    pub(crate) fn unknown(&mut self) {
        self.holds = None;
        self.in_flight.clear();
    }

    /// What a hook that just said hello holds, as far as its greeting names it.
    pub(crate) fn greeted(&mut self, holds: Option<Hosts>) {
        self.holds = holds;
    }

    /// A `Load` of these zones is going out: the id it goes with.
    pub(crate) fn sending(&mut self, hosts: Hosts) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.in_flight.push_back((id, hosts));
        id
    }

    /// The hook took the `Load` it names. One this agent does not count — sent before a
    /// reconnection, or by someone else — leaves it holding zones nobody here can name,
    /// and the ones still on their way on their way.
    ///
    /// A hook that names none predates the id: its answers are taken in order.
    pub(crate) fn loaded(&mut self, id: Option<u64>) {
        self.holds = match id {
            Some(id) => self.answered(id),
            None => self.in_flight.pop_front().map(|(_, hosts)| hosts),
        };
    }

    /// The hook refused the `Load` it names: it keeps what it held.
    pub(crate) fn load_failed(&mut self, id: Option<u64>) {
        match id {
            Some(id) => {
                self.answered(id);
            }
            None => {
                if self.in_flight.pop_front().is_none() {
                    self.holds = None;
                }
            }
        }
    }

    /// The `Load` `id` is answered: no longer on its way, and nor is any sent before it —
    /// the hook answers in order, so an earlier one it has not answered never will be.
    fn answered(&mut self, id: u64) -> Option<Hosts> {
        let at = self.in_flight.iter().position(|(sent, _)| *sent == id)?;
        self.in_flight
            .drain(..=at)
            .next_back()
            .map(|(_, hosts)| hosts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(names: &[&str]) -> Hosts {
        names.iter().map(|&n| n.to_owned()).collect()
    }

    /// The race the ids are for. A `Load` of zones leading to the tower goes out; the
    /// connection drops before it is written, and the agent counts it lost — but the
    /// client still holds the frame, and writes it to the next connection. There the
    /// agent sends its own `Load` of zones leading nowhere. The first answer is the
    /// tower's: taken by rank it would read as "the hook holds zones fit for no remote"
    /// while the hook holds the tower's.
    #[test]
    fn an_answer_to_a_load_counted_lost_is_not_taken_for_the_next_one() {
        let mut zones = HookZones::starting_at(1);
        zones.greeted(Some(hosts(&[])));
        let lost = zones.sending(hosts(&["tower"]));

        zones.unknown();
        zones.greeted(Some(hosts(&[])));
        let wanted = zones.sending(hosts(&[]));

        zones.loaded(Some(lost));
        assert_eq!(
            zones.holds(),
            None,
            "the hook holds zones this agent no longer counts: unknown, not the next Load's"
        );
        assert_eq!(
            zones.in_flight().count(),
            1,
            "the wanted Load is still on its way"
        );

        zones.loaded(Some(wanted));
        assert_eq!(zones.holds(), Some(&hosts(&[])));
        assert_eq!(zones.in_flight().count(), 0);
    }

    #[test]
    fn a_refusal_of_a_load_counted_lost_leaves_the_next_one_on_its_way() {
        let mut zones = HookZones::starting_at(1);
        zones.greeted(Some(hosts(&[])));
        let lost = zones.sending(hosts(&["tower"]));
        zones.unknown();
        zones.greeted(Some(hosts(&[])));
        let wanted = zones.sending(hosts(&["laptop"]));

        zones.load_failed(Some(lost));
        assert_eq!(
            zones.holds(),
            Some(&hosts(&[])),
            "a refused Load changes nothing"
        );
        assert_eq!(zones.in_flight().count(), 1);

        zones.loaded(Some(wanted));
        assert_eq!(zones.holds(), Some(&hosts(&["laptop"])));
    }

    #[test]
    fn answers_on_one_connection_follow_their_loads() {
        let mut zones = HookZones::starting_at(u64::MAX);
        zones.greeted(None);
        let first = zones.sending(hosts(&["tower"]));
        let second = zones.sending(hosts(&[]));
        assert_ne!(first, second, "ids wrap without repeating the last one");

        zones.load_failed(Some(first));
        assert_eq!(zones.holds(), None, "refused: still what it held, unknown");
        zones.loaded(Some(second));
        assert_eq!(zones.holds(), Some(&hosts(&[])));
    }

    /// A hook built before the id answers with none: its answers are taken in order, as
    /// every answer was before.
    #[test]
    fn a_hook_that_names_no_load_is_answered_in_order() {
        let mut zones = HookZones::starting_at(1);
        zones.greeted(Some(hosts(&[])));
        zones.sending(hosts(&["tower"]));
        zones.sending(hosts(&[]));

        zones.loaded(None);
        assert_eq!(zones.holds(), Some(&hosts(&["tower"])));
        zones.load_failed(None);
        assert_eq!(zones.holds(), Some(&hosts(&["tower"])));
        assert_eq!(zones.in_flight().count(), 0);

        // An answer to nothing this agent sent: what the hook holds is unknown.
        zones.loaded(None);
        assert_eq!(zones.holds(), None);
    }
}
