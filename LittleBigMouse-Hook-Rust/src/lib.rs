//! Little Big Mouse hook daemon — Rust port of the native C++ `LittleBigMouse.Hook`.
//!
//! The daemon is a separate process launched by the C# UI. The two communicate
//! over a loopback TCP socket (port 25196) exchanging `\n`-delimited XML
//! messages — the language-agnostic contract that lets this Rust process replace
//! the C++ one wholesale.
//!
//! Phase status: **Phase 0** implements the socket/XML boundary only (no Win32
//! hooking yet). Later phases add the message pump, the low-level mouse hook, the
//! zone engine, and the build/CI integration.

pub mod daemon;
pub mod ipc;
pub mod shared;
