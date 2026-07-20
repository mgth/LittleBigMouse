//! TCP server + client registry.
//!
//! Port of `RemoteServerSocket`. Binds a loopback listener, accepts connections,
//! and spawns one reader thread per client. The registry holds `Arc<ClientHandle>`
//! writers so events can be broadcast to listening clients.
//!
//! The C++ `RemoteServerSocket::Remove` spawned a detached thread that
//! `Stop/Join/delete`d the client — the historical use-after-free. Here a client
//! owns its own reader thread and its `Arc<ClientHandle>` drops when the last
//! reference goes; there is no manual free, so that UAF is unrepresentable.

use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::ipc::client;
use crate::shared::Shared;

pub type ClientId = u64;

/// A live connection's write half plus its listening flag.
pub struct ClientHandle {
    pub id: ClientId,
    writer: Mutex<TcpStream>,
    listening: AtomicBool,
}

impl ClientHandle {
    fn new(id: ClientId, writer: TcpStream) -> Self {
        // State broadcasts (RUNNING/STOPPED, focus events…) are written from the
        // ROUTING thread, which must never block unboundedly (see the rule in
        // hook/linux/evdev.rs): a client that stopped reading with a full socket
        // buffer would otherwise freeze the pointer with the mice still grabbed.
        // The timeout bounds that worst case; the failed send gets the client
        // pruned by broadcast().
        let _ = writer.set_write_timeout(Some(std::time::Duration::from_millis(500)));
        ClientHandle {
            id,
            writer: Mutex::new(writer),
            listening: AtomicBool::new(false),
        }
    }

    /// Write a message to this client. Returns an error if the socket is dead so
    /// the caller can prune it.
    pub fn send(&self, msg: &str) -> std::io::Result<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(msg.as_bytes())?;
        w.flush()
    }

    pub fn set_listening(&self) {
        self.listening.store(true, Ordering::SeqCst);
    }

    pub fn is_listening(&self) -> bool {
        self.listening.load(Ordering::SeqCst)
    }
}

/// Cloneable handle to the client registry; used to broadcast/target events.
#[derive(Clone)]
pub struct ServerHandle {
    registry: Arc<Mutex<HashMap<ClientId, Arc<ClientHandle>>>>,
}

impl ServerHandle {
    fn new() -> Self {
        ServerHandle {
            registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn insert(&self, handle: Arc<ClientHandle>) {
        self.registry.lock().unwrap().insert(handle.id, handle);
    }

    /// Remove a client from the registry (on disconnect).
    pub fn remove(&self, id: ClientId) {
        self.registry.lock().unwrap().remove(&id);
    }

    fn get(&self, id: ClientId) -> Option<Arc<ClientHandle>> {
        self.registry.lock().unwrap().get(&id).cloned()
    }

    /// Mark a client as listening (C++ `RemoteClient::Listen`), so broadcasts
    /// reach it.
    pub fn set_listening(&self, id: ClientId) {
        if let Some(c) = self.get(id) {
            c.set_listening();
        }
    }

    /// Send a message to a single client (C++ `Send(msg, client)`).
    pub fn send_to(&self, id: ClientId, msg: &str) {
        if let Some(c) = self.get(id) {
            let _ = c.send(msg);
        }
    }

    /// Broadcast a message to all listening clients (C++ `Send(msg, nullptr)`),
    /// pruning any whose socket has died.
    pub fn broadcast(&self, msg: &str) {
        // Snapshot listening clients under the lock, then send with the lock
        // released so a slow/blocked write can't stall the registry.
        let clients: Vec<Arc<ClientHandle>> = {
            let reg = self.registry.lock().unwrap();
            reg.values().filter(|c| c.is_listening()).cloned().collect()
        };

        let mut dead = Vec::new();
        for c in clients {
            if c.send(msg).is_err() {
                dead.push(c.id);
            }
        }

        if !dead.is_empty() {
            let mut reg = self.registry.lock().unwrap();
            for id in dead {
                reg.remove(&id);
            }
        }
    }
}

/// Bind the loopback listener on `port` (use `25196` for the real contract, or
/// `0` in tests for an OS-assigned port), spawn the accept loop, and return the
/// server handle together with the actually-bound port.
pub fn start(shared: &'static Shared, port: u16) -> (ServerHandle, u16) {
    // C++ binds INADDR_ANY; loopback is safer and the C# UI dials "localhost".
    let listener = TcpListener::bind(("127.0.0.1", port))
        .unwrap_or_else(|e| panic!("failed to bind 127.0.0.1:{port}: {e}"));
    let bound = listener.local_addr().expect("local_addr").port();

    let handle = ServerHandle::new();
    let accept_handle = handle.clone();

    thread::spawn(move || {
        let mut next_id: ClientId = 1;
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };

            let id = next_id;
            next_id += 1;

            // The write half lives in the registry; the read half stays with the
            // reader thread. Both are clones of the same socket.
            let Ok(writer) = stream.try_clone() else {
                continue;
            };
            accept_handle.insert(Arc::new(ClientHandle::new(id, writer)));

            let server = accept_handle.clone();
            thread::spawn(move || client::run(id, stream, server, shared));
        }
    });

    (handle, bound)
}
