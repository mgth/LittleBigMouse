//! Port of `Engine/HookMouseEventArg` (timing fields dropped).

use crate::geometry::Point;

pub struct MouseEventArg {
    pub point: Point<i32>,
    /// Set by the engine when it repositioned the cursor; the callback then
    /// blocks the event (returns 1) so the cursor sticks to the border.
    pub handled: bool,
    /// Whether the hook is active. A synthetic `running = false` event is fed on
    /// unhook so the engine restores any clip it set.
    pub running: bool,
}

impl MouseEventArg {
    pub fn new(point: Point<i32>) -> Self {
        MouseEventArg {
            point,
            handled: false,
            running: true,
        }
    }
}
