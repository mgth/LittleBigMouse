//! What the agent has told this window, and what it means.
//!
//! This lived in `main.rs` until two bugs were found in it by **running the window**, and
//! neither by a test — because nothing outside a binary can drive a binary. Both sat
//! exactly here:
//!
//! * `Subscribe` answers *with* the snapshot rather than sending a `State` event for it,
//!   so a window that only listened for events learnt nothing until something moved and
//!   sat showing "No engine" in front of a connected hook;
//! * a live preview outlived the window that asked for it.
//!
//! So the conversation is a value now. It takes what the agent said and gives back what
//! the window must **do** about it, without doing any of it: no socket, no window, no
//! clock. What is left in `main.rs` is the part that genuinely needs those.
//!
//! It does not own [`lbm_ui::State`]. That is the bar's, and the bar is drawn from it
//! every frame; this only moves it, through `lbm_ui::update`, so that the rules living
//! there — the engine going down ends a preview, a press is refused while one is out —
//! are applied and not re-implemented.

use std::collections::HashMap;

use crate::client::Message;

/// The requests in flight, and everything the agent has said about itself.
#[derive(Debug, Default)]
pub struct Conversation {
    /// What each request in flight was, by the id its answer will carry.
    ///
    /// Without it an answer is an anonymous `Ok(null)` and the window can only guess what
    /// it settles — which is why a `SaveLayout` the agent refused used to be invisible.
    asked: HashMap<u64, &'static str>,
    /// The last thing the agent refused, and what was refused. Shown, because a request
    /// that fails silently is worse than one that fails.
    pub refused: Option<String>,
    /// The processes seen in the foreground this session, as the agent lists them.
    pub seen: Vec<String>,
    /// The shortcut the hook said it could not arm, if it said so.
    ///
    /// Only the hook knows: it is the one that registers. A well-formed shortcut nothing
    /// armed is exactly the case a user would otherwise discover at the moment they need
    /// the rescue.
    pub shortcut_unavailable: Option<String>,
    /// Whether anything has been heard from the agent yet.
    pub heard: bool,
}

