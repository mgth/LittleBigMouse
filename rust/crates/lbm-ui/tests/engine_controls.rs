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
use lbm_ui::{can, engine_controls, Action, Effect, Engine, Press, State};

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
