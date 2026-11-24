//! `lbm-service`, the Windows service of the multi-PC mode
//! (docs/multi-pc-service-windows.md).
//!
//! It runs as LocalSystem in session 0 and launches `lbm-inject` into the session at
//! the console, with its own SYSTEM token moved into that session: the one way to put
//! a master's input on the secure desktop (UAC prompt, lock screen) and above the
//! windows of a standard user. The input it will hand the injector comes, in a later
//! step, only from masters an administrator approved; nothing else feeds it.
//!
//! - [`supervisor`]: which session gets an injector, and when it is stopped. Pure.
//! - [`trust`]: the masters an administrator approved. Pure.
//! - `win`: the service itself, and the launch.

pub mod supervisor;
pub mod trust;

#[cfg(windows)]
pub mod win;

/// The service's name in the Service Control Manager.
pub const SERVICE_NAME: &str = "LittleBigMouseInput";

/// The injector's file name, beside the service's.
pub const INJECTOR_EXE: &str = "lbm-inject.exe";

/// The user-defined control code `lbm-service trust`/`untrust` sends the running
/// service: "the store changed, read it again". It does nothing else.
pub const CONTROL_RELOAD_TRUST: u32 = 128;
