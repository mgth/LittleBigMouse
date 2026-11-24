//! The multi-PC mode in the window (docs/multi-pc.md, lot 7): the Machines page of the
//! settings drawer, and the switch of a panel two machines know.
//!
//! The page is a view of the agent's `Peers` state and nothing else. It keeps no list of
//! its own: what it shows is what the agent last said, and what the user does comes back
//! as a [`Did`] for the caller to send. That is what keeps it honest about pairing — a
//! machine is accepted when the agent says it is, not when a button was pressed.
//!
//! Pairing is trust on first use, confirmed on both machines against the fingerprint
//! (docs/multi-pc.md, "Network"). So the fingerprint is printed the same way on every
//! machine, in groups a person can read out to someone across the room, and a pending
//! machine's own fingerprint is shown right beside its Accept.

use crate::settings_icons::Icon;

/// Which end of the input stream a machine is on its link with this one, as the agent
/// says it (`Role` in the state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// It drives this machine's screens with its own mouse and keyboard.
    Master,
    /// Its screens are on this machine's map.
    Remote,
}

/// One machine the agent knows of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// Its certificate's fingerprint, lowercase hex.
    pub machine_id: String,
    pub name: String,
    pub address: String,
    pub connected: bool,
    pub role: Role,
    /// Connected and unknown: waiting for this user to accept or refuse it.
    pub pending: bool,
    /// How many screens it reported.
    pub displays: usize,
    /// What went wrong with the link, in words, while it stands.
    pub problem: Option<String>,
    /// It receives this machine's mouse and keyboard now: the pointer crossed to it.
    pub driven: bool,
}

/// The agent's multi-PC state (`Peers`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub enabled: bool,
    pub port: u16,
    /// What this machine is called on the others.
    pub name: String,
    /// This machine's fingerprint; `None` before the mode was first turned on, since no
    /// identity is made before that.
    pub machine_id: Option<String>,
    pub listen_address: Option<String>,
    /// Why the mode is not fully working, when it is not.
    pub error: Option<String>,
    /// The shortcut that locks the pointer on the machine it is on; empty for none.
    pub pointer_lock_shortcut: String,
    /// The pointer is locked on the machine it is on.
    pub pointer_locked: bool,
    pub peers: Vec<Peer>,
}

/// What the user is typing on the page, kept between frames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Panel {
    /// The address to connect to.
    pub address: String,
    /// This machine's name while it is being edited; `None` shows the agent's.
    pub name: Option<String>,
    /// The pointer lock's shortcut is being recorded.
    pub recording_lock: bool,
}

/// What the user asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Did {
    /// `SetMultiPc { Enabled }`.
    Enable(bool),
    /// `SetMultiPc { Enabled, Name }`, the mode left as it is.
    Rename(String),
    /// `SetMultiPc { Enabled, PointerLockShortcut }`, the mode left as it is; empty
    /// for none.
    LockShortcut(String),
    /// `PeerConnect { Address }`.
    Connect(String),
    /// `PeerAccept { MachineId }`.
    Accept(String),
    /// `PeerRefuse { MachineId }`.
    Refuse(String),
    /// `PeerForget { MachineId }`.
    Forget(String),
}

/// A fingerprint as people compare it: upper case, in groups of four, eight groups to a
/// line. The two machines run this same function, so what one prints the other prints.
pub fn fingerprint(machine_id: &str) -> String {
    let digits: Vec<char> = machine_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let groups: Vec<String> = digits
        .chunks(4)
        .map(|group| group.iter().collect())
        .collect();
    groups
        .chunks(8)
        .map(|line| line.join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The fingerprint, printed so that it can be read and selected, under an accessible
/// name that says whose it is.
fn printed_fingerprint(ui: &mut egui::Ui, whose: &str, machine_id: &str) {
    let text = fingerprint(machine_id);
    let response = ui.add(
        egui::Label::new(egui::RichText::new(&text).monospace())
            .selectable(true)
            .wrap_mode(egui::TextWrapMode::Extend),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            ui.is_enabled(),
            format!("{whose} {text}"),
        )
    });
}

