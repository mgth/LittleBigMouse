//! The other mouse-and-keyboard sharing tools: what they are called, and what the
//! agent says when one runs beside LittleBigMouse.
//!
//! A Deskflow, Synergy, Barrier or Input Leap running on the same machine moves the
//! pointer to other computers on its own, whatever LittleBigMouse does: with the
//! engine stopped, the mouse still leaves the screen. Without a word from LittleBigMouse
//! that looks like a LittleBigMouse bug (an agent running twice, a hook left behind).
//! The agent finds them (`lbm_agent::input_sharing`), puts them in its state
//! (`Snapshot::input_sharing`), and the window and the tray say so in the words of
//! [`warnings`].
//!
//! Shared here because the agent writes the list and the frontends read it.

use serde::{Deserialize, Serialize};

/// A sharing tool, as the user knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tool {
    Deskflow,
    Synergy,
    Barrier,
    InputLeap,
}

impl Tool {
    /// Its name in a sentence.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Deskflow => "Deskflow",
            Tool::Synergy => "Synergy",
            Tool::Barrier => "Barrier",
            Tool::InputLeap => "Input Leap",
        }
    }
}

/// Which end of the tool's link a process is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// It sends this machine's mouse and keyboard to the others.
    Server,
    /// Another machine's mouse and keyboard drive this one through it.
    Client,
}

/// The port the four tools listen on unless told otherwise.
pub const DEFAULT_PORT: u16 = 24800;

/// One process of a sharing tool, as the agent found it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Running {
    pub tool: Tool,
    /// The executable's name, as the system lists it.
    pub process: String,
    pub pid: u32,
    /// `None` for a window of the tool (its GUI), or when nothing said which end it is.
    #[serde(default)]
    pub role: Option<Role>,
    /// It is the tool's window, which moves nothing itself: it starts and stops the
    /// process that does. Absent from the state (false) for the others.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub window: bool,
    /// It holds an established TCP connection: another machine is on the other end
    /// now. `None` when the agent could not tell (Linux: a process of another user,
    /// whose sockets it may not read, and no connection on [`DEFAULT_PORT`]).
    #[serde(default)]
    pub connected: Option<bool>,
}

/// The executables, lowercase and without `.exe`, with what their name alone says.
/// `deskflow-core` and `synergy-core` are told `server` or `client` on their command
/// line ([`role_from_args`]); `deskflow` and `synergy` are the windows that start them.
/// `deskflow-server` and `deskflow-client` are the names before `deskflow-core`. None
/// is longer than the 15 bytes of a Linux `comm`, so `comm` is compared whole.
const KNOWN: &[(&str, Known)] = &[
    ("deskflow-core", Known::core(Tool::Deskflow, None)),
    ("deskflow", Known::window(Tool::Deskflow)),
    (
        "deskflow-server",
        Known::core(Tool::Deskflow, Some(Role::Server)),
    ),
    (
        "deskflow-client",
        Known::core(Tool::Deskflow, Some(Role::Client)),
    ),
    ("synergy-core", Known::core(Tool::Synergy, None)),
    ("synergys", Known::core(Tool::Synergy, Some(Role::Server))),
    ("synergyc", Known::core(Tool::Synergy, Some(Role::Client))),
    ("barriers", Known::core(Tool::Barrier, Some(Role::Server))),
    ("barrierc", Known::core(Tool::Barrier, Some(Role::Client))),
    (
        "input-leaps",
        Known::core(Tool::InputLeap, Some(Role::Server)),
    ),
    (
        "input-leapc",
        Known::core(Tool::InputLeap, Some(Role::Client)),
    ),
];

/// What an executable's name says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub tool: Tool,
    /// The end its name says it is, if it says one.
    pub role: Option<Role>,
    /// The tool's window ([`Running::window`]).
    pub window: bool,
}

impl Known {
    const fn core(tool: Tool, role: Option<Role>) -> Known {
        Known {
            tool,
            role,
            window: false,
        }
    }

    const fn window(tool: Tool) -> Known {
        Known {
            tool,
            role: None,
            window: true,
        }
    }
}

/// What an executable is, from its name (`deskflow-core`, `Deskflow-Core.exe`). Case is
/// ignored: Windows does.
pub fn classify(executable: &str) -> Option<Known> {
    let lower = executable.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    KNOWN
        .iter()
        .find(|(name, _)| *name == stem)
        .map(|&(_, known)| known)
}

/// `deskflow-core server …`, `synergy-core client …`: the first argument after the
/// program that names an end.
pub fn role_from_args<'a>(args: impl IntoIterator<Item = &'a str>) -> Option<Role> {
    args.into_iter().skip(1).find_map(|arg| match arg {
        "server" | "--server" => Some(Role::Server),
        "client" | "--client" => Some(Role::Client),
        _ => None,
    })
}

/// The tools of `running`, each once, in the order of [`Tool`].
pub fn tools(running: &[Running]) -> Vec<Tool> {
    let mut tools: Vec<Tool> = running.iter().map(|r| r.tool).collect();
    tools.sort();
    tools.dedup();
    tools
}

