//! The window's connection lifetime. One background runtime, one writer per connection.
//! Reconnects subscribe anew; requests from a lost connection are never replayed.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use lbm_ipc::framing::{read_frame, write_frame};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};

use crate::client::{self, Message};

pub type Requests = mpsc::UnboundedSender<(u64, &'static str, Value)>;

#[derive(Debug)]
pub enum Event {
    /// Hello has been checked and Subscribe has answered. User ids start at 3.
    Connected {
        requests: Requests,
        snapshot: Value,
        /// What the agent announced in `Hello` beyond the protocol
        /// (`lbm_agent::api::CAPABILITIES`); empty for an agent that announces nothing.
        capabilities: Vec<String>,
    },
    Message(Message),
    Disconnected(String),
}

/// The resident is a sibling of the frontend, with the same build configuration.
/// An explicit endpoint is connect-only: a test or side-by-side window must never
/// start the normal session's agent when its private endpoint is absent.
pub struct Config {
    pub endpoint: PathBuf,
    pub launch: Option<Launch>,
    pub retry: Duration,
    pub deadline: Duration,
}

pub struct Launch {
    pub program: PathBuf,
    pub frontend: PathBuf,
}

impl Config {
    pub fn for_session() -> Option<Self> {
        let explicit =
            std::env::var_os(client::ENDPOINT_VARIABLE).is_some_and(|value| !value.is_empty());
        let frontend = std::env::current_exe().ok();
        Some(Self {
            endpoint: client::default_endpoint()?,
            launch: if explicit {
                None
            } else {
                frontend.and_then(|frontend| {
                    let program = frontend.parent()?.join(if cfg!(windows) {
                        "lbm-agent.exe"
                    } else {
                        "lbm-agent"
                    });
                    Some(Launch { program, frontend })
                })
            },
            retry: Duration::from_millis(250),
            deadline: Duration::from_secs(5),
        })
    }
}

impl Launch {
    fn start(&self) -> io::Result<()> {
        let mut command = Command::new(&self.program);
        command
            .arg("--ui")
            .arg(&self.frontend)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()); // The agent rotates its own log when not on a terminal.
        if let Some(parent) = self.program.parent() {
            command.current_dir(parent);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn()?;
        // Reap without owning its lifetime: closing this window must not kill it.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

/// Dropping the session cancels even a silent handshake/read and closes both halves.
/// That connection closure also ends any preview owned by this window at the agent.
pub struct Session(Option<oneshot::Sender<()>>);

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(stop) = self.0.take() {
            let _ = stop.send(());
        }
    }
}

pub fn spawn(config: Config, emit: impl Fn(Event) -> bool + Send + 'static) -> Session {
    let (stop, stopped) = oneshot::channel();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                emit(Event::Disconnected(error.to_string()));
                return;
            }
        };
        runtime.block_on(async {
            tokio::select! {
                _ = stopped => {},
                _ = maintain(config, &emit) => {},
            }
        });
    });
    Session(Some(stop))
}

async fn maintain(mut config: Config, emit: &impl Fn(Event) -> bool) {
    let mut delay = config.retry;
    loop {
        // Only absence authorizes launch. Access denied, busy pipes and incompatible
        // protocols must not produce a second resident or an elevation loop.
        let connected = tokio::time::timeout(config.deadline, open(&config.endpoint)).await;
        let stream = match connected {
            Ok(Ok(stream)) => Some(stream),
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    _ => io::Error::new(io::ErrorKind::TimedOut, "agent connection timed out"),
                };
                let mut why = error.to_string();
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) {
                    if let Some(launch) = config.launch.take() {
                        why = match launch.start() {
                            Ok(()) => "starting the agent".into(),
                            Err(error) => {
                                format!("could not start {}: {error}", launch.program.display())
                            }
                        };
                    }
                }
                if !emit(Event::Disconnected(why)) {
                    return;
                }
                None
            }
        };
        if let Some(mut stream) = stream {
            // Once an endpoint answers, this window never launches a replacement. In
            // particular, the tray's Quit remains a quit; we wait for an external restart.
            config.launch = None;
            let greeting = tokio::time::timeout(config.deadline, greet(&mut stream)).await;
            match greeting {
                Ok(Ok((snapshot, capabilities))) => {
                    delay = config.retry;
                    let (requests, receiver) = mpsc::unbounded_channel();
                    if !emit(Event::Connected {
                        requests,
                        snapshot,
                        capabilities,
                    }) {
                        return;
                    }
                    let error = converse(stream, receiver, config.deadline, emit).await;
                    if !emit(Event::Disconnected(error.to_string())) {
                        return;
                    }
                }
                result => {
                    let why = match result {
                        Ok(Err(error)) => error.to_string(),
                        _ => "agent handshake timed out".into(),
                    };
                    if !emit(Event::Disconnected(why)) {
                        return;
                    }
                }
            }
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(5));
    }
}

trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}
type Stream = Box<dyn Duplex>;

#[cfg(unix)]
async fn open(endpoint: &Path) -> io::Result<Stream> {
    Ok(Box::new(tokio::net::UnixStream::connect(endpoint).await?))
}

