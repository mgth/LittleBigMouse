//! Where a remote parks its pointer when the master's session ends.
//!
//! The pointer leaves a remote across one of its edges, so the place the master left
//! it is always on an edge, and often in a corner: exactly where Windows has hot
//! corners, the auto-hidden taskbar's trigger and the Start button's hover. Leaving
//! it there would keep the taskbar up or a "show desktop" peek going for as long as
//! the remote is not driven. The middle of the monitor the pointer was on is none of
//! those, is on a real screen even when the monitors do not form a rectangle (the
//! middle of the virtual screen can fall in a hole between them), and is where the
//! person at that machine looks first when they take their own mouse back.
//!
//! Kept free of any system call so it can be tested on every system.

/// A monitor's rectangle in desktop pixels, right and bottom excluded, as Win32's
/// `RECT` has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Screen {
    fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// The squared distance from `point` to the nearest pixel of this screen; 0 inside.
    fn distance_squared(&self, (x, y): (i32, i32)) -> i64 {
        let gap = |v: i32, low: i32, high: i32| -> i64 {
            if v < low {
                i64::from(low) - i64::from(v)
            } else if v >= high {
                i64::from(v) - i64::from(high - 1)
            } else {
                0
            }
        };
        let dx = gap(x, self.left, self.right);
        let dy = gap(y, self.top, self.bottom);
        dx * dx + dy * dy
    }

    fn centre(&self) -> (i32, i32) {
        // Halved as i64: a monitor far out on a large desktop must not overflow.
        let mid = |low: i32, high: i32| ((i64::from(low) + i64::from(high)) / 2) as i32;
        (mid(self.left, self.right), mid(self.top, self.bottom))
    }

    fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }
}

/// Where to park a pointer that is at `cursor`: the centre of the monitor it is on, or
/// of the nearest one when it is on none (a display just went away). `None` when there
/// is no monitor at all, and the caller parks in place.
pub fn parking_spot(cursor: (i32, i32), screens: &[Screen]) -> Option<(i32, i32)> {
    screen_of(cursor, screens).map(|i| screens[i].centre())
}

/// The index of the monitor `cursor` is on, or of the nearest one when it is on none.
/// `None` when there is no monitor that is not empty.
pub fn screen_of(cursor: (i32, i32), screens: &[Screen]) -> Option<usize> {
    let mut usable = screens.iter().enumerate().filter(|(_, s)| !s.is_empty());
    if let Some((on, _)) = usable.clone().find(|(_, s)| s.contains(cursor)) {
        return Some(on);
    }
    // The first of equally near ones, so the same desktop always gives the same spot.
    let mut best: Option<(usize, i64)> = None;
    for (index, screen) in &mut usable {
        let distance = screen.distance_squared(cursor);
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// The side of the window that hides the pointer, in pixels: one, so that a move of a
/// single pixel by the machine's own mouse takes the pointer off it — and the pointer
/// is shown again by whatever window it lands on — and so that it can swallow no click
/// but one aimed at that very pixel.
pub const HIDER_SIDE: i32 = 1;

/// The hiding window's rectangle as `(x, y, width, height)` for a pointer parked at
/// `spot`: the pointer's hot spot is its only pixel.
pub fn hider_rect(spot: (i32, i32)) -> (i32, i32, i32, i32) {
    (spot.0, spot.1, HIDER_SIDE, HIDER_SIDE)
}

/// What parking does on the hiding window's thread, in the order it must be done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParkStep {
    /// Put the one-pixel window at the spot.
    ShowWindow,
    /// Blank the cursor image. The pointer is still where the master left it, so this
    /// is when nothing of it can be seen at the spot yet.
    BlankCursor,
    /// Move the pointer onto the window.
    MovePointer,
}

/// The parking, in order. The pointer jumps from an edge to the middle of a monitor;
/// the window only hides it once it has handled `WM_SETCURSOR`, which it does after
/// the current handler returns (the window's thread is the one that moves the
/// pointer). Until then the system draws the cursor it had: an arrow, flashed in the
/// middle of the screen. So the image is blanked *before* the pointer moves.
pub const PARK_ORDER: [ParkStep; 3] = [
    ParkStep::ShowWindow,
    ParkStep::BlankCursor,
    ParkStep::MovePointer,
];

#[cfg(test)]
mod tests {
    use super::*;

    const fn screen(left: i32, top: i32, width: i32, height: i32) -> Screen {
        Screen {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    /// A 4K monitor at the origin and a 1080p one to its left, lower, as a laptop and
    /// an external screen often are: the virtual screen's middle is on neither.
    const DESK: [Screen; 2] = [screen(0, 0, 3840, 2160), screen(-1920, 600, 1920, 1080)];

    #[test]
    fn a_pointer_left_in_a_corner_is_parked_in_the_middle_of_its_monitor() {
        assert_eq!(parking_spot((3839, 2159), &DESK), Some((1920, 1080)));
    }

    #[test]
    fn a_pointer_on_a_monitor_left_of_the_origin_is_parked_on_that_monitor() {
        assert_eq!(parking_spot((-1920, 700), &DESK), Some((-960, 1140)));
    }

    #[test]
    fn a_pointer_on_no_monitor_is_parked_on_the_nearest_one() {
        // Above the left monitor, in the hole the two leave at the top left.
        assert_eq!(parking_spot((-100, 100), &DESK), Some((1920, 1080)));
        assert_eq!(parking_spot((-1000, 100), &DESK), Some((-960, 1140)));
    }

    #[test]
    fn the_right_and_bottom_edges_belong_to_the_next_monitor() {
        assert_eq!(parking_spot((0, 700), &DESK), Some((1920, 1080)));
        assert_eq!(parking_spot((-1, 700), &DESK), Some((-960, 1140)));
    }

    #[test]
    fn no_monitor_means_no_spot_and_an_empty_one_is_not_a_monitor() {
        assert_eq!(parking_spot((5, 5), &[]), None);
        assert_eq!(parking_spot((5, 5), &[screen(0, 0, 0, 100)]), None);
    }

    #[test]
    fn a_monitor_far_out_does_not_overflow_its_centre() {
        let far = screen(i32::MAX - 100, i32::MAX - 100, 100, 100);
        assert_eq!(
            parking_spot((i32::MAX - 1, i32::MAX - 1), &[far]),
            Some((i32::MAX - 50, i32::MAX - 50))
        );
    }

    #[test]
    fn the_hiding_window_is_the_single_pixel_under_the_hot_spot() {
        assert_eq!(hider_rect((-960, 1140)), (-960, 1140, 1, 1));
    }

    #[test]
    fn the_cursor_is_blanked_before_the_pointer_jumps_to_the_spot() {
        let at = |step| PARK_ORDER.iter().position(|s| *s == step).unwrap();
        assert!(at(ParkStep::BlankCursor) < at(ParkStep::MovePointer));
        assert!(at(ParkStep::ShowWindow) < at(ParkStep::MovePointer));
    }
}
