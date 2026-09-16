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

/// What an apply of the topology did, or would have done.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    /// Every command line, in the order the agent ran them — the engine's gap-closing
    /// first, because it really does go first.
    pub commands: Vec<String>,
    /// Nothing was run. The positions are a **prediction**: the real pass re-reads the
    /// compositor after the scales are applied and re-asserts whatever drifts, and
    /// neither of those can be known without changing something.
    pub dry: bool,
}

/// The requests in flight, and everything the agent has said about itself.
#[derive(Debug, Default)]
pub struct Conversation {
    /// What each request in flight was, by the id its answer will carry.
    ///
    /// Without it an answer is an anonymous `Ok(null)` and the window can only guess what
    /// it settles — which is why a `SaveLayout` the agent refused used to be invisible.
    asked: HashMap<u64, &'static str>,
    /// What a request in flight **earns** if the agent takes it: the document a save
    /// sent, which becomes what Save and Undo compare against.
    ///
    /// Held here rather than in the window because the rule is about answers, and
    /// answers are this module's business. Two things it gets right that a reference
    /// taken on arrival would not: the document is the one **as sent**, so editing while
    /// a save is in flight does not get quietly called saved; and a refusal drops it, so
    /// the buttons stay lit over a layout that is still not on disk.
    earning: HashMap<u64, crate::saved::Reference>,
    /// A save has landed: the reference the window must adopt. Taken, not read — it is a
    /// one-shot fact, and leaving it behind would re-adopt it at every frame.
    earned: Option<crate::saved::Reference>,
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
    /// What an `ApplyTopology` reported: the command lines, and whether they were only
    /// going to be run. Kept so the window can show them — the one action in the product
    /// that cannot be undone is also the one worth reading before and after.
    pub topology: Option<Ran>,
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

    /// Records the document request `id` was built from, to be adopted if it is accepted.
    pub fn earning(&mut self, id: u64, reference: crate::saved::Reference) {
        self.earning.insert(id, reference);
    }

    /// The document a landed save earned, once. See [`Conversation::earning`].
    pub fn earned(&mut self) -> Option<crate::saved::Reference> {
        self.earned.take()
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
                // Whatever this request was carrying is settled either way: taken on an
                // answer, dropped on a refusal.
                let earning = self.earning.remove(&id);
                match result {
                    Ok(value) => {
                        self.earned = earning;
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
            // The screens moved, or would have. Either way the window shows the lines.
            "ApplyTopology" => {
                self.topology = Some(Ran {
                    commands: value
                        .get("Commands")
                        .and_then(serde_json::Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(|v| v.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    dry: value.get("DryRun").and_then(serde_json::Value::as_bool) == Some(true),
                });
                Vec::new()
            }
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
    //==================//
    // What a save earns//
    //==================//

    /// The layout a window would be holding, as the real pipeline builds it.
    fn layout() -> lbm_layout::model::Layout {
        let mut layout =
            lbm_layout::model::Layout::new(lbm_layout::model::LayoutOptions::default());
        lbm_layout::linux::populate(&mut layout, &[], |_| Ok::<(), std::io::Error>(())).unwrap();
        layout.set_location(
            &layout.monitors()[0].id.clone(),
            lbm_layout::geo::Point::new(0.0, 0.0),
        );
        layout
    }

    #[test]
    fn a_save_the_agent_takes_hands_back_the_document_it_sent() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        let layout = layout();
        agent.asking(7, "SaveLayout");
        agent.earning(7, crate::saved::Reference::of(&layout));
        assert!(agent.earned().is_none(), "nothing until the answer comes");

        agent.said(
            Message::Answer {
                id: 7,
                result: Ok(serde_json::Value::Null),
            },
            &mut state,
        );

        let earned = agent.earned().expect("the document that save sent");
        assert!(earned.holds(&layout), "it is the layout that was sent");
        assert!(
            agent.earned().is_none(),
            "taken once: a second frame must not re-adopt it"
        );
    }

    /// The window goes on being usable while a save is out. What that save earns is the
    /// document it **sent**, so an edit made in the meantime is still an edit to save.
    #[test]
    fn an_edit_made_while_a_save_is_in_flight_is_not_swallowed_by_it() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        let mut layout = layout();
        agent.asking(7, "SaveLayout");
        agent.earning(7, crate::saved::Reference::of(&layout));

        // The user drags a screen before the answer lands.
        let monitor = layout.monitors()[0].id.clone();
        layout.set_location(&monitor, lbm_layout::geo::Point::new(300.0, 100.0));

        agent.said(
            Message::Answer {
                id: 7,
                result: Ok(serde_json::Value::Null),
            },
            &mut state,
        );

        let earned = agent.earned().expect("the save landed");
        assert!(
            !earned.holds(&layout),
            "the drag made after the save was sent is still unsaved"
        );
    }

    /// A refusal writes nothing, so it earns nothing: the buttons stay lit over a layout
    /// that is still not on disk.
    #[test]
    fn a_save_the_agent_refuses_earns_nothing() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        agent.asking(7, "SaveLayout");
        agent.earning(7, crate::saved::Reference::of(&layout()));

        agent.said(
            Message::Answer {
                id: 7,
                result: Err("not the current one".to_owned()),
            },
            &mut state,
        );

        assert!(agent.earned().is_none(), "a refusal saved nothing");
        assert!(agent.refused.is_some(), "and it is shown");
    }

    /// Another request answering in between settles nothing about the save.
    #[test]
    fn an_answer_to_something_else_does_not_clear_the_save() {
        let mut agent = Conversation::default();
        let mut state = lbm_ui::State::default();
        agent.asking(7, "SaveLayout");
        agent.earning(7, crate::saved::Reference::of(&layout()));
        agent.asking(8, "Snapshot");

        agent.said(
            Message::Answer {
                id: 8,
                result: Ok(snapshot("Running", true)),
            },
            &mut state,
        );

        assert!(
            agent.earned().is_none(),
            "a snapshot answered, not the save"
        );
    }
}
