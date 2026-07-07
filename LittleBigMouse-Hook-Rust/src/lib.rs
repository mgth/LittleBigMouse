//! Little Big Mouse hook daemon — Rust port of the native C++ `LittleBigMouse.Hook`.
//!
//! The daemon is a separate process launched by the C# UI. The two communicate
//! over a loopback TCP socket (port 25196) exchanging `\n`-delimited XML
//! messages — the language-agnostic contract that lets this Rust process replace
//! the C++ one wholesale.
//!
//! Phase status: **Phase 1** — the socket/XML boundary (Phase 0) plus the Win32
//! low-level mouse hook, the WinEvent/display hooks and the message pump. The
//! mouse callback is still a pass-through (no zone engine yet — Phase 3).

pub mod daemon;
pub mod engine;
pub mod geometry;
pub mod hook;
pub mod ipc;
pub mod platform;
pub mod priority;
pub mod shared;
pub mod zones;