/// What the user is told, one sentence per tool, in the order of [`Tool`].
pub fn warnings(running: &[Running]) -> Vec<String> {
    tools(running)
        .into_iter()
        .map(|tool| {
            let of_tool = || running.iter().filter(move |r| r.tool == tool);
            let name = tool.name();
            let client = of_tool().any(|r| r.role == Some(Role::Client));
            let server = of_tool().any(|r| r.role == Some(Role::Server));
            let connected = of_tool().any(|r| r.connected == Some(true));
            let mut words = if of_tool().all(|r| r.window) {
                // The window alone moves nothing: it is what starts what would.
                format!("{name} is open: once started, it can move the mouse to other machines")
            } else if client && !server {
                format!(
                    "{name} is also running: another machine's mouse and keyboard can drive \
                     this one through it"
                )
            } else {
                format!("{name} is also running and can move the mouse to other machines")
            };
            if connected {
                words.push_str(", and it is connected to one now");
            }
            words
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_known_executables_are_recognised_whatever_their_case() {
        let core = |tool, role| Some(Known::core(tool, role));
        assert_eq!(classify("deskflow-core"), core(Tool::Deskflow, None));
        assert_eq!(classify("Deskflow-Core.exe"), core(Tool::Deskflow, None));
        assert_eq!(classify("deskflow"), Some(Known::window(Tool::Deskflow)));
        assert_eq!(
            classify("deskflow-server"),
            core(Tool::Deskflow, Some(Role::Server))
        );
        assert_eq!(
            classify("synergys"),
            core(Tool::Synergy, Some(Role::Server))
        );
        assert_eq!(
            classify("synergyc.exe"),
            core(Tool::Synergy, Some(Role::Client))
        );
        assert_eq!(classify("synergy-core"), core(Tool::Synergy, None));
        assert_eq!(
            classify("barriers"),
            core(Tool::Barrier, Some(Role::Server))
        );
        assert_eq!(
            classify("BARRIERC.EXE"),
            core(Tool::Barrier, Some(Role::Client))
        );
        assert_eq!(
            classify("input-leaps"),
            core(Tool::InputLeap, Some(Role::Server))
        );
        assert_eq!(
            classify("input-leapc"),
            core(Tool::InputLeap, Some(Role::Client))
        );
    }

    #[test]
    fn a_name_that_only_looks_like_one_is_not_one() {
        assert_eq!(classify("deskflow-daemon"), None);
        assert_eq!(classify("barrier"), None);
        assert_eq!(classify("synergy-cored"), None);
        assert_eq!(classify("lbm-agent"), None);
        assert_eq!(classify(""), None);
        assert_eq!(classify(".exe"), None);
    }

    #[test]
    fn the_core_says_its_end_on_its_command_line() {
        assert_eq!(
            role_from_args(["/usr/bin/deskflow-core", "server", "--name", "pc"]),
            Some(Role::Server)
        );
        assert_eq!(
            role_from_args(["deskflow-core", "client", "192.168.1.2"]),
            Some(Role::Client)
        );
        assert_eq!(role_from_args(["deskflow-core", "--help"]), None);
        // The program's own name says nothing, even when it is "server".
        assert_eq!(role_from_args(["server"]), None);
    }

    fn running(tool: Tool, role: Option<Role>, connected: Option<bool>) -> Running {
        Running {
            tool,
            process: "p".to_owned(),
            pid: 1,
            role,
            window: false,
            connected,
        }
    }

    /// Deskflow's window with nothing started from it (its GUI says "not running"): it
    /// moves nothing, and is not said to.
    #[test]
    fn a_window_alone_is_said_to_be_open_not_moving_anything() {
        let mut window = running(Tool::Deskflow, None, Some(false));
        window.window = true;
        assert_eq!(
            warnings(std::slice::from_ref(&window)),
            ["Deskflow is open: once started, it can move the mouse to other machines"]
        );
        // Its core beside it: the core is what is said.
        let core = running(Tool::Deskflow, Some(Role::Server), Some(false));
        assert_eq!(
            warnings(&[window, core]),
            ["Deskflow is also running and can move the mouse to other machines"]
        );
    }

    #[test]
    fn one_sentence_per_tool_and_the_connection_when_known() {
        assert!(warnings(&[]).is_empty());
        let said = warnings(&[
            running(Tool::Deskflow, None, Some(false)),
            running(Tool::Deskflow, Some(Role::Server), Some(true)),
        ]);
        assert_eq!(
            said,
            [
                "Deskflow is also running and can move the mouse to other machines, \
              and it is connected to one now"
            ]
        );
        let said = warnings(&[
            running(Tool::InputLeap, Some(Role::Client), None),
            running(Tool::Barrier, None, None),
        ]);
        assert_eq!(
            said,
            [
                "Barrier is also running and can move the mouse to other machines",
                "Input Leap is also running: another machine's mouse and keyboard can \
                 drive this one through it",
            ]
        );
    }

    #[test]
    fn the_state_reads_back_what_it_wrote_and_tolerates_less() {
        let one = running(Tool::Synergy, Some(Role::Client), Some(true));
        let text = serde_json::to_string(&one).unwrap();
        assert_eq!(
            text,
            r#"{"Tool":"Synergy","Process":"p","Pid":1,"Role":"Client","Connected":true}"#
        );
        assert_eq!(serde_json::from_str::<Running>(&text).unwrap(), one);
        let less: Running =
            serde_json::from_str(r#"{"Tool":"Barrier","Process":"barriers","Pid":7}"#).unwrap();
        assert_eq!(less.role, None);
        assert_eq!(less.connected, None);
    }
}