impl Conversation {
    /// A conversation that already has requests out — the handshake's.
    ///
    /// `Hello` and `Subscribe` go before the window has a table to record them in, and
    /// **the answer to `Subscribe` is the agent's whole state**: a window that did not
    /// know which request an answer answered would throw that state away.
    pub fn after(handshake: impl IntoIterator<Item = (u64, &'static str)>) -> Conversation {
        Conversation {
            asked: handshake.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Records a request going out.
    ///
    /// Called **before** it leaves, on the thread that sends it: an answer that arrives
    /// the instant after must not find the pairing missing. That is why the window picks
    /// the ids rather than the writing thread — the two orderings are not equally safe
    /// and only one of them has no race in it.
    pub fn asking(&mut self, id: u64, method: &'static str) {
        self.asked.insert(id, method);
    }

    /// A new rescue shortcut is on its way, so the hook's verdict is about the old one.
    pub fn shortcut_changed(&mut self) {
        self.shortcut_unavailable = None;
    }

    /// What the agent said, in the bar's terms. Returns what the window must go and do.
    pub fn said(&mut self, message: Message, state: &mut lbm_ui::State) -> Vec<lbm_ui::Effect> {
        self.heard = true;
        match message {
            Message::Answer { id, result } => {
                let method = self.asked.remove(&id).unwrap_or("something");
                match result {
                    Ok(value) => {
                        self.refused = None;
                        let effects = self.answered(method, &value, state);
                        // Whatever it said, a request came back: the bar stops waiting.
                        lbm_ui::update(state, lbm_ui::Action::Answered);
                        return effects;
                    }
                    // The agent says why in words meant for a person ("no layout yet",
                    // "this agent keeps no options"); passing them through beats
                    // inventing a summary of them.
                    Err(why) => self.refused = Some(format!("{method} refused: {why}")),
                }
                lbm_ui::update(state, lbm_ui::Action::Answered)
            }
            Message::State(snapshot) => self.snapshot(&snapshot, state),
            // The hook's own report on the rescue. Only it knows whether the registration
            // took, so this is repeated rather than reasoned about.
            Message::Hook { name, payload } => {
                if name == "ShortcutUnavailable" {
                    self.shortcut_unavailable = Some(payload);
                }
                Vec::new()
            }
            // A message this version does not know. Ignored rather than guessed at: the
            // client keeps the same rule for events it cannot name.
            Message::Unknown(_) => Vec::new(),
        }
    }

    /// What an answer carried, for the requests whose answer says something.
    fn answered(
        &mut self,
        method: &str,
        value: &serde_json::Value,
        state: &mut lbm_ui::State,
    ) -> Vec<lbm_ui::Effect> {
        match method {
            // **These answer with the agent's whole state**; everything after arrives as
            // an event, and an event is a *change*. Reading it here is what tells the
            // window what it has connected to — without it the bar waits for something to
            // move, which on a quiet desktop is never.
            "Subscribe" | "Snapshot" => self.snapshot(value, state),
            "SeenProcesses" => {
                self.seen = value
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                Vec::new()
            }
            // Most say `null`: the agent did it, and the `State` event that follows is
            // the real news.
            _ => Vec::new(),
        }
    }

    /// The agent's state, however it arrived.
    fn snapshot(
        &mut self,
        snapshot: &serde_json::Value,
        state: &mut lbm_ui::State,
    ) -> Vec<lbm_ui::Effect> {
        let engine = snapshot
            .get("Engine")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Dead");
        // Through `update`, not written straight onto the state: the rule that ends a
        // live preview when the engine goes down lives there, and it needs the transition.
        let said = lbm_ui::Action::AgentSaid {
            engine: lbm_ui::Engine::from_agent(engine),
            connected: snapshot
                .get("HookConnected")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        };
        let effects = lbm_ui::update(state, said);
        state.waiting = false;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The shape a real agent answers `Subscribe` with, taken from one.
    fn snapshot(engine: &str, hook: bool) -> serde_json::Value {
        json!({
            "AgentVersion": "0.1.0",
            "HookConnected": hook,
            "Engine": engine,
            "Suspended": false,
            "LayoutId": "PHL0927_AU52036001187+SAME035_H1AK500000",
            "Enabled": false,
            "Saved": false,
            "LoadAtStartup": false,
            "HideTrayIcon": false,
            "Previewing": false
        })
    }

    /// **The bug this module exists for.** `Subscribe` answers *with* the state; it sends
    /// no event for it. A window that only read events sat saying "No engine" in front of
    /// a connected hook, and no test could see it while this lived in `main.rs`.
    #[test]
    fn subscribing_is_what_tells_the_window_what_it_connected_to() {
        let mut agent = Conversation::after([(1, "Hello"), (2, "Subscribe")]);
        let mut state = lbm_ui::State::default();
        assert!(!state.hook_connected, "nothing known yet");

        agent.said(
            Message::Answer {
                id: 2,
                result: Ok(snapshot("Stopped", true)),
            },
            &mut state,
        );

        assert!(state.hook_connected, "the state in the answer was dropped");
        assert_eq!(state.engine, lbm_ui::Engine::Stopped);
        assert!(lbm_ui::can(&state, lbm_ui::Press::Start));
        assert!(lbm_ui::can(&state, lbm_ui::Press::Live));
    }

    /// And an answer whose request nobody recorded must not be read as a state: that is
    /// how the handshake's ids came to be carried back in the first place.
    #[test]
    fn an_answer_to_a_request_nobody_recorded_changes_nothing() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();

        agent.said(
            Message::Answer {
                id: 99,
                result: Ok(snapshot("Running", true)),
            },
            &mut state,
        );

        assert!(
            !state.hook_connected,
            "a snapshot was read from an answer nobody could identify"
        );
    }

    #[test]
    fn a_refusal_names_what_was_refused_and_is_kept_to_be_shown() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        agent.asking(7, "SaveLayout");

        agent.said(
            Message::Answer {
                id: 7,
                result: Err("no layout yet".to_owned()),
            },
            &mut state,
        );

        let said = agent.refused.expect("a refusal the user can read");
        assert!(said.contains("SaveLayout"), "{said}");
        assert!(said.contains("no layout yet"), "{said}");
    }

    /// A request that succeeds clears the last refusal: it was about a request that is
    /// over, and leaving it on screen would blame the wrong one.
    #[test]
    fn an_answer_that_worked_clears_the_last_refusal() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        agent.asking(1, "Start");
        agent.said(
            Message::Answer {
                id: 1,
                result: Err("nope".to_owned()),
            },
            &mut state,
        );
        assert!(agent.refused.is_some());

        agent.asking(2, "Stop");
        agent.said(
            Message::Answer {
                id: 2,
                result: Ok(json!(null)),
            },
            &mut state,
        );
        assert_eq!(agent.refused, None);
    }

    #[test]
    fn the_seen_processes_are_read_out_of_their_answer() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        agent.asking(4, "SeenProcesses");

        agent.said(
            Message::Answer {
                id: 4,
                result: Ok(json!(["/usr/bin/game", "/opt/other"])),
            },
            &mut state,
        );

        assert_eq!(agent.seen, vec!["/usr/bin/game", "/opt/other"]);
    }