/// Which machine the mouse and keyboard go to, in words: said once a remote is linked,
/// the only time it can be anything but this machine.
pub fn input_goes_to(state: &State) -> Option<String> {
    if !state.enabled {
        return None;
    }
    if let Some(peer) = state.peers.iter().find(|peer| peer.driven) {
        let name = if peer.name.is_empty() {
            "an unnamed machine"
        } else {
            peer.name.as_str()
        };
        return Some(locked(state, format!("The mouse and keyboard are on {name}")));
    }
    state
        .peers
        .iter()
        .any(|peer| peer.role == Role::Remote && peer.connected && !peer.pending)
        .then(|| locked(state, "The mouse and keyboard are on this machine".to_owned()))
}

/// `words`, and that the pointer stays there when it is locked.
fn locked(state: &State, words: String) -> String {
    if state.pointer_locked {
        format!("{words}, locked there until the shortcut is pressed again")
    } else {
        words
    }
}

/// The pointer lock's shortcut: shown, recorded, taken away. Returns what the user
/// chose, empty for none.
fn lock_shortcut(ui: &mut egui::Ui, shortcut: &str, recording: &mut bool) -> Option<String> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        let label = if *recording {
            "Press a combination…"
        } else if shortcut.is_empty() {
            "None"
        } else {
            shortcut
        };
        let button = ui
            .add(egui::Button::selectable(*recording, label))
            .on_hover_text("Click, then press the combination; Escape leaves it as it is.");
        button.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("Pointer lock shortcut: {label}"),
            )
        });
        if button.clicked() {
            *recording = !*recording;
        }
        ui.label("Pointer lock shortcut");
        if !shortcut.is_empty() && ui.button("No shortcut").clicked() {
            *recording = false;
            chosen = Some(String::new());
        }
    });
    if *recording {
        let pressed = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*modifiers, *key)),
                _ => None,
            })
        });
        if let Some((modifiers, key)) = pressed {
            if key == egui::Key::Escape && modifiers.is_none() {
                *recording = false;
            } else {
                // A bare key on the way to the combination is not the answer.
                let spelled = crate::shortcut::spell(modifiers, key);
                if lbm_ipc::shortcut::Shortcut::parse(&spelled).is_some() {
                    *recording = false;
                    chosen = Some(spelled);
                }
            }
        }
    }
    crate::options::note(
        ui,
        "Locks the pointer on the machine it is on, and unlocks it. The rescue \
         shortcut always brings it home.",
    );
    chosen
}

