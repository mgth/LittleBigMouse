//! What the window is made of, apart from the window.
//!
//! `main.rs` is the binary; this is the part that can be tested without one — the agent
//! client, which has to be exercised against a real agent rather than a double.
pub mod client;
pub mod conversation;
pub mod desktop_wallpaper;
pub mod displays;
pub mod image_picker;
pub mod instance;
pub mod machines;
pub mod patterns;
pub mod probe;
pub mod properties;
pub mod refresh;
pub mod resistance;
pub mod saved;
pub mod screen_input;
pub mod session;
pub mod settings;
pub mod wallpaper_editor;

pub mod vcp;

pub mod remote;
pub mod remote_rulers;
pub mod remote_wallpapers;
