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
        /// The desktop those zones sit on, in pixels, as the agent's own display
        /// enumeration sees it. Absent from an older agent, and then the hook falls
        /// back to what it can infer.
        ///
        /// It is here rather than in the document because it is a fact about the
        /// machine, not a property of the layout — and because the two must never
        /// get out of step: a display change rebuilds both and sends them together.
        #[serde(rename = "Desktop", default, skip_serializing_if = "Option::is_none")]
        desktop: Option<Desktop>,
        /// What this `Load` is known by: the hook names it in the [`Event::Loaded`] or
        /// [`Event::LoadFailed`] that answers it. Absent from an older agent, and then
        /// the answer names nothing.
        ///
        /// Without it an answer can only be matched to a `Load` by its rank, and the
        /// rank lies across a reconnection: a frame written to the new connection by an
        /// agent that had already counted it lost is answered there, and its answer
        /// taken for the next one's — the agent then believes the hook holds zones it
        /// does not, which is what the multi-PC mode waits on before it opens a border
        /// to another machine.
        ///
        /// No new [`PROTOCOL`]: both sides read a frame without it, and a hook that
        /// never echoes it is answered the way every hook was before.
        #[serde(rename = "Id", default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
    },
    /// The desktop this machine's screens make, in pixels, without a layout: what a
    /// hook that is not routing needs to inject a remote stream's positions (the
    /// multi-PC mode), when no [`Command::Load`] carried it. Arms nothing.
    Desktop {
        #[serde(rename = "Desktop")]
        desktop: Desktop,
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
    /// The processes to stand aside for: path fragments, as the exclusion list spells
    /// them. The hook keeps the matching and the decision — it is the only one that can
    /// ask who is in front at the instant it is about to grab, which is what #541 was —
    /// but the list is the agent's, who owns what the user edits.
    Excluded {
        #[serde(rename = "Processes")]
        processes: Vec<String>,
    },
    /// Whether this hook belongs to the agent that is speaking: on `true`, the end of
    /// this connection is the end of the hook — it lets go of the mice and leaves.
    ///
    /// Sent on every connection rather than carried by the layout, so it cannot go
    /// stale and so a hook adopted by a new agent is told afresh what that agent
    /// wants. Default (never sent) is `false`: D5 says a hook outlives its agent, and
    /// being bound is the option.
    BindToAgent {
        #[serde(rename = "Bound")]
        bound: bool,
    },
    /// The shortcut that locks the pointer on the machine it is on, and unlocks it
    /// (the hook's crossing rules, "The pointer lock"); empty when there is none, which
    /// is the default. Only a multi-PC master watches for it. Sent on every connection,
    /// and again when it changes: an older agent never sends it, and its hook then
    /// watches for nothing.
    PointerLock {
        #[serde(rename = "Shortcut")]
        shortcut: String,
    },
    /// Leave.
    Quit,
    /// A command this version does not know. Kept rather than refused: a frame is
    /// ignored as a whole otherwise, and one command from a newer agent must not
    /// silence the `Run` beside it.
    #[serde(other)]
    Unknown,
}

