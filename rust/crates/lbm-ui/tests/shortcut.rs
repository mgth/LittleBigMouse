//! Recording a rescue shortcut, through the real widget.
//!
//! The spelling and the refusals are checked beside the code. What is asked here is what
//! they cannot: that pressing a combination while recording is what produces a shortcut,
//! and — the part a wrong recorder gets wrong invisibly — that pressing a **modifier on
//! the way there** does not end the recording with half a combination.

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use lbm_ui::shortcut::{recorder, Recording};

struct State {
    shortcut: String,
    recording: Recording,
    recorded: Vec<String>,
}

fn harness(state: &std::cell::RefCell<State>) -> Harness<'_> {
    Harness::builder()
        .with_size(egui::vec2(520.0, 300.0))
        .build_ui(move |ui| {
            let mut state = state.borrow_mut();
            let (shortcut, unavailable) = (state.shortcut.clone(), None);
            if let Some(new) = recorder(ui, &shortcut, &mut state.recording, unavailable) {
                state.shortcut = new.clone();
                state.recorded.push(new);
            }
        })
}

fn press(harness: &Harness, key: egui::Key, modifiers: egui::Modifiers) {
    harness.event(egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
}

fn fresh() -> std::cell::RefCell<State> {
    std::cell::RefCell::new(State {
        shortcut: lbm_ipc::shortcut::DEFAULT.to_owned(),
        recording: Recording::No,
        recorded: Vec::new(),
    })
}

#[test]
fn a_combination_pressed_while_recording_becomes_the_shortcut() {
    let state = fresh();
    let mut harness = harness(&state);
    harness.run();

    harness.get_by_label(lbm_ipc::shortcut::DEFAULT).click();
    harness.run();
    assert_eq!(state.borrow().recording, Recording::Yes);

    press(
        &harness,
        egui::Key::F9,
        egui::Modifiers {
            ctrl: true,
            alt: true,
            ..Default::default()
        },
    );
    harness.run();

    assert_eq!(state.borrow().recorded, vec!["Ctrl+Alt+F9".to_owned()]);
    assert_eq!(
        state.borrow().recording,
        Recording::No,
        "the recorder stayed open after taking a shortcut"
    );
}

/// **A modifier on its own does not end the recording.** Reaching Ctrl+Alt+M means
/// pressing Ctrl first; a recorder that took the first key event would store "Ctrl", a
/// shortcut the daemon refuses, and the user would think they had set one.
#[test]
fn the_modifiers_pressed_on_the_way_are_not_the_answer() {
    let state = fresh();
    let mut harness = harness(&state);
    harness.run();
    harness.get_by_label(lbm_ipc::shortcut::DEFAULT).click();
    harness.run();

    // Ctrl alone, then Ctrl+Shift, then the real thing.
    let ctrl = egui::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    press(&harness, egui::Key::M, egui::Modifiers::default());
    harness.run();
    assert!(
        state.borrow().recorded.is_empty(),
        "a bare key was taken for a shortcut: it would be registered globally"
    );
    assert_eq!(state.borrow().recording, Recording::Yes, "still waiting");

    press(&harness, egui::Key::M, ctrl);
    harness.run();
    assert_eq!(state.borrow().recorded, vec!["Ctrl+M".to_owned()]);
}

/// Escape is the way out that changes nothing — every other key is a candidate, so
/// without it there is no way to stop recording except by setting something.
#[test]
fn escape_leaves_the_recorder_without_setting_anything() {
    let state = fresh();
    let mut harness = harness(&state);
    harness.run();
    harness.get_by_label(lbm_ipc::shortcut::DEFAULT).click();
    harness.run();

    press(&harness, egui::Key::Escape, egui::Modifiers::default());
    harness.run();

    assert_eq!(state.borrow().recording, Recording::No);
    assert!(state.borrow().recorded.is_empty());
    assert_eq!(state.borrow().shortcut, lbm_ipc::shortcut::DEFAULT);
}

/// What the hook says is shown even when the text is perfectly well formed: being well
/// formed and being armed are two different facts, and only the hook knows the second.
#[test]
fn a_well_formed_shortcut_the_hook_could_not_arm_still_says_so() {
    let recording = std::cell::RefCell::new(Recording::No);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(520.0, 300.0))
        .build_ui(|ui| {
            recorder(
                ui,
                lbm_ipc::shortcut::DEFAULT,
                &mut recording.borrow_mut(),
                Some(lbm_ipc::shortcut::DEFAULT),
            );
        });
    harness.run();

    let said = harness
        .query_by_label_contains("could not be armed")
        .is_some();
    assert!(
        said,
        "a shortcut nothing armed looked fine: the user finds out when they need it"
    );
}
