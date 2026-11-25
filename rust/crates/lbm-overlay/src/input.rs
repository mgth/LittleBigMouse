//! Native input uses surface-local UI points, independent of fractional buffer scale.
use crate::scene::{Band, Pixels};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Press,
    Move,
    Release,
    Wheel(f64),
    Escape,
    Delete,
    Cancel,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub band: Band,
    pub kind: Kind,
    pub at: f64,
    pub length: f64,
    pub thickness: f64,
    pub ctrl: bool,
    pub shift: bool,
    pub time: u32,
}
impl Input {
    pub fn new(
        band: &Band,
        kind: Kind,
        point: (f64, f64),
        units: f64,
        time: u32,
        ctrl: bool,
        shift: bool,
    ) -> Self {
        Self {
            band: band.clone(),
            kind,
            at: if band.side.vertical() {
                point.1
            } else {
                point.0
            } / units,
            length: f64::from(band.axis_pixels()) / units,
            thickness: f64::from(band.thickness()) / units,
            ctrl,
            shift,
            time,
        }
    }
    pub fn valid(&self) -> bool {
        [self.at, self.length, self.thickness]
            .iter()
            .all(|n| n.is_finite() && n.abs() <= 1_000_000.)
            && self.length > 0.
            && self.thickness > 0.
            && !matches!(self.kind, Kind::Wheel(d) if !d.is_finite() || d.abs() > 100.)
            && self.band.interactive()
    }
}
/// Tells a gesture cut short from the capture ending because the button came up.
///
/// Windows reports both as `WM_CAPTURECHANGED`, even when the window released the capture
/// itself. Cancelling on the normal one would forget every click, and so every double click.
#[derive(Clone, Copy, Debug, Default)]
pub struct Capture {
    held: bool,
}
impl Capture {
    pub fn press(&mut self) {
        self.held = true;
    }
    pub fn release(&mut self) {
        self.held = false;
    }
    /// The capture is gone: a cancellation only if the button was still down.
    pub fn lost(&mut self) -> Option<Kind> {
        std::mem::take(&mut self.held).then_some(Kind::Cancel)
    }
}

/// The same half-open diagonals as the pixels: neighbouring edge windows never
/// consume each other's corner. Reference lines and passive bands have no input region
/// at all.
pub fn regions(band: &Band) -> Vec<Pixels> {
    if !band.interactive() {
        return vec![];
    }
    let mut regions = Vec::new();
    let axis = band.axis_pixels();
    let depth = band.thickness();
    for inward in 0..depth {
        let start = inward + i32::from(band.side.vertical());
        let length = axis - 2 * start;
        if length <= 0 {
            continue;
        }
        use crate::scene::Side;
        regions.push(match band.side {
            Side::Top => Pixels {
                x: start,
                y: inward,
                width: length,
                height: 1,
            },
            Side::Bottom => Pixels {
                x: start,
                y: depth - 1 - inward,
                width: length,
                height: 1,
            },
            Side::Left => Pixels {
                x: inward,
                y: start,
                width: 1,
                height: length,
            },
            Side::Right => Pixels {
                x: depth - 1 - inward,
                y: start,
                width: 1,
                height: length,
            },
        });
    }
    regions
}

/// The most gestures that wait once moves are merged: presses, releases, wheel notches
/// and keys, which come at the rate of a hand.
pub const MAX_GESTURES: usize = 1024;

/// Gestures waiting to be handed on — to a master over a link, or to the editor at its
/// next frame — each with a tag (the revision it was made under, say), in order.
///
/// A move that follows a move on the same band replaces it: what is pending of a drag
/// is where the pointer is now, so a slow reader delays it and never makes the queue
/// grow at the mouse's rate. Nothing else is merged nor dropped: the release that
/// commits a drag, which an editor applies only then, always follows what came before
/// it. Should [`MAX_GESTURES`] of those wait all the same, what waits is given up and a
/// [`Kind::Cancel`] stands for it: the editor ends the gesture it is in rather than
/// keep it open for an end that will not come.
#[derive(Debug)]
pub struct Gestures<T = ()> {
    queue: VecDeque<(T, Input)>,
}