/// A rectangle of desktop pixels: where the desktop starts and how big it is.
///
/// The hook needs it for one thing the layout cannot tell it — the range of the
/// absolute pointing device it creates. A compositor maps that range onto the whole
/// desktop, so a range that is not the desktop is a cursor that lands somewhere else,
/// scaled by the ratio between the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Desktop {
    #[serde(rename = "Left")]
    pub left: i32,
    #[serde(rename = "Top")]
    pub top: i32,
    #[serde(rename = "Width")]
    pub width: i32,
    #[serde(rename = "Height")]
    pub height: i32,
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
    /// The answer to [`Command::Hello`]: who is there, and what it is applying.
    Hello {
        #[serde(rename = "Protocol")]
        protocol: u32,
        #[serde(rename = "Version")]
        version: String,
        /// [`fingerprint`] of the layout this hook holds; empty when it holds none.
        ///
        /// This is what makes reattachment (D5) more than a guess. A hook outlives
        /// its agent, so the agent that finds one running must decide whether to
        /// leave it alone — and leaving it alone is only right if it is applying the
        /// layout the agent wants. It is not, if the previous agent died mid-preview:
        /// the mice would keep being routed by an experiment nobody ever committed
        /// to, for as long as the session lasts, with nothing to say so.
        #[serde(rename = "Layout", default)]
        layout: String,
        /// What this hook reads beyond its protocol, by name: `InputLocks`
        /// (`crate::input::LOCKS_CAPABILITY`) when it reads the lock keys' state in the
        /// input stream. Absent from an older hook, which reads none of it: an agent
        /// that relays a remote's input to it strips what it would refuse.
        #[serde(
            rename = "Capabilities",
            default,
            skip_serializing_if = "Vec::is_empty"
        )]
        capabilities: Vec<String>,
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
        /// The [`Command::Load`]'s own `Id`, echoed; absent when it had none, or from a
        /// hook that predates it.
        #[serde(rename = "Id", default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
    },
    /// A `Load` was refused: the document did not parse.
    LoadFailed {
        /// The same as [`Event::Loaded`]'s.
        #[serde(rename = "Id", default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
    },
    /// The panic shortcut ran.
    Rescued,
    /// The panic shortcut is not armed, and why is the platform's own: something else
    /// owns the combination (Windows), or the desktop registered it with no key
    /// (Linux). Carries the shortcut, so whoever shows it can name it.
    ShortcutUnavailable {
        #[serde(rename = "Shortcut")]
        shortcut: String,
    },
    /// The panic shortcut is armed: a key answers it. Withdraws what
    /// [`Event::ShortcutUnavailable`] said about this shortcut.
    ShortcutAvailable {
        #[serde(rename = "Shortcut")]
        shortcut: String,
    },
    /// The pointer was locked on the machine it is on, or let go
    /// ([`Command::PointerLock`]). Said on every change, and never for a lock that was
    /// never taken: a hook that never says it holds none.
    PointerLocked {
        #[serde(rename = "Locked")]
        locked: bool,
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
            Event::LoadFailed { .. } => "LoadFailed",
            Event::Rescued => "Rescued",
            Event::ShortcutUnavailable { .. } => "ShortcutUnavailable",
            Event::ShortcutAvailable { .. } => "ShortcutAvailable",
            Event::PointerLocked { .. } => "PointerLocked",
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
                ..
            } => format!(
                "{zones} zones ({main} main){}",
                if *virtual_layout { ", virtual" } else { "" }
            ),
            Event::LoadFailed { .. } => "the layout could not be parsed".to_owned(),
            Event::ShortcutUnavailable { shortcut } | Event::ShortcutAvailable { shortcut } => {
                shortcut.clone()
            }
            Event::PointerLocked { locked } => {
                if *locked { "locked" } else { "unlocked" }.to_owned()
            }
            _ => String::new(),
        }
    }
}

