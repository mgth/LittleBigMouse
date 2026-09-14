//! The frontend's shape, before it has a window.
//!
//! Phase 6 of the v6 plan asks for one thing before any screen is drawn: state, an
//! `Action`, and a pure `update` — views being functions of the state, and effects (the
//! agent, DDC that takes two seconds, files) never running on the UI thread. This crate
//! is that shape, with the smallest piece of the real product in it: the engine
//! controls of the bottom bar, which mirror what the agent says and ask it for what the
//! user pressed.
//!
//! It exists mostly to answer a question the Avalonia UI never could here: **can the
//! interface be driven and asserted on without a window server?** Under KWin, `xdotool`
//! cannot click, so every UI check on this machine has been a screenshot and a squint.
//! `egui_kittest` drives the widgets through the accessibility tree, headless, in CI —
//! see `tests/bottom_bar.rs`.

pub mod drag;
pub mod frame;
pub mod list;
pub mod map;
pub mod options;
pub mod probe;
pub mod wallpaper;

/// What the engine is doing, in the agent's own words (its `Snapshot.Engine`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Engine {
    Running,
    Stopped,
    Paused,
    /// No hook to ask. What the agent reports when nothing answers at the endpoint.
    #[default]
    Dead,
}

impl Engine {
    /// What the agent sends; anything else is `Dead` rather than a guess — a frontend
    /// that mapped an unknown state onto a known one would claim to know.
    pub fn from_agent(word: &str) -> Engine {
        match word {
            "Running" => Engine::Running,
            "Stopped" => Engine::Stopped,
            "Paused" => Engine::Paused,
            _ => Engine::Dead,
        }
    }
}

/// Everything the bottom bar draws itself from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub engine: Engine,
    /// The agent has a hook to drive. Without one, asking it to start is asking for
    /// nothing to happen. The same fact as `engine == Dead`, from the other side.
    pub hook_connected: bool,
    /// A request is out and its answer has not come back. The agent is a socket away,
    /// so this is not instantaneous and the user must not be left pressing again.
    pub waiting: bool,
    /// The layout on screen is the layout in the store. The plan settles what this
    /// means — "the current DTO is not the saved DTO" — and that comparison belongs
    /// upstream: by the time the bar sees it, it is a yes or a no.
    pub saved: bool,
    /// A foreign layout, loaded to be looked at rather than run. It can always be sent
    /// again, because sending it changes nothing on this machine.
    pub is_virtual: bool,
    /// Every edit is fed to the engine as it is made, so the layout can be felt with the
    /// real mouse before it is kept.
    ///
    /// **This runs the engine.** The agent answers a preview with `Load` *and* `Run`
    /// (`runtime.rs`, `Effect::Preview`), so turning it on over a stopped engine starts
    /// it — which the C# says outright is a legitimate way to start. It is the one
    /// control in this window that makes the hook take the mice, and the button says so.
    ///
    /// Turning it off does not leave the engine where the preview put it: the agent goes
    /// back to the current layout, or takes the hook down if the user does not want this
    /// layout hooked (`reconcile.rs`, `end_preview`). So the switch is reversible, which
    /// is what makes it safe to offer at all.
    ///
    /// Never restored from a previous session: coming back to unsaved geometry already
    /// live would be a trap, and the mode costs one click
    /// (`LocationControlViewModel.LiveUpdate`).
    pub live: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            engine: Engine::default(),
            hook_connected: false,
            waiting: false,
            // Nothing has been edited yet, so there is nothing to save: a bar that
            // opened offering Save and Undo would be offering to undo nothing.
            saved: true,
            is_virtual: false,
            live: false,
        }
    }
}

/// What the user did, or what the agent said. The only two ways the state moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Pressed(Press),
    /// The agent's state, as a `State` event or a `Snapshot` answer carries it.
    AgentSaid {
        engine: Engine,
        connected: bool,
    },
    /// A request came back, whatever it said: the bar stops waiting.
    Answered,
}

/// The presses the bar can produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Start,
    Stop,
    /// Write the layout to the store.
    Save,
    /// Throw the edits away and load the stored layout again.
    Undo,
    /// Turn the live preview on or off.
    Live,
}

/// What the caller has to go and do, off the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Ask(Press),
    /// Stop previewing — the layout the agent runs goes back to the stored one.
    ///
    /// Its own effect rather than `Ask(Live)`, because it happens when nobody pressed
    /// anything: the engine going down under a live preview ends it.
    EndPreview,
}

