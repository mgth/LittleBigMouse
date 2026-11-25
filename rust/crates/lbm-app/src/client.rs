//! Talking to the agent.
//!
//! This blocking transport only connects. The window uses `session` to manage startup,
//! asynchronous I/O and reconnection; both transports share framing and message decoding.
//! Tests and other synchronous callers can still connect directly without launching
//! anything. Endpoint names are shared with the agent through lbm-ipc.

use std::ffi::OsStr;
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// The API version this client speaks. The agent answers its own to `Hello`, and a
/// mismatch is not something to paper over: a frontend that guessed would be reading
/// fields that may have moved.
pub const PROTOCOL: u64 = 3;

/// What the agent sends.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// An answer to a request, by its id.
    Answer {
        id: u64,
        result: Result<Value, String>,
    },
    /// The agent's state, on subscription and at every change.
    State(Value),
    /// Something the hook said, passed on.
    Hook { name: String, payload: String },
    /// The multi-PC state, whenever it changes (capability `MultiPc`): the body of
    /// `{"Event": "Peers", "Peers": …}`, read by `crate::machines`.
    Peers(Value),
    /// A gesture on the rulers this window showed on a remote's screens (capability
    /// `PeerOverlayInput`): its machine id, and an `lbm_overlay::input::Input`, unread.
    PeerOverlayInput { machine_id: String, input: Value },
    /// Something this version does not know. Kept whole rather than dropped: an agent
    /// newer than its frontend is a case that will happen during the port, and a
    /// frontend that treated it as an error would be wrong more often than it is right.
    Unknown(Value),
}

/// The reading half.
pub struct Incoming(BufReader<Box<dyn Read + Send>>);

/// The writing half, and the request ids it hands out.
pub struct Outgoing {
    stream: Box<dyn Write + Send>,
    next_id: u64,
}

impl Incoming {
    /// The next message, blocking until one arrives. An error ends the connection —
    /// the agent went away, and the window should say so rather than retry silently.
    pub fn receive(&mut self) -> io::Result<Message> {
        let frame = lbm_ipc::framing::read_frame_blocking(&mut self.0)?;
        decode(&frame)
    }
}

