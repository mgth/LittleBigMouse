//! What the window is made of, apart from the window.
//!
//! `main.rs` is the binary; this is the part that can be tested without one — the agent
//! client, which has to be exercised against a real agent rather than a double.
pub mod client;
pub mod conversation;
pub mod displays;
pub mod probe;
pub mod properties;
pub mod refresh;
pub mod resistance;
pub mod saved;
pub mod session;
pub mod settings;