#[cfg(windows)]
async fn open(endpoint: &Path) -> io::Result<Stream> {
    Ok(Box::new(
        tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint)?,
    ))
}

async fn answer(stream: &mut Stream, id: u64, method: &str, extra: Value) -> io::Result<Value> {
    write_frame(stream, &request(id, method, extra)).await?;
    match client::decode(&read_frame(stream).await?)? {
        Message::Answer {
            id: received,
            result,
        } if received == id => result.map_err(io::Error::other),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected handshake answer",
        )),
    }
}

/// The Subscribe answer and the capabilities the agent announced.
async fn greet(stream: &mut Stream) -> io::Result<(Value, Vec<String>)> {
    let hello = answer(stream, 1, "Hello", json!({"Client": "lbm-app"})).await?;
    if hello["Agent"] != "lbm-agent" || hello["Protocol"].as_u64() != Some(client::PROTOCOL) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "incompatible agent protocol (expected {}, received {})",
                client::PROTOCOL,
                hello["Protocol"]
            ),
        ));
    }
    let capabilities = hello["Capabilities"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|c| c.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Ok((
        answer(stream, 2, "Subscribe", json!({})).await?,
        capabilities,
    ))
}

fn request(id: u64, method: &str, extra: Value) -> String {
    let mut frame = extra.as_object().cloned().unwrap_or_default();
    frame.insert("Id".into(), id.into());
    frame.insert("Method".into(), method.into());
    Value::Object(frame).to_string()
}

async fn converse(
    stream: Stream,
    mut requests: mpsc::UnboundedReceiver<(u64, &'static str, Value)>,
    deadline: Duration,
    emit: &impl Fn(Event) -> bool,
) -> io::Error {
    let (mut read, mut write) = tokio::io::split(stream);
    // These are whole loops, not a select per frame: cancelling a partial read when a
    // request arrives would throw away its prefix and corrupt the next frame.
    let reading = async {
        loop {
            let message = client::decode(&read_frame(&mut read).await?)?;
            if !emit(Event::Message(message)) {
                return Err(io::Error::other("window closed"));
            }
        }
    };
    let writing = async {
        while let Some((id, method, extra)) = requests.recv().await {
            tokio::time::timeout(
                deadline,
                write_frame(&mut write, &request(id, method, extra)),
            )
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "agent write timed out"))??;
        }
        Err(io::Error::other("window closed"))
    };
    let result: io::Result<()> = tokio::select! {
        result = reading => result,
        result = writing => result,
    };
    result.unwrap_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn writing_while_an_event_is_fragmented_preserves_the_read_frame() {
        let (client, server) = tokio::io::duplex(4096);
        let (requests, receiver) = mpsc::unbounded_channel();
        let (events, mut received) = mpsc::unbounded_channel();
        let connection = async {
            converse(
                Box::new(client),
                receiver,
                Duration::from_secs(1),
                &|event| events.send(event).is_ok(),
            )
            .await
        };
        let exercise = async {
            let (mut read, mut write) = tokio::io::split(server);
            let message = r#"{"Event":"State","State":{"Engine":"Running"}}"#;
            let length = (message.len() as u32).to_le_bytes();
            write.write_all(&length[..2]).await.unwrap();
            tokio::task::yield_now().await;
            requests.send((3, "Stop", json!({}))).unwrap();
            let request: Value =
                serde_json::from_str(&read_frame(&mut read).await.unwrap()).unwrap();
            assert_eq!(request["Method"], "Stop");
            write.write_all(&length[2..]).await.unwrap();
            write.write_all(message.as_bytes()).await.unwrap();
            assert!(
                matches!(received.recv().await.unwrap(), Event::Message(Message::State(s)) if s["Engine"] == "Running")
            );
            drop(requests);
            let mut byte = [0];
            assert_eq!(read.read(&mut byte).await.unwrap(), 0);
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(connection, exercise);
        })
        .await
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn launch_is_once_and_passes_the_frontend_path_as_one_argument() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("fake agent");
        std::fs::write(&program, "#!/bin/sh\nprintf '%s\\n' \"$@\" >> launched\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let frontend = dir.path().join("frontend with spaces");
        let (send, events) = std::sync::mpsc::channel();
        let session = spawn(
            Config {
                endpoint: dir.path().join("missing.sock"),
                launch: Some(Launch {
                    program,
                    frontend: frontend.clone(),
                }),
                retry: Duration::from_millis(10),
                deadline: Duration::from_secs(1),
            },
            move |event| send.send(event).is_ok(),
        );
        assert!(
            matches!(events.recv_timeout(Duration::from_secs(2)).unwrap(), Event::Disconnected(why) if why == "starting the agent")
        );
        for _ in 0..4 {
            assert!(matches!(
                events.recv_timeout(Duration::from_secs(2)).unwrap(),
                Event::Disconnected(_)
            ));
        }
        drop(session);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("launched")).unwrap(),
            format!("--ui\n{}\n", frontend.display())
        );
    }
}
