//! Little Big Mouse hook daemon — Rust port of the native C++ `LittleBigMouse.Hook`.
//!
//! The daemon is a separate process, driven by the agent (v6) — by the C# UI before
//! it. The two communicate over per-user local IPC using bounded length-prefixed
//! UTF-8 frames carrying `lbm_ipc::protocol` — JSON since the two ends became Rust,
//! XML while the contract had to be language-agnostic enough to let this process
//! replace the C++ one wholesale.
//!
//! The safe zone engine, platform hooks, and authenticated local IPC transport
//! together replace the historical C++ daemon in distributable builds.

pub mod daemon;
pub mod hook;
pub mod ipc;
pub mod platform;
pub mod shared;
/// The rescue shortcut's grammar, which now lives with the protocol.
///
/// It moved because its own first paragraph said where it belonged: "as it travels on
/// the wire", and "keeping the grammar in one place means the UI's recorder and the
/// daemon's registrar cannot drift apart". The v7 frontend is that recorder, and it must
/// never link this crate — linking the hook is linking the code that takes the mice. So
/// the grammar sits in `lbm-ipc`, which both sides already have, and this re-export keeps
/// every `crate::shortcut::…` in the hook meaning what it did.
pub use lbm_ipc::shortcut;

#[cfg(test)]
mod testing;

// Split out into sibling crates of the workspace so the v6 agent and frontend can
// use them without the platform layer. Re-exported under their historical paths:
// the daemon, the tests and the benches keep writing `crate::engine`,
// `littlebigmouse_hook::zones` and so on.
pub use lbm_engine as engine;
pub use lbm_geom as geometry;
pub use lbm_zones as zones;
pub use lbm_zones::priority;