/// Decode the same protocol on the blocking and asynchronous transports.
pub(crate) fn decode(frame: &str) -> io::Result<Message> {
    let value: Value =
        serde_json::from_str(frame).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    if let Some(id) = value.get("Id").and_then(Value::as_u64) {
        return Ok(Message::Answer {
            id,
            result: match value.get("Error").and_then(Value::as_str) {
                Some(error) => Err(error.to_owned()),
                None => Ok(value.get("Result").cloned().unwrap_or(Value::Null)),
            },
        });
    }
    Ok(match value.get("Event").and_then(Value::as_str) {
        Some("State") => Message::State(value.get("State").cloned().unwrap_or(Value::Null)),
        Some("Peers") => Message::Peers(value.get("Peers").cloned().unwrap_or(Value::Null)),
        Some("PeerOverlayInput") => Message::PeerOverlayInput {
            machine_id: value
                .get("MachineId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            input: value.get("Input").cloned().unwrap_or(Value::Null),
        },
        Some("Hook") => Message::Hook {
            name: value
                .get("Hook")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            payload: value
                .get("Payload")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        },
        _ => Message::Unknown(value),
    })
}

impl Outgoing {
    /// Sends a request and gives back the id its answer will carry.
    ///
    /// `extra` is merged into the frame beside the method, so a caller writes what the
    /// API documents and nothing else.
    pub fn ask(&mut self, method: &str, extra: Value) -> io::Result<u64> {
        let id = self.reserve();
        self.ask_as(id, method, extra)?;
        Ok(id)
    }

    /// Takes the id the next request will carry, without sending anything.
    ///
    /// For a caller that has to know **what** an answer answers. The id [`ask`](Self::ask)
    /// hands back comes too late for that: the request is already on the wire, so the
    /// reading thread can have the answer before the writing thread has recorded what it
    /// asked. Reserving first puts the pairing in place before the frame exists, which is
    /// an ordering the agent cannot get ahead of.
    pub fn reserve(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Sends a request under an id already taken from [`reserve`](Self::reserve).
    ///
    /// `extra` is merged into the frame beside the method, so a caller writes what the
    /// API documents and nothing else.
    pub fn ask_as(&mut self, id: u64, method: &str, extra: Value) -> io::Result<()> {
        let mut frame = json!({ "Id": id, "Method": method });
        if let (Some(object), Some(more)) = (frame.as_object_mut(), extra.as_object()) {
            for (key, value) in more {
                object.insert(key.clone(), value.clone());
            }
        }
        lbm_ipc::framing::write_frame_blocking(&mut self.stream, &frame.to_string())
    }
}

/// `LBM_AGENT_ENDPOINT`: the socket to use instead of this session's.
///
/// The twin of the hook's `LBM_HOOK_ENDPOINT`, and for the same two reasons: a
/// side-by-side instance, and being able to point a window at an agent raised for a
/// test instead of the one holding the user's mice.
pub const ENDPOINT_VARIABLE: &str = "LBM_AGENT_ENDPOINT";

/// Where the agent listens, by the rules it binds with — or wherever
/// [`ENDPOINT_VARIABLE`] says.
///
/// Windows: the agent's pipe of this logon session (`lbm-agent`'s `winpipe::agent_pipe`),
/// never a file. The window decides from this endpoint's absence whether to start an
/// agent, so a wrong default would start one beside the running agent and then never
/// reach either.
pub fn default_endpoint() -> Option<PathBuf> {
    let explicit = std::env::var_os(ENDPOINT_VARIABLE);
    #[cfg(windows)]
    let session = SessionEndpoint::Pipe(lbm_ipc::endpoint::session_id().ok());
    #[cfg(not(windows))]
    let session = SessionEndpoint::Socket {
        runtime: std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        data: Some(lbm_store::lbm_paths::data_dir()),
    };
    endpoint(explicit.as_deref(), session)
}

/// What this session's default endpoint is made from, on each platform.
#[derive(Debug)]
pub(crate) enum SessionEndpoint {
    /// Windows: the logon session, when it could be read.
    #[cfg_attr(not(windows), allow(dead_code))]
    Pipe(Option<u32>),
    /// Elsewhere: the agent's socket directories.
    #[cfg_attr(windows, allow(dead_code))]
    Socket {
        runtime: Option<PathBuf>,
        data: Option<PathBuf>,
    },
}

/// The explicit endpoint when there is a non-empty one, else this session's.
pub(crate) fn endpoint(explicit: Option<&OsStr>, session: SessionEndpoint) -> Option<PathBuf> {
    if let Some(named) = explicit.filter(|named| !named.is_empty()) {
        return Some(PathBuf::from(named));
    }
    match session {
        SessionEndpoint::Pipe(session) => session
            .map(lbm_ipc::endpoint::agent_pipe_name)
            .map(PathBuf::from),
        SessionEndpoint::Socket { runtime, data } => {
            lbm_ipc::endpoint::agent_socket_path(runtime.as_deref(), data.as_deref())
        }
    }
}

/// Opens a connection to an agent that is **already running**.
///
/// Nothing is started, nothing is retried: an agent that is not there is an answer, not
/// a failure to work around.
pub fn connect(endpoint: &Path) -> io::Result<(Incoming, Outgoing)> {
    connect_with(endpoint, None)
}

/// The same, giving reads a deadline.
///
/// No deadline by default, and that is not an oversight: a subscribed frontend can
/// rightly hear nothing for hours, and a timeout would turn a quiet desktop into a lost
/// connection. A deadline is for a caller that is waiting for something in particular —
/// a test, or a handshake that ought to be answered at once.
pub fn connect_with(
    endpoint: &Path,
    read_timeout: Option<std::time::Duration>,
) -> io::Result<(Incoming, Outgoing)> {
    let (read, write) = open(endpoint, read_timeout)?;
    Ok((
        Incoming(BufReader::new(read)),
        Outgoing {
            stream: write,
            next_id: 1,
        },
    ))
}

#[cfg(unix)]
fn open(
    endpoint: &Path,
    read_timeout: Option<std::time::Duration>,
) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
    let stream = std::os::unix::net::UnixStream::connect(endpoint)?;
    stream.set_read_timeout(read_timeout)?;
    let write = stream.try_clone()?;
    Ok((Box::new(stream), Box::new(write)))
}

#[cfg(windows)]
fn open(
    endpoint: &Path,
    _read_timeout: Option<std::time::Duration>,
) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
    // A named pipe opens as a file, both ways. Untried on a real Windows session — the
    // agent's pipe is per-logon and there is none here to answer — so it compiles and
    // is honest about being unproven rather than being left out.
    let pipe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(endpoint)?;
    let read = pipe.try_clone()?;
    Ok((Box::new(read), Box::new(pipe)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_defaults_to_the_agents_pipe_of_the_session_never_a_file() {
        assert_eq!(
            endpoint(None, SessionEndpoint::Pipe(Some(3))),
            Some(PathBuf::from(r"\\.\pipe\LittleBigMouse-Agent-v1-session-3"))
        );
        // No session read: no endpoint, so nothing is launched against a guess.
        assert_eq!(endpoint(None, SessionEndpoint::Pipe(None)), None);
    }

    #[test]
    fn an_explicit_endpoint_wins_on_both_platforms_and_an_empty_one_does_not_count() {
        let named = OsStr::new("/run/user/1000/lbm-test.sock");
        assert_eq!(
            endpoint(Some(named), SessionEndpoint::Pipe(Some(3))),
            Some(PathBuf::from(named))
        );
        let socket = || SessionEndpoint::Socket {
            runtime: Some(PathBuf::from("/run/user/1000")),
            data: None,
        };
        assert_eq!(endpoint(Some(named), socket()), Some(PathBuf::from(named)));
        assert_eq!(
            endpoint(Some(OsStr::new("")), socket()),
            Some(PathBuf::from("/run/user/1000/lbm-agent.sock"))
        );
    }

    /// On the platform itself, with nothing named: what the agent's `winpipe::agent_pipe`
    /// binds.
    #[cfg(windows)]
    #[test]
    fn the_default_endpoint_on_windows_is_this_sessions_agent_pipe() {
        if std::env::var_os(ENDPOINT_VARIABLE).is_some() {
            return;
        }
        let session = lbm_ipc::endpoint::session_id().unwrap();
        assert_eq!(
            default_endpoint(),
            Some(PathBuf::from(lbm_ipc::endpoint::agent_pipe_name(session)))
        );
    }
}
