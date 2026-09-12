//! What the agent asks the hook, and what the hook says back — as types.
//!
//! Both sides are Rust now (v6): the C# UI spoke this protocol until phase 4 moved it
//! behind the agent's own API, and an XML envelope hand-written on one side and parsed
//! with a DOM on the other was the price of that. The envelope is JSON and the
//! vocabulary is an enum, so the compiler carries what a golden file used to.
//!
//! A frame from the agent is an **array** of commands: `Load` and `Run` travel together
//! and the hook must not take the hook down between them. A frame from the hook is one
//! event.
//!
//! The frames themselves are [`crate::framing`]'s — a length, then UTF-8 — which is
//! what makes JSON safe to put in them without a delimiter to escape.

use serde::{Deserialize, Serialize};

/// Bumped whenever the two sides must agree on something new. The agent asks for it
/// with [`Command::Hello`] and relaunches a hook that answers something else: a hook
/// outlives its agent (D5), so an upgrade can leave an old one holding the mice.
pub const PROTOCOL: u32 = 1;

/// What the agent asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "Command")]
pub enum Command {
    /// Who is there, and what it speaks. Answered with [`Event::Hello`].
    Hello {
        #[serde(rename = "Protocol")]
        protocol: u32,
    },
    /// Send me every event from now on.
    Listen,
    /// Take this layout: a `<ZonesLayout>` document, as the engine reads it.
    Load {
        #[serde(rename = "Zones")]
        zones: String,
    },
    /// Install the hook and route.
    Run,
    /// Unhook.
    Stop,
    /// Answer with the current state.
    State,
    /// Adopt this panic shortcut now, without waiting for a layout to carry it.
    /// Recording one in the options has to take effect there and then — and has to
    /// say so when the combination is already owned by something else.
    Shortcut {
        #[serde(rename = "Text")]
        text: String,
    },
    /// Leave.
    Quit,
    /// A command this version does not know. Kept rather than refused: a frame is
    /// ignored as a whole otherwise, and one command from a newer agent must not
    /// silence the `Run` beside it.
    #[serde(other)]
    Unknown,
}

/// `Quit`, spelled the way a hook that predates this protocol reads it.
///
/// It exists for one moment: a hook outlives its agent (D5), so an upgrade can leave
/// one running that speaks XML and understands nothing said here. The agent retires
/// it with this and launches one of its own rather than leaving the mice held by a
/// process nothing can talk to. Frozen: it is not a protocol, it is a farewell.
pub const LEGACY_QUIT: &str = r#"<CommandMessage Command="Quit" Payload=""></CommandMessage>"#;

/// What the hook says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "Event")]
pub enum Event {
    /// The answer to [`Command::Hello`].
    Hello {
        #[serde(rename = "Protocol")]
        protocol: u32,
        #[serde(rename = "Version")]
        version: String,
    },
    /// The input hook is installed.
    Running,
    /// The input hook is down.
    Stopped,
    /// Hooked but standing aside (an excluded application has the focus).
    Paused,
    /// Never on the wire: what a client raises for itself when the hook it was
    /// talking to is gone. It lives here because it is the same vocabulary the rest
    /// of the program reasons in.
    Dead,
    /// A system setting changed.
    SettingsChanged,
    DisplayChanged,
    DesktopChanged,
    /// The foreground application changed; empty when it cannot be resolved, which
    /// means "unknown", not "nobody".
    FocusChanged {
        #[serde(rename = "Process")]
        process: String,
    },
    /// The display turned off; the hook unhooked itself.
    Suspended,
    /// The display is back.
    Resumed,
    /// A `Load` was accepted. What it says about the layout is informative; the event
    /// itself is the success signal, which is what makes a Load without a Run
    /// observable (the foreign-layout flow gets no Running to wait for).
    Loaded {
        #[serde(rename = "Zones")]
        zones: usize,
        #[serde(rename = "Main")]
        main: usize,
        #[serde(rename = "Virtual")]
        virtual_layout: bool,
    },
    /// A `Load` was refused: the document did not parse.
    LoadFailed,
    /// The panic shortcut ran.
    Rescued,
    /// The panic shortcut is not armed, and why is the platform's own: something else
    /// owns the combination (Windows), or the desktop registered it with no key
    /// (Linux). Carries the shortcut, so whoever shows it can name it.
    ShortcutUnavailable {
        #[serde(rename = "Shortcut")]
        shortcut: String,
    },
    /// An event this version does not know. A newer hook must leave a client's idea
    /// of the state unchanged rather than move it to the wrong one, which is what
    /// mapping an unknown name onto a known event would do.
    #[serde(other)]
    Unknown,
}

impl Event {
    /// The name this event is known by across the program — the one C#'s
    /// `LittleBigMouseEvent` uses, and the one the agent forwards to its frontends.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Hello { .. } => "Hello",
            Event::Running => "Running",
            Event::Stopped => "Stopped",
            Event::Paused => "Paused",
            Event::Dead => "Dead",
            Event::SettingsChanged => "SettingsChanged",
            Event::DisplayChanged => "DisplayChanged",
            Event::DesktopChanged => "DesktopChanged",
            Event::FocusChanged { .. } => "FocusChanged",
            Event::Suspended => "Suspended",
            Event::Resumed => "Resumed",
            Event::Loaded { .. } => "Loaded",
            Event::LoadFailed => "LoadFailed",
            Event::Rescued => "Rescued",
            Event::ShortcutUnavailable { .. } => "ShortcutUnavailable",
            Event::Unknown => "Unknown",
        }
    }

    /// What this event has to say in words, for whoever shows it to a user. Empty
    /// when the event is the whole message.
    pub fn payload(&self) -> String {
        match self {
            Event::FocusChanged { process } => process.clone(),
            Event::Loaded {
                zones,
                main,
                virtual_layout,
            } => format!(
                "{zones} zones ({main} main){}",
                if *virtual_layout { ", virtual" } else { "" }
            ),
            Event::LoadFailed => "the layout could not be parsed".to_owned(),
            Event::ShortcutUnavailable { shortcut } => shortcut.clone(),
            _ => String::new(),
        }
    }
}

