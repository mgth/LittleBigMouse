//! Mouse traversal engine — port of `Engine/MouseEngine`.
//!
//! Phase 2 only holds the zone layout (populated by `Load`). Phase 3 adds the
//! actual traversal algorithm (Strait/Cross, resistance, freelook) and the
//! non-blocking access from the low-level hook callback.

use crate::zones::ZonesLayout;

pub struct MouseEngine {
    pub layout: ZonesLayout,
    // Phase 3: old_point, old_zone: Option<ZoneId>, clip state, resistance,
    // freelook, dispatch function pointer.
}

impl MouseEngine {
    pub fn new() -> Self {
        MouseEngine {
            layout: ZonesLayout::default(),
        }
    }

    /// C++ `MouseEngine::Layout.Load` — replace the layout. Dropping the old
    /// `ZonesLayout` drops its whole arena, so every previously-handed-out
    /// `ZoneId` becomes stale (resolves to `None`) rather than dangling.
    pub fn load(&mut self, layout: ZonesLayout) {
        self.layout = layout;
        // Phase 3: reset old_zone / resistance / clip here.
    }
}

impl Default for MouseEngine {
    fn default() -> Self {
        Self::new()
    }
}