    /// An empty session answers with an empty **array**, not null — which is what lets
    /// the window tell "nothing seen" from "it failed".
    #[test]
    fn an_empty_seen_list_is_not_a_failure() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        agent.asking(4, "SeenProcesses");
        agent.said(
            Message::Answer {
                id: 4,
                result: Ok(json!([])),
            },
            &mut state,
        );
        assert!(agent.seen.is_empty());
        assert_eq!(agent.refused, None);
    }

    #[test]
    fn the_hooks_verdict_on_the_shortcut_is_kept_and_cleared_when_it_changes() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();

        agent.said(
            Message::Hook {
                name: "ShortcutUnavailable".to_owned(),
                payload: "Ctrl+Alt+Shift+M".to_owned(),
            },
            &mut state,
        );
        assert_eq!(
            agent.shortcut_unavailable.as_deref(),
            Some("Ctrl+Alt+Shift+M")
        );

        agent.shortcut_changed();
        assert_eq!(agent.shortcut_unavailable, None);
    }

    /// The rule that matters most about the engine, reached through a real message: the
    /// tray's Stop under a live preview must end the preview, or the next tick hands the
    /// hook a `Load` and a `Run` and puts it straight back up.
    #[test]
    fn an_engine_that_goes_down_under_a_preview_ends_it() {
        let mut agent = Conversation::after([(2, "Subscribe")]);
        let mut state = lbm_ui::State::default();
        agent.said(
            Message::Answer {
                id: 2,
                result: Ok(snapshot("Stopped", true)),
            },
            &mut state,
        );
        lbm_ui::update(&mut state, lbm_ui::Action::Pressed(lbm_ui::Press::Live));
        agent.said(Message::State(snapshot("Running", true)), &mut state);
        assert!(state.live, "the preview should still be on");

        let effects = agent.said(Message::State(snapshot("Stopped", true)), &mut state);

        assert!(!state.live);
        assert_eq!(effects, vec![lbm_ui::Effect::EndPreview]);
    }

    /// A message this version does not know leaves everything as it was, rather than
    /// being mapped onto something it might not be.
    #[test]
    fn an_unknown_message_moves_nothing() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State {
            hook_connected: true,
            ..Default::default()
        };

        let effects = agent.said(
            Message::Unknown(json!({ "Event": "FromTheFuture" })),
            &mut state,
        );

        assert!(effects.is_empty());
        assert!(state.hook_connected);
        assert!(agent.heard, "it still counts as having heard the agent");
    }
}
