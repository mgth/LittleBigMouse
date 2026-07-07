//! Zone graph — port of `Engine/Zone`, `ZoneLink`, `ZonesLayout`.
//!
//! The C++ graph is built from raw owning pointers (`Zone*`, `ZoneLink*`,
//! `unordered_map<Zone*>`), the source of the historical use-after-free crashes.
//! Here zones live in a generational [`slotmap`] arena keyed by [`ZoneId`]: a
//! layout reload drops the whole arena and bumps generations, so any stale
//! `ZoneId` held elsewhere (e.g. the engine's `old_zone`) resolves to `None`
//! instead of dereferencing freed memory.

pub mod layout;
pub mod travel;
pub mod xml;
pub mod zone;
pub mod zone_link;

pub use layout::{Algorithm, ZonesLayout};
pub use zone::Zone;
pub use zone_link::ZoneLink;

slotmap::new_key_type! {
    /// Generational handle into the zone arena.
    pub struct ZoneId;
}
