//! The multi-PC master's crossing rules, with no platform in sight (docs/multi-pc.md).
//!
//! A master drives another machine through the input stream while the pointer is
//! there. What it does then — which side a release goes to, when the keyboards are
//! grabbed, when the pointer comes home, the rescue watched for in the grabbed stream —
//! does not depend on how the platform reads its devices. Those rules live here, so
//! that every master (evdev on Linux today, the Windows hook and the portal later)
//! applies the same ones and is tested against the same ones.

// Only the evdev backend is a master so far: on Windows nothing calls into this yet,
// and it is compiled there all the same, so that it stays platform-neutral.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

pub(crate) mod keys;