/// One frame's worth of commands, as the agent writes it.
///
/// Always an array, even for one command: a reader that has to tell an object from an
/// array is a reader with two shapes to get wrong.
pub fn frame(commands: &[Command]) -> String {
    serde_json::to_string(commands).unwrap_or_else(|_| "[]".to_owned())
}

/// The commands one frame carries. An unreadable frame carries none — the hook has
/// never had anything better to do with one than ignore it, and a client that can
/// make it send nonsense can make it send anything.
pub fn parse(frame: &str) -> Vec<Command> {
    serde_json::from_str(frame).unwrap_or_default()
}

/// One event, as the hook writes it.
pub fn event(event: &Event) -> String {
    serde_json::to_string(event).unwrap_or_default()
}

/// The event one frame carries, if it is one.
pub fn parse_event(frame: &str) -> Option<Event> {
    serde_json::from_str(frame).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_carries_its_commands_in_order() {
        // Load and Run travel together: what the hook does between them is the whole
        // reason a frame holds more than one command.
        let sent = frame(&[
            Command::Load {
                zones: "<ZonesLayout/>".to_owned(),
            },
            Command::Run,
        ]);

        assert_eq!(
            parse(&sent),
            [
                Command::Load {
                    zones: "<ZonesLayout/>".to_owned()
                },
                Command::Run
            ]
        );
    }

    #[test]
    fn a_zones_document_survives_the_frame_whatever_is_in_it() {
        // It is XML inside JSON: quotes, angle brackets and newlines all have to come
        // back exactly, or the engine reads a different layout than the agent sent.
        let zones =
            "<ZonesLayout Algorithm=\"Strait\">\n\t<Zone Name=\"A &amp; B\"/>\n</ZonesLayout>";

        let [Command::Load { zones: back }] = &parse(&frame(&[Command::Load {
            zones: zones.to_owned(),
        }]))[..] else {
            panic!("a Load came back");
        };

        assert_eq!(back, zones);
    }

    #[test]
    fn an_unreadable_frame_carries_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("not json").is_empty());
        // An object is not a frame: a frame is always an array.
        assert!(parse("{\"Command\":\"Run\"}").is_empty());
    }

    #[test]
    fn a_command_from_a_newer_agent_does_not_silence_the_one_beside_it() {
        // The Run has to survive the command this version has never heard of.
        assert_eq!(
            parse("[{\"Command\":\"Rewind\"},{\"Command\":\"Run\"}]"),
            [Command::Unknown, Command::Run]
        );
    }

    #[test]
    fn an_event_from_a_newer_hook_is_not_mistaken_for_a_known_one() {
        // Unknown, not dropped and not guessed: a client's idea of the state stays
        // where it was rather than moving to the wrong place.
        assert_eq!(
            parse_event("{\"Event\":\"Levitating\"}"),
            Some(Event::Unknown)
        );
    }

    #[test]
    fn an_event_says_its_name_and_its_words() {
        let loaded = Event::Loaded {
            zones: 2,
            main: 2,
            virtual_layout: false,
        };
        assert_eq!(loaded.name(), "Loaded");
        assert_eq!(loaded.payload(), "2 zones (2 main)");

        let foreign = Event::Loaded {
            zones: 1,
            main: 1,
            virtual_layout: true,
        };
        assert_eq!(foreign.payload(), "1 zones (1 main), virtual");

        assert_eq!(Event::Running.payload(), "");
        assert_eq!(
            Event::FocusChanged {
                process: "/usr/bin/kate".to_owned()
            }
            .payload(),
            "/usr/bin/kate"
        );
    }

    #[test]
    fn every_event_reads_back_as_itself() {
        for original in [
            Event::Hello {
                protocol: PROTOCOL,
                version: "0.1.0".to_owned(),
            },
            Event::Running,
            Event::Stopped,
            Event::Paused,
            Event::SettingsChanged,
            Event::DisplayChanged,
            Event::DesktopChanged,
            Event::FocusChanged {
                process: "C:\\Program Files\\Game\\game.exe".to_owned(),
            },
            Event::Suspended,
            Event::Resumed,
            Event::Loaded {
                zones: 3,
                main: 2,
                virtual_layout: true,
            },
            Event::LoadFailed,
            Event::Rescued,
            Event::ShortcutUnavailable {
                shortcut: "Ctrl+Alt+Shift+M".to_owned(),
            },
        ] {
            assert_eq!(parse_event(&event(&original)), Some(original.clone()));
        }
    }

    #[test]
    fn a_frame_that_is_not_an_event_is_not_one() {
        assert_eq!(parse_event(""), None);
        assert_eq!(parse_event("[{\"Command\":\"Run\"}]"), None);
    }
}