/// The whole of the decision, with no window, no socket and no clock in it.
///
/// A press is refused rather than queued while one is already out: the agent answers in
/// order, and a second Start on the way to a hook that is already starting is a race
/// the user did not ask for.
pub fn update(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        // The live switch is not a request with an answer: it turns a mode on, and the
        // sending is the caller's business from then on. So it does not wait, and
        // turning it *off* is `EndPreview` rather than another ask.
        Action::Pressed(Press::Live) => {
            if !can(state, Press::Live) {
                return Vec::new();
            }
            state.live = !state.live;
            if state.live {
                vec![Effect::Ask(Press::Live)]
            } else {
                vec![Effect::EndPreview]
            }
        }
        Action::Pressed(press) => {
            if state.waiting || !can(state, press) {
                return Vec::new();
            }
            state.waiting = true;
            vec![Effect::Ask(press)]
        }
        Action::AgentSaid { engine, connected } => {
            let was = routing(state);
            state.engine = engine;
            state.hook_connected = connected;
            // **The engine going down outranks the preview.** The tray's Stop, a display
            // change, an excluded application — without this the next tick would hook it
            // straight back up, because a preview is a `Load` *and* a `Run`. A
            // transition, not the current state: turning the switch on over a stopped
            // engine is a legitimate way to start
            // (`LocationControlViewModel`, `.Skip(1).Where(running => !running)`).
            if state.live && was && !routing(state) {
                state.live = false;
                return vec![Effect::EndPreview];
            }
            // And a hook that goes away leaves nothing to preview into.
            if state.live && !can(state, Press::Live) {
                state.live = false;
                return vec![Effect::EndPreview];
            }
            Vec::new()
        }
        Action::Answered => {
            state.waiting = false;
            Vec::new()
        }
    }
}

/// Whether a press means anything in this state — which is also what greys the button.
///
/// These are the Avalonia rules (`LocationControlViewModel`), which are richer than
/// they first look:
///
/// * **Start** is not only for a stopped engine. A layout edited since it was applied
///   can be started again — that is how an edit reaches the engine — and a foreign
///   layout can always be sent, because sending it simulates rather than routes.
/// * **Stop** applies to a paused engine too. Paused is the engine standing aside for
///   an excluded application; it is running, and the C# tracker does not even move its
///   `Running` flag when the daemon says so.
/// * **Save** and **Undo** are the same condition read twice: there is something to
///   write, so there is something to throw away.
pub fn can(state: &State, press: Press) -> bool {
    match press {
        Press::Start => {
            state.hook_connected && (state.is_virtual || !routing(state) || !state.saved)
        }
        Press::Stop => state.hook_connected && routing(state),
        // Not the agent's business and not the hook's: the store's. An engine that is
        // not there does not stop the user saving what they have edited.
        Press::Save | Press::Undo => !state.saved,
        // `CanLiveUpdate`: `!dead && !virtualLayout`. A foreign layout is never previewed
        // into the local mouse — it describes someone else's desk.
        Press::Live => state.hook_connected && !state.is_virtual,
    }
}

/// The engine has the mice, whether or not it is standing aside just now.
fn routing(state: &State) -> bool {
    matches!(state.engine, Engine::Running | Engine::Paused)
}

/// What a press is called on the button that makes it.
pub fn label(press: Press) -> &'static str {
    match press {
        Press::Start => "Start",
        Press::Stop => "Stop",
        Press::Save => "Save",
        Press::Undo => "Undo",
        Press::Live => "Live",
    }
}

