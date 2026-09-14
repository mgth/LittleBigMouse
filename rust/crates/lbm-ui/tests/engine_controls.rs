//! The spike phase 6 asks for before any screen: can the interface be driven and
//! asserted on with no window server?
//!
//! On this machine the Avalonia UI never could — `xdotool` cannot click under KWin, so
//! every check was a screenshot and a squint. `egui_kittest` drives widgets through the
//! accessibility tree: no window, no GPU, no compositor, and it runs in the same
//! `cargo test` as everything else.
//!
//! What these tests are really pinning is that a *view is a function of the state*: the
//! same button is there or greyed out purely because of what the agent last said.

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use lbm_ui::{can, engine_controls, update, Action, Effect, Engine, Press, State};

/// Drives the bar over one state and hands back what the press produced.
fn press(state: State, label: &str) -> (Vec<Effect>, State) {
    let mut state = state;
    let mut produced = None;
    {
        let mut harness = Harness::new_ui(|ui| {
            if let Some(action) = engine_controls(ui, &state) {
                produced = Some(action);
            }
        });
        harness.get_by_label(label).click();
        harness.run();
    }
    let effects = match produced {
        Some(action) => lbm_ui::update(&mut state, action),
        None => Vec::new(),
    };
    (effects, state)
}

fn stopped() -> State {
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
fn clicking_start_on_a_stopped_engine_asks_for_a_start() {
    let (effects, state) = press(stopped(), "Start");

    assert_eq!(effects, [Effect::Ask(Press::Start)]);
    assert!(state.waiting, "the bar waits for the answer");
    // And the engine is not moved by the press: only the agent says what it is doing.
    assert_eq!(state.engine, Engine::Stopped);
}

#[test]
fn clicking_stop_on_a_running_engine_asks_for_a_stop() {
    let running = State {
        engine: Engine::Running,
        ..stopped()
    };

    let (effects, _) = press(running, "Stop");

    assert_eq!(effects, [Effect::Ask(Press::Stop)]);
}

/// The button that means nothing is not merely ignored, it is unclickable — which is
/// the part only a driven interface can check. `can` says so, and the view obeys it;
/// this asserts the two agree where it matters, through the widget itself.
#[test]
fn a_button_that_means_nothing_cannot_be_pressed() {
    let running = State {
        engine: Engine::Running,
        ..stopped()
    };
    assert!(!can(&running, Press::Start));

    let harness = Harness::new_ui(|ui| {
        engine_controls(ui, &running);
    });
    let start = harness.get_by_label("Start");

    assert!(
        start.accesskit_node().is_disabled(),
        "Start over a running engine is greyed, not silently inert"
    );
}

/// With no hook to drive, nothing is offered at all — pressing would ask an agent that
/// has nobody to pass it on to.
#[test]
fn an_agent_with_no_hook_offers_nothing() {
    let alone = State {
        engine: Engine::Dead,
        hook_connected: false,
        waiting: false,
        saved: true,
        is_virtual: false,
        live: false,
    };

    let harness = Harness::new_ui(|ui| {
        engine_controls(ui, &alone);
    });

    assert!(harness.get_by_label("Start").accesskit_node().is_disabled());
    assert!(harness.get_by_label("Stop").accesskit_node().is_disabled());
    // And it says why, rather than showing a stopped engine that is not there.
    let _ = harness.get_by_label("No engine");
}

/// What the agent says is what the bar shows — the whole of "a view is a function of
/// the state".
#[test]
fn the_bar_shows_what_the_agent_last_said() {
    for (word, shown) in [
        ("Running", "Running"),
        ("Stopped", "Stopped"),
        ("Paused", "Paused"),
        ("Levitating", "No engine"),
    ] {
        let mut state = State::default();
        lbm_ui::update(
            &mut state,
            Action::AgentSaid {
                engine: Engine::from_agent(word),
                connected: true,
            },
        );

        let harness = Harness::new_ui(|ui| {
            engine_controls(ui, &state);
        });
        let _ = harness.get_by_label(shown);
    }
}

//==========================================================================//
// The live preview                                                         //
//==========================================================================//

/// Turning it on is an ask; turning it off ends the preview instead of asking again.
#[test]
fn the_live_switch_turns_on_with_an_ask_and_off_with_an_end() {
    let mut state = stopped();

    let effects = update(&mut state, Action::Pressed(Press::Live));
    assert!(state.live);
    assert_eq!(effects, vec![Effect::Ask(Press::Live)]);
    assert!(
        !state.waiting,
        "a mode is not a request with an answer; waiting would grey the whole bar"
    );

    let effects = update(&mut state, Action::Pressed(Press::Live));
    assert!(!state.live);
    assert_eq!(effects, vec![Effect::EndPreview]);
}

/// **The engine going down outranks the preview.** The tray's Stop, a display change, an
/// excluded application: without this the next tick would hand the hook a `Load` and a
/// `Run` and put it straight back up, against a user who just stopped it.
#[test]
fn an_engine_that_goes_down_ends_the_preview() {
    let mut state = stopped();
    update(&mut state, Action::Pressed(Press::Live));
    // It is running now, previewing.
    update(
        &mut state,
        Action::AgentSaid {
            engine: Engine::Running,
            connected: true,
        },
    );
    assert!(state.live, "a preview that started the engine stays on");

    let effects = update(
        &mut state,
        Action::AgentSaid {
            engine: Engine::Stopped,
            connected: true,
        },
    );
    assert!(!state.live, "the switch stayed on over a stopped engine");
    assert_eq!(effects, vec![Effect::EndPreview]);
}

/// A transition, not the current state: over an engine that is *already* stopped,
/// turning the switch on is a legitimate way to start — and must not be undone at once
/// by the next state the agent sends.
#[test]
fn a_preview_started_over_a_stopped_engine_survives_the_next_state() {
    let mut state = stopped();
    update(&mut state, Action::Pressed(Press::Live));
    assert!(state.live);

    // The agent has not caught up yet and repeats "stopped".
    let effects = update(
        &mut state,
        Action::AgentSaid {
            engine: Engine::Stopped,
            connected: true,
        },
    );
    assert!(
        state.live,
        "the switch turned itself off on a state that did not change"
    );
    assert!(effects.is_empty());
}

/// A hook that goes away leaves nothing to preview into.
#[test]
fn losing_the_hook_ends_the_preview() {
    let mut state = stopped();
    update(&mut state, Action::Pressed(Press::Live));

    let effects = update(
        &mut state,
        Action::AgentSaid {
            engine: Engine::Dead,
            connected: false,
        },
    );
    assert!(!state.live);
    assert_eq!(effects, vec![Effect::EndPreview]);
}

/// A foreign layout is never fed to the local mouse: it describes someone else's desk.
#[test]
fn a_foreign_layout_is_never_previewed() {
    let mut state = State {
        is_virtual: true,
        ..stopped()
    };
    assert!(!can(&state, Press::Live));
    let effects = update(&mut state, Action::Pressed(Press::Live));
    assert!(!state.live);
    assert!(effects.is_empty());
}
