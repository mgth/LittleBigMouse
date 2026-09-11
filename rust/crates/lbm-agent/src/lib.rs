//! The v6 resident process of LittleBigMouse (`docs/v6-architecture-plan.md` on the
//! `v6` branch, phase 3), as a library for its binary and its tests.
//!
//! - [`reconcile`]: the decisions — when a display change rebuilds the layout, when the
//!   engine is hooked — as a state machine without I/O.
//! - [`hook`]: the connection to the hook, kept up.
//! - [`fake_hook`]: a hook that hooks nothing, for `--fake-hook` and the tests.

pub mod fake_hook;
pub mod hook;
pub mod reconcile;