/// The Machines page. `busy` greys the controls while a request is out; `error` is the
/// last request the agent refused.
pub fn panel(
    ui: &mut egui::Ui,
    state: Option<&State>,
    draft: &mut Panel,
    busy: bool,
    error: Option<&str>,
) -> Option<Did> {
    let mut did = None;
    crate::options::note(
        ui,
        "One mouse and keyboard for several computers · applied at once",
    );
    let Some(state) = state else {
        ui.label("Asking the agent…");
        return None;
    };
    ui.add_enabled_ui(!busy, |ui| {
        crate::options::group(ui, "This machine", |ui| {
            let mut enabled = state.enabled;
            if crate::options::toggle(
                ui,
                Icon::Virtual,
                "Multi-PC mode",
                "Share this mouse and keyboard with other machines, or be driven by one",
                &mut enabled,
            ) {
                did = Some(Did::Enable(enabled));
            }
            ui.separator();
            let shown = draft.name.get_or_insert_with(|| state.name.clone());
            let label = ui.label("Name");
            let edit = ui.add(
                egui::TextEdit::singleline(shown)
                    .hint_text("What the other machines call this one")
                    .desired_width(f32::INFINITY),
            );
            let edit = edit.labelled_by(label.id);
            let renamed = shown.trim().to_owned();
            let changed = !renamed.is_empty() && renamed != state.name;
            let entered = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add_enabled(changed, egui::Button::new("Rename"))
                .clicked()
                || (entered && changed)
            {
                did = Some(Did::Rename(renamed));
                draft.name = None;
            } else if !edit.has_focus() && !changed {
                // Not being edited: follow what the agent says rather than a stale copy.
                draft.name = None;
            }
            ui.separator();
            ui.label(egui::RichText::new("Fingerprint").strong());
            match &state.machine_id {
                Some(id) => printed_fingerprint(ui, "This machine's fingerprint", id),
                None => crate::options::note(ui, "Made when the mode is first turned on."),
            }
            if let Some(address) = &state.listen_address {
                ui.label(format!("Listening on {address}"));
            } else if state.enabled {
                ui.label("Not listening");
            }
            if let Some(why) = &state.error {
                ui.colored_label(ui.visuals().error_fg_color, why);
            }
            if let Some(whereabouts) = input_goes_to(state) {
                ui.label(whereabouts);
            }
            ui.separator();
            if let Some(shortcut) =
                lock_shortcut(ui, &state.pointer_lock_shortcut, &mut draft.recording_lock)
            {
                did = Some(Did::LockShortcut(shortcut));
            }
        });

        crate::options::group(ui, "Connect to a machine", |ui| {
            crate::options::note(
                ui,
                "The machine you connect to becomes a remote: its screens join this map.",
            );
            ui.horizontal(|ui| {
                let label = ui.label("Address");
                let field = ui.add(
                    egui::TextEdit::singleline(&mut draft.address)
                        .hint_text("host, host:port or [v6]:port")
                        .desired_width(220.0),
                );
                let field = field.labelled_by(label.id);
                let address = draft.address.trim().to_owned();
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let can = state.enabled && !address.is_empty();
                if ui.add_enabled(can, egui::Button::new("Connect")).clicked() || (entered && can) {
                    did = Some(Did::Connect(address));
                }
            });
            if !state.enabled {
                crate::options::note(ui, "Turn the mode on to connect.");
            }
        });

        crate::options::group(ui, "Machines", |ui| {
            if state.peers.is_empty() {
                crate::options::note(ui, "No machine yet.");
            }
            for (n, peer) in state.peers.iter().enumerate() {
                if n > 0 {
                    ui.separator();
                }
                if let Some(chosen) = peer_row(ui, peer) {
                    did = Some(chosen);
                }
            }
        });
    });
    if let Some(why) = error {
        ui.colored_label(ui.visuals().error_fg_color, why);
    }
    did
}

/// One machine: who it is, what it is to this one, and what can be done about it. Every
/// button carries the machine's name, so a list of them can be told apart by ear.
fn peer_row(ui: &mut egui::Ui, peer: &Peer) -> Option<Did> {
    let mut did = None;
    let name = if peer.name.is_empty() {
        "Unnamed machine"
    } else {
        peer.name.as_str()
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(name).strong());
        let (status, colour) = if peer.connected {
            ("Connected", crate::theme::ACCENT)
        } else {
            ("Disconnected", crate::theme::MUTED)
        };
        crate::theme::status(ui, status, colour);
        if peer.driven {
            crate::theme::status(ui, "Has the mouse and keyboard", crate::theme::ACCENT);
        }
    });
    let role = match (peer.role, peer.pending) {
        (_, true) => "Waiting for you to accept it",
        (Role::Remote, false) => "Remote: its screens are on this map",
        (Role::Master, false) => "Master: its mouse and keyboard drive this machine",
    };
    crate::options::note(ui, role);
    let mut whereabouts = Vec::new();
    if !peer.address.is_empty() {
        whereabouts.push(peer.address.clone());
    }
    if peer.role == Role::Remote && peer.connected {
        whereabouts.push(match peer.displays {
            1 => "1 screen".to_owned(),
            n => format!("{n} screens"),
        });
    }
    if !whereabouts.is_empty() {
        crate::options::note(ui, &whereabouts.join(" · "));
    }
    if let Some(problem) = &peer.problem {
        ui.colored_label(ui.visuals().error_fg_color, problem);
    }
    if peer.pending {
        ui.label("Check that the other machine shows this fingerprint:");
        printed_fingerprint(ui, &format!("Fingerprint of {name}"), &peer.machine_id);
        ui.horizontal(|ui| {
            let accept = ui.add(egui::Button::new("Accept"));
            accept.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    format!("Accept {name}"),
                )
            });
            if accept.clicked() {
                did = Some(Did::Accept(peer.machine_id.clone()));
            }
            let refuse = ui.add(egui::Button::new("Refuse"));
            refuse.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    format!("Refuse {name}"),
                )
            });
            if refuse.clicked() {
                did = Some(Did::Refuse(peer.machine_id.clone()));
            }
        });
    } else {
        let forget = ui.add(egui::Button::new("Forget"));
        forget.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("Forget {name}"),
            )
        });
        if forget
            .on_hover_text("Unpair: it will have to be accepted again")
            .clicked()
        {
            did = Some(Did::Forget(peer.machine_id.clone()));
        }
    }
    did
}