/// The bottom bar, as a function of the state. Returns what the user pressed.
pub fn bottom_bar(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let mut pressed = None;
    ui.horizontal(|ui| {
        ui.label(match state.engine {
            Engine::Running => "Running",
            Engine::Stopped => "Stopped",
            Engine::Paused => "Paused",
            Engine::Dead => "No engine",
        });
        // Save and Undo first, as the window has them: what you do to the layout comes
        // before what you do to the engine.
        for press in [Press::Save, Press::Undo, Press::Start, Press::Stop] {
            if ui
                .add_enabled(
                    !state.waiting && can(state, press),
                    egui::Button::new(label(press)),
                )
                .clicked()
            {
                pressed = Some(Action::Pressed(press));
            }
        }
        // A mode, not a command, so it is drawn as one — lit while it is on.
        //
        // **The tooltip is not decoration.** A preview is a `Load` *and* a `Run`: turning
        // this on hands the layout being edited to the engine and lets it move the
        // cursor. In the Avalonia app that sits next to an Apply button, where the user
        // came to apply something; here it is one toggle among four, and the consequence
        // has to be readable before it is pressed rather than felt afterwards.
        if ui
            .add_enabled(
                can(state, Press::Live),
                egui::Button::selectable(state.live, label(Press::Live)),
            )
            .on_hover_text(
                "Feed each edit to the mouse engine as you make it, so the layout can be \
                 felt before it is kept. This runs the engine: the cursor follows the \
                 layout you are editing.",
            )
            .on_disabled_hover_text(if state.is_virtual {
                "This layout belongs to another machine: it is never fed to the local mouse."
            } else {
                "No engine to preview into."
            })
            .clicked()
        {
            pressed = Some(Action::Pressed(Press::Live));
        }
    });
    pressed
}

/// The engine controls alone, kept as the name the first tests knew.
pub fn engine_controls(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    bottom_bar(ui, state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> State {
        State {
            engine: Engine::Stopped,
            hook_connected: true,
            waiting: false,
            saved: true,
            is_virtual: false,
            live: false,
        }
    }

    #[test]
    fn a_press_asks_once_and_then_waits() {
        let mut state = ready();

        assert_eq!(
            update(&mut state, Action::Pressed(Press::Start)),
            [Effect::Ask(Press::Start)]
        );
        // The second press, while the first is still out, asks nothing.
        assert!(update(&mut state, Action::Pressed(Press::Start)).is_empty());

        update(&mut state, Action::Answered);
        assert_eq!(
            update(&mut state, Action::Pressed(Press::Start)),
            [Effect::Ask(Press::Start)]
        );
    }

    #[test]
    fn nothing_can_be_asked_of_an_agent_with_no_hook() {
        let mut state = State {
            hook_connected: false,
            ..ready()
        };
        assert!(update(&mut state, Action::Pressed(Press::Start)).is_empty());
        assert!(!state.waiting, "refusing is not waiting");
    }

    /// Paused is the engine standing aside for an excluded application. It is running,
    /// so Stop is what applies to it.
    #[test]
    fn paused_is_stopped_not_started() {
        let state = State {
            engine: Engine::Paused,
            ..ready()
        };
        assert!(!can(&state, Press::Start));
        assert!(can(&state, Press::Stop));
    }

    /// The rule that is easy to get wrong, and that I did get wrong before reading the
    /// Avalonia one: Start is not "the engine is stopped". An edit reaches the engine by
    /// being started again, so a running engine with unsaved changes offers Start.
    #[test]
    fn an_edited_layout_can_be_started_over_a_running_engine() {
        let running = State {
            engine: Engine::Running,
            ..ready()
        };
        assert!(!can(&running, Press::Start), "nothing to re-apply");

        let edited = State {
            saved: false,
            ..running
        };
        assert!(can(&edited, Press::Start), "this is how an edit is applied");
    }

    /// A foreign layout is loaded to be looked at, never run here, so sending it again
    /// costs nothing and is always offered.
    #[test]
    fn a_foreign_layout_can_always_be_sent_again() {
        let simulating = State {
            engine: Engine::Running,
            is_virtual: true,
            ..ready()
        };
        assert!(can(&simulating, Press::Start));
    }

    /// Saving is the store's business. An agent with no hook does not stop the user
    /// keeping what they have edited — losing an edit because a daemon is missing would
    /// be the worst of both.
    #[test]
    fn what_can_be_saved_does_not_depend_on_the_engine() {
        let edited_and_alone = State {
            engine: Engine::Dead,
            hook_connected: false,
            saved: false,
            ..ready()
        };
        assert!(can(&edited_and_alone, Press::Save));
        assert!(can(&edited_and_alone, Press::Undo));
        assert!(!can(&edited_and_alone, Press::Start), "there is no engine");
    }

    #[test]
    fn a_bar_that_opens_offers_neither_save_nor_undo() {
        let fresh = State::default();
        assert!(!can(&fresh, Press::Save), "nothing has been edited");
        assert!(!can(&fresh, Press::Undo), "nothing to throw away");
    }

    #[test]
    fn a_state_this_version_does_not_know_is_not_guessed() {
        assert_eq!(Engine::from_agent("Levitating"), Engine::Dead);
        assert_eq!(Engine::from_agent("Running"), Engine::Running);
    }
}
