//! Talking to the agent.
//!
//! **This connects. It never starts an agent.** The C# UI does start one when nothing
//! answers, and starting an agent starts a hook, and a hook takes the mice — so a
//! frontend that could do it by accident is a frontend that can take the mice by
//! accident. When no agent answers here, the window says so and draws what it can.
//!
//! Blocking, on threads of its own. The frame is `lbm_ipc::framing`'s, in the shape that
//! crate keeps for callers with no runtime; the endpoint is `lbm_ipc::endpoint`'s, the
//! same names the agent binds and the C# client spells.

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
        let value: Value = serde_json::from_str(&frame)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

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
}

impl Outgoing {
    /// Sends a request and gives back the id its answer will carry.
    ///
    /// `extra` is merged into the frame beside the method, so a caller writes what the
    /// API documents and nothing else.
    pub fn ask(&mut self, method: &str, extra: Value) -> io::Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        let mut frame = json!({ "Id": id, "Method": method });
        if let (Some(object), Some(more)) = (frame.as_object_mut(), extra.as_object()) {
            for (key, value) in more {
                object.insert(key.clone(), value.clone());
            }
        }
        lbm_ipc::framing::write_frame_blocking(&mut self.stream, &frame.to_string())?;
        Ok(id)
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
pub fn default_endpoint() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os(ENDPOINT_VARIABLE) {
        if !named.is_empty() {
            return Some(PathBuf::from(named));
        }
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let data = lbm_store::lbm_paths::data_dir();
    lbm_ipc::endpoint::agent_socket_path(runtime.as_deref(), Some(&data))
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