/// What a layout document is known by on the wire, so that two processes can tell
/// whether they mean the same one without one of them sending it back.
///
/// FNV-1a, 64 bits, in hex. A fingerprint, not a signature: the two sides trust each
/// other (same user, same session, 0600 socket), so this only has to survive
/// accidents, and an accidental collision between two layouts a user actually has is
/// not a thing that happens. It is exact where it matters — the document is produced
/// deterministically by one side and hashed byte for byte, which is what already
/// makes the agent's own "is this the layout I last sent" comparison mean something.
///
/// Deliberately not shared with the wallpaper's file-name hash, which happens to use
/// the same function: that one names files on disk and can never change without
/// orphaning them, this one lives for the length of a connection.
pub fn fingerprint(zones: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = zones.bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    });
    format!("{hash:016x}")
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
                desktop: Some(Desktop {
                    left: 0,
                    top: 0,
                    width: 3840,
                    height: 1080,
                }),
                id: Some(7),
            },
            Command::Run,
        ]);

        assert_eq!(
            parse(&sent),
            [
                Command::Load {
                    zones: "<ZonesLayout/>".to_owned(),
                    desktop: Some(Desktop {
                        left: 0,
                        top: 0,
                        width: 3840,
                        height: 1080,
                    }),
                    id: Some(7),
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

        let [Command::Load { zones: back, .. }] = &parse(&frame(&[Command::Load {
            zones: zones.to_owned(),
            desktop: None,
            id: None,
        }]))[..] else {
            panic!("a Load came back");
        };

        assert_eq!(back, zones);
    }

    #[test]
    fn a_binding_reads_back_as_itself_and_its_absence_is_not_one() {
        assert_eq!(
            parse(&frame(&[Command::BindToAgent { bound: true }])),
            [Command::BindToAgent { bound: true }]
        );
        // An agent that never says binds nothing: the hook outlives it (D5), and only
        // an explicit `true` makes the connection's end the hook's end.
        assert_eq!(
            parse(&frame(&[Command::BindToAgent { bound: false }])),
            [Command::BindToAgent { bound: false }]
        );
    }

    #[test]
    fn an_exclusion_list_reads_back_as_itself_including_empty() {
        let list = vec![r"\steamapps\".to_owned(), "*EscapeFrom*".to_owned()];
        assert_eq!(
            parse(&frame(&[Command::Excluded {
                processes: list.clone()
            }])),
            [Command::Excluded { processes: list }]
        );
        // Emptying the list is a thing a user does, and it has to travel as itself
        // rather than as "nothing to say".
        assert_eq!(
            parse(&frame(&[Command::Excluded { processes: vec![] }])),
            [Command::Excluded { processes: vec![] }]
        );
    }

    #[test]
    fn a_pointer_lock_shortcut_reads_back_as_itself_including_none() {
        for shortcut in ["Ctrl+Alt+L", ""] {
            let command = Command::PointerLock {
                shortcut: shortcut.to_owned(),
            };
            assert_eq!(parse(&frame(std::slice::from_ref(&command))), [command]);
        }
    }

    #[test]
    fn an_unreadable_frame_carries_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("not json").is_empty());
        // An object is not a frame: a frame is always an array.
        assert!(parse("{\"Command\":\"Run\"}").is_empty());
    }

    #[test]
    fn a_load_without_a_desktop_is_still_a_load() {
        // What an agent built before the field sends. It must not become an unreadable
        // frame — the hook would ignore the Run beside it and the engine would never
        // start — and it must not become a desktop of zeroes either.
        assert_eq!(
            parse(r#"[{"Command":"Load","Zones":"<ZonesLayout/>"}]"#),
            [Command::Load {
                zones: "<ZonesLayout/>".to_owned(),
                desktop: None,
                id: None,
            }]
        );
    }

    #[test]
    fn a_desktop_survives_the_frame_with_its_origin() {
        // A screen to the left of the primary puts the desktop origin at a negative
        // x. Losing that sign moves every cursor position by the width of a screen.
        let sent = frame(&[Command::Load {
            zones: "<ZonesLayout/>".to_owned(),
            desktop: Some(Desktop {
                left: -1920,
                top: -120,
                width: 5760,
                height: 1200,
            }),
            id: None,
        }]);

        let [Command::Load { desktop, .. }] = &parse(&sent)[..] else {
            panic!("a Load came back");
        };
        assert_eq!(
            *desktop,
            Some(Desktop {
                left: -1920,
                top: -120,
                width: 5760,
                height: 1200
            })
        );
    }

    #[test]
    fn a_desktop_alone_goes_through_the_wire() {
        let desktop = Desktop {
            left: 0,
            top: 0,
            width: 5388,
            height: 1728,
        };
        assert_eq!(
            parse(&frame(&[Command::Desktop { desktop }])),
            [Command::Desktop { desktop }]
        );
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
            id: Some(1),
        };
        assert_eq!(loaded.name(), "Loaded");
        assert_eq!(loaded.payload(), "2 zones (2 main)");

        let foreign = Event::Loaded {
            zones: 1,
            main: 1,
            virtual_layout: true,
            id: None,
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
                layout: fingerprint("<ZonesLayout/>"),
                capabilities: vec![crate::input::LOCKS_CAPABILITY.to_owned()],
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
                id: Some(u64::MAX),
            },
            Event::Loaded {
                zones: 1,
                main: 1,
                virtual_layout: false,
                id: None,
            },
            Event::LoadFailed { id: Some(42) },
            Event::LoadFailed { id: None },
            Event::Rescued,
            Event::ShortcutUnavailable {
                shortcut: "Ctrl+Alt+Shift+M".to_owned(),
            },
            Event::ShortcutAvailable {
                shortcut: "Ctrl+Alt+Shift+M".to_owned(),
            },
            Event::PointerLocked { locked: true },
            Event::PointerLocked { locked: false },
        ] {
            assert_eq!(parse_event(&event(&original)), Some(original.clone()));
        }
    }

    #[test]
    fn the_same_layout_fingerprints_the_same_and_a_different_one_does_not() {
        let zones = "<ZonesLayout Algorithm=\"Strait\"><Zone Id=\"0\"/></ZonesLayout>";
        assert_eq!(fingerprint(zones), fingerprint(zones));
        assert_ne!(
            fingerprint(zones),
            fingerprint("<ZonesLayout Algorithm=\"Cross\"><Zone Id=\"0\"/></ZonesLayout>")
        );
        // Byte for byte: a layout that moved a screen by a millimetre is a different
        // layout, and the whole point is to notice.
        assert_ne!(
            fingerprint("<Rect Left=\"0\"/>"),
            fingerprint("<Rect Left=\"1\"/>")
        );
        // Fixed width, so it reads as one thing in a log line.
        assert_eq!(fingerprint("").len(), 16);
    }

    #[test]
    fn a_hook_that_names_no_layout_still_says_hello() {
        // The field arrived after PROTOCOL 1 was written down. A hook built before it
        // omits it, and omitting it must mean "no layout", not "unreadable greeting" —
        // the agent would retire a hook it can perfectly well talk to.
        assert_eq!(
            parse_event(r#"{"Event":"Hello","Protocol":1,"Version":"0.1.0"}"#),
            Some(Event::Hello {
                protocol: 1,
                version: "0.1.0".to_owned(),
                layout: String::new(),
                capabilities: Vec::new(),
            })
        );
    }

    #[test]
    fn a_load_answer_names_the_load_it_answers_and_an_older_one_names_none() {
        // The id goes out with the Load and comes back with its answer...
        let [Command::Load { id, .. }] = &parse(&frame(&[Command::Load {
            zones: "<ZonesLayout/>".to_owned(),
            desktop: None,
            id: Some(12),
        }]))[..] else {
            panic!("a Load came back");
        };
        assert_eq!(*id, Some(12));
        // ...and an answer from a hook that predates it is still an answer, naming
        // nothing: the agent falls back to taking answers in order.
        assert_eq!(
            parse_event(r#"{"Event":"Loaded","Zones":2,"Main":2,"Virtual":false}"#),
            Some(Event::Loaded {
                zones: 2,
                main: 2,
                virtual_layout: false,
                id: None,
            })
        );
        assert_eq!(
            parse_event(r#"{"Event":"LoadFailed"}"#),
            Some(Event::LoadFailed { id: None })
        );
        // Nor does a Load without one say it has one.
        assert!(!frame(&[Command::Load {
            zones: String::new(),
            desktop: None,
            id: None,
        }])
        .contains("\"Id\""));
    }

    #[test]
    fn a_frame_that_is_not_an_event_is_not_one() {
        assert_eq!(parse_event(""), None);
        assert_eq!(parse_event("[{\"Command\":\"Run\"}]"), None);
    }
}
