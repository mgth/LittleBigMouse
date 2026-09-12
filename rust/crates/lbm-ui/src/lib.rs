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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub engine: Engine,
    /// The agent has a hook to drive. Without one, asking it to start is asking for
    /// nothing to happen.
    pub hook_connected: bool,
    /// A request is out and its answer has not come back. The agent is a socket away,
    /// so this is not instantaneous and the user must not be left pressing again.
    pub waiting: bool,
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
}

/// What the caller has to go and do, off the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Ask(Press),
}

/// The whole of the decision, with no window, no socket and no clock in it.
///
/// A press is refused rather than queued while one is already out: the agent answers in
/// order, and a second Start on the way to a hook that is already starting is a race
/// the user did not ask for.
pub fn update(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::Pressed(press) => {
            if state.waiting || !can(state, press) {
                return Vec::new();
            }
            state.waiting = true;
            vec![Effect::Ask(press)]
        }
        Action::AgentSaid { engine, connected } => {
            state.engine = engine;
            state.hook_connected = connected;
            Vec::new()
        }
        Action::Answered => {
            state.waiting = false;
            Vec::new()
        }
    }
}

/// Whether a press means anything in this state — which is also what greys the button.
pub fn can(state: &State, press: Press) -> bool {
    if !state.hook_connected {
        return false;
    }
    match press {
        // Paused is running, standing aside for an excluded application: starting again
        // is not the answer to it, and the hook would refuse anyway.
        Press::Start => matches!(state.engine, Engine::Stopped),
        Press::Stop => matches!(state.engine, Engine::Running | Engine::Paused),
    }
}

/// The engine controls, as a function of the state. Returns what the user pressed.
pub fn engine_controls(ui: &mut egui::Ui, state: &State) -> Option<Action> {
    let mut pressed = None;
    ui.horizontal(|ui| {
        ui.label(match state.engine {
            Engine::Running => "Running",
            Engine::Stopped => "Stopped",
            Engine::Paused => "Paused",
            Engine::Dead => "No engine",
        });
        if ui
            .add_enabled(can(state, Press::Start), egui::Button::new("Start"))
            .clicked()
        {
            pressed = Some(Action::Pressed(Press::Start));
        }
        if ui
            .add_enabled(can(state, Press::Stop), egui::Button::new("Stop"))
            .clicked()
        {
            pressed = Some(Action::Pressed(Press::Stop));
        }
    });
    pressed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> State {
        State {
            engine: Engine::Stopped,
            hook_connected: true,
            waiting: false,
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
    /// so Stop is what applies to it — pressing Start would ask the hook for something
    /// it refuses anyway.
    #[test]
    fn paused_is_stopped_not_started() {
        let state = State {
            engine: Engine::Paused,
            ..ready()
        };
        assert!(!can(&state, Press::Start));
        assert!(can(&state, Press::Stop));
    }

    #[test]
    fn a_state_this_version_does_not_know_is_not_guessed() {
        assert_eq!(Engine::from_agent("Levitating"), Engine::Dead);
        assert_eq!(Engine::from_agent("Running"), Engine::Running);
    }
}
