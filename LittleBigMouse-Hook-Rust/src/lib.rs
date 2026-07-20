//! Little Big Mouse hook daemon — Rust port of the native C++ `LittleBigMouse.Hook`.
//!
//! The daemon is a separate process launched by the C# UI. The two communicate
//! over per-user local IPC using bounded length-prefixed UTF-8 XML
//! messages — the language-agnostic contract that lets this Rust process replace
//! the C++ one wholesale.
//!
//! The safe zone engine, platform hooks, and authenticated local IPC transport
//! together replace the historical C++ daemon in distributable builds.

pub mod daemon;
pub mod engine;
pub mod geometry;
pub mod hook;
pub mod ipc;
pub mod platform;
pub mod priority;
pub mod shared;
pub mod zones;