/// One of the machines a shared panel can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The source id, what `SetActiveSource` names.
    pub id: String,
    /// The machine it is on, as the user knows it.
    pub machine: String,
    /// Whether that machine has the panel in its desktop right now.
    pub attached: bool,
}

/// Which machine a panel both machines know is showing, and the buttons to show another.
/// Returns the source the user chose. `enabled` is false while the layout cannot be
/// changed (a reload pending, a foreign layout).
pub fn source_switch(
    ui: &mut egui::Ui,
    sources: &[Source],
    active: Option<&str>,
    enabled: bool,
) -> Option<String> {
    let mut chosen = None;
    let showing = sources.iter().find(|s| Some(s.id.as_str()) == active);
    ui.label(egui::RichText::new("Shared screen").strong());
    ui.label(match showing {
        Some(source) => format!("Showing {}", source.machine),
        None => "Showing no machine".to_owned(),
    });
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            for source in sources.iter().filter(|s| Some(s.id.as_str()) != active) {
                let button = ui
                    .add(egui::Button::new(format!("Show {}", source.machine)))
                    .on_hover_text(if source.attached {
                        "Make this screen show that machine"
                    } else {
                        "That machine does not have this screen in its desktop right now"
                    });
                if button.clicked() {
                    chosen = Some(source.id.clone());
                }
            }
        });
    });
    chosen
}

/// The note a local-only control shows instead of itself on a monitor another machine
/// is showing.
pub fn on_another_machine(ui: &mut egui::Ui, machine: &str) {
    ui.label(format!(
        "This screen is showing {machine}: its system settings, monitor controls and \
         wallpaper are reached on that machine. Its place, size, borders and resistance \
         are edited here."
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_says_where_the_mouse_and_keyboard_are() {
        let laptop = Peer {
            machine_id: "aa".into(),
            name: "laptop".into(),
            address: String::new(),
            connected: true,
            role: Role::Remote,
            pending: false,
            displays: 1,
            driven: false,
            problem: None,
        };
        let mut state = State {
            enabled: true,
            peers: vec![laptop.clone()],
            ..State::default()
        };
        assert_eq!(
            input_goes_to(&state).as_deref(),
            Some("The mouse and keyboard are on this machine")
        );
        state.peers[0].driven = true;
        assert_eq!(
            input_goes_to(&state).as_deref(),
            Some("The mouse and keyboard are on laptop")
        );
        state.pointer_locked = true;
        assert_eq!(
            input_goes_to(&state).as_deref(),
            Some("The mouse and keyboard are on laptop, locked there until the shortcut is pressed again")
        );
        state.peers[0].driven = false;
        assert_eq!(
            input_goes_to(&state).as_deref(),
            Some("The mouse and keyboard are on this machine, locked there until the shortcut is pressed again")
        );
        state.pointer_locked = false;
        // Nothing to say with no remote linked, or with the mode off.
        state.peers[0] = Peer {
            connected: false,
            ..laptop
        };
        assert_eq!(input_goes_to(&state), None);
        state.enabled = false;
        state.peers.clear();
        assert_eq!(input_goes_to(&state), None);
    }

    #[test]
    fn a_fingerprint_reads_in_groups_of_four_eight_to_a_line() {
        let id = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
        assert_eq!(
            fingerprint(id),
            "A1B2 C3D4 E5F6 0718 293A 4B5C 6D7E 8F90\nA1B2 C3D4 E5F6 0718 293A 4B5C 6D7E 8F90"
        );
        // A short or empty one is not an error: it is printed as it is.
        assert_eq!(fingerprint("abc"), "ABC");
        assert_eq!(fingerprint(""), "");
    }
}
