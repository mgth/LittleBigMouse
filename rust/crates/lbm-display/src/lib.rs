//! Display detection for LittleBigMouse (phase 2 of the v6 plan,
//! `docs/v6-architecture-plan.md` on the `v6` branch): which monitors are there, who
//! they are, and a cheap way to tell that it changed.
//!
//! - [`edid`]: the EDID parser, bit for bit the C# one — every monitor id is built
//!   from what it reads.
//! - [`linux`]: the Linux outputs (KScreen, then xrandr), with their EDIDs from sysfs.
//! - [`windows`]: the Win32 device tree (sources, monitors, EDIDs through SetupAPI, the
//!   CCD targets). The Win32 calls only build on Windows; the rules applied to their
//!   answers build and are tested everywhere.
//!
//! The output is the neutral description `lbm-layout` maps into a layout; nothing here
//! knows about layouts.

pub mod edid;
pub mod linux;
pub mod windows;