impl<T> Default for Gestures<T> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl<T> Gestures<T> {
    /// Queues `input` with `tag`; false when the queue was full and the gestures waiting
    /// were cancelled instead.
    pub fn push(&mut self, tag: T, input: Input) -> bool {
        if let Some((last_tag, last)) = self.queue.back_mut() {
            if last.kind == Kind::Move
                && input.kind == Kind::Move
                && last.band.binding() == input.band.binding()
            {
                *last_tag = tag;
                *last = input;
                return true;
            }
        }
        if self.queue.len() >= MAX_GESTURES {
            self.queue.clear();
            self.queue.push_back((
                tag,
                Input {
                    kind: Kind::Cancel,
                    ..input
                },
            ));
            return false;
        }
        self.queue.push_back((tag, input));
        true
    }

    /// The oldest.
    pub fn pop(&mut self) -> Option<(T, Input)> {
        self.queue.pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Content, Side};

    fn ruler(side: Side) -> Band {
        let screen = Pixels {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        Band {
            monitor: "A".into(),
            connector: "A-connector".into(),
            screen,
            rect: if side.vertical() {
                Pixels {
                    width: 100,
                    ..screen
                }
            } else {
                Pixels {
                    height: 100,
                    ..screen
                }
            },
            side,
            scale: 1.0,
            axis_mm: 527.0,
            cross_pixels_per_mm: 3.6,
            content: Content::Ruler {
                zero_mm: 0.0,
                measured_mm: 527.0,
            },
            selected: None,
            passive: false,
        }
    }

    fn gesture(band: &Band, kind: Kind, at: f64) -> Input {
        Input::new(band, kind, (at, 10.0), 1.0, 0, false, false)
    }

    #[test]
    fn moves_merge_and_nothing_that_ends_a_drag_is_lost() {
        let top = ruler(Side::Top);
        let left = ruler(Side::Left);
        let mut gestures = Gestures::default();
        assert!(gestures.push(1, gesture(&top, Kind::Press, 10.0)));
        // A drag a slow reader does not keep up with: thousands of moves, one waiting.
        for at in 11..5000 {
            assert!(gestures.push(1 + at as u64 / 1000, gesture(&top, Kind::Move, at as f64)));
        }
        assert_eq!(gestures.len(), 2);
        assert!(gestures.push(6, gesture(&top, Kind::Release, 5000.0)));
        // A move after the release, or on another band, is a gesture of its own.
        assert!(gestures.push(6, gesture(&top, Kind::Move, 5001.0)));
        assert!(gestures.push(6, gesture(&left, Kind::Move, 3.0)));
        assert!(gestures.push(6, gesture(&top, Kind::Wheel(1.0), 3.0)));
        assert!(gestures.push(6, gesture(&top, Kind::Wheel(1.0), 3.0)));
        let out: Vec<(u64, Kind, f64)> = std::iter::from_fn(|| gestures.pop())
            .map(|(tag, input)| (tag, input.kind, input.at))
            .collect();
        assert_eq!(
            out,
            [
                (1, Kind::Press, 10.0),
                (5, Kind::Move, 4999.0),
                (6, Kind::Release, 5000.0),
                (6, Kind::Move, 5001.0),
                // Along its own axis: a left ruler's runs down the screen.
                (6, Kind::Move, 10.0),
                (6, Kind::Wheel(1.0), 3.0),
                (6, Kind::Wheel(1.0), 3.0),
            ]
        );
        assert!(gestures.is_empty());
    }

    #[test]
    fn a_full_queue_cancels_what_waits_rather_than_lose_an_end() {
        let top = ruler(Side::Top);
        let mut gestures: Gestures = Gestures::default();
        for i in 0..MAX_GESTURES {
            let kind = if i % 2 == 0 {
                Kind::Press
            } else {
                Kind::Release
            };
            assert!(gestures.push((), gesture(&top, kind, i as f64)));
        }
        assert!(!gestures.push((), gesture(&top, Kind::Press, 7.0)));
        let left: Vec<Kind> = std::iter::from_fn(|| gestures.pop())
            .map(|(_, input)| input.kind)
            .collect();
        assert_eq!(left, [Kind::Cancel]);
        // And on with what comes next.
        assert!(gestures.push((), gesture(&top, Kind::Press, 8.0)));
        assert_eq!(gestures.len(), 1);
    }
}
