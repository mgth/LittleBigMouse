//! The two monitor orders behind Windows' per-monitor wallpaper.
//!
//! `IDesktopWallpaper` files an image under the slot of the device path it is written
//! through (the `GetMonitorDevicePathAt` order), but it paints slot `k` on the k-th monitor
//! of another order: the primary first, then the others in filing order. On a layout whose
//! primary is not the first enumerated monitor (a portrait screen on DP at the left, the
//! landscape primary on HDMI at the right) every image lands one screen off and the desktop
//! shows the screens swapped. Reported, and the order established on that two-monitor
//! layout, by yinlichaoxi007 in mgth/LittleBigMouse#789.
//!
//! Both ends use the same pairing: the writer files each image under the slot that paints
//! its screen; the reader attributes a slot's image to the screen it is painted on.
//!
//! The pairing is positional, so a slot whose lookup failed keeps its position as a hole:
//! dropping it would shift every later slot by one and send an image to a monitor nobody
//! configured. A pair that touches a hole is left out of [`Plan::pairs`].
//!
//! Pure and platform-free, so it is tested on any host; the COM calls live with their
//! callers.

/// A monitor rectangle as `GetMonitorRECT` gives it, in desktop pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl SlotRect {
    fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    fn is_origin(self) -> bool {
        self.left == 0 && self.top == 0
    }
}

/// One readable slot: the path it is filed under, that monitor's rectangle, and what
/// `GetWallpaper` returned for it — `""` for a solid colour, `None` when the call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub path: String,
    pub rect: SlotRect,
    pub image: Option<String>,
}

/// The raw read of one enumerated slot, failures included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotRead {
    /// `GetMonitorDevicePathAt` or `GetMonitorRECT` failed: maybe an active monitor we
    /// cannot place, maybe a stale entry.
    Unknown,
    /// Read fine, with an empty rectangle: listed but not on the desktop.
    Inactive,
    Active(Slot),
}

impl SlotRead {
    /// Classifies a read that got this far: a rectangle, or none when its lookup failed.
    pub fn from_parts(path: Option<String>, rect: Option<SlotRect>, image: Option<String>) -> Self {
        match (path, rect) {
            (Some(path), Some(rect)) if !rect.is_empty() => {
                SlotRead::Active(Slot { path, rect, image })
            }
            (Some(_), Some(_)) => SlotRead::Inactive,
            _ => SlotRead::Unknown,
        }
    }
}

/// The pairing of one snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// `(filed, painted)`: the image filed under `filed.path` is painted on `painted.rect`.
    /// Only the pairs where both slots were read.
    pub pairs: Vec<(Slot, Slot)>,
    /// False when some active monitor could not be read.
    pub complete: bool,
}

/// For each filing index, the filing index of the slot it is painted on: the primary (the
/// monitor at the origin of the virtual desktop) first, the rest in filing order. Identity
/// when the primary is already first, or when no monitor sits at the origin.
pub fn paint_order(primary: Option<usize>, len: usize) -> Vec<usize> {
    let mut painted = Vec::with_capacity(len);
    painted.extend(primary.filter(|&p| p < len));
    painted.extend((0..len).filter(|&i| Some(i) != primary));
    painted
}

/// Pairs every filed slot with the slot the shell paints it on.
///
/// `active_monitors` (the desktop monitor count, `SM_CMONITORS`) tells an unreadable slot
/// that is an active monitor from a stale entry: when every active monitor was read, the
/// unreadable ones are not on the desktop and drop out; when exactly the unreadable ones are
/// missing, they stay as holes. Any other count, or holes among which the primary cannot be
/// placed, make the snapshot unusable: `None`.
pub fn plan(reads: &[SlotRead], active_monitors: usize) -> Option<Plan> {
    let known = reads
        .iter()
        .filter(|r| matches!(r, SlotRead::Active(_)))
        .count();
    let unknown = reads
        .iter()
        .filter(|r| matches!(r, SlotRead::Unknown))
        .count();

    // Active monitors in filing order; `None` marks a hole.
    let active: Vec<Option<&Slot>> = if unknown == 0 || known == active_monitors {
        reads
            .iter()
            .filter_map(|r| match r {
                SlotRead::Active(s) => Some(Some(s)),
                _ => None,
            })
            .collect()
    } else if active_monitors > 0 && known + unknown == active_monitors {
        reads
            .iter()
            .filter_map(|r| match r {
                SlotRead::Active(s) => Some(Some(s)),
                SlotRead::Unknown => Some(None),
                SlotRead::Inactive => None,
            })
            .collect()
    } else {
        return None;
    };

    if active.is_empty() {
        return None;
    }

    let holes = active.iter().filter(|s| s.is_none()).count();
    let primary = match active
        .iter()
        .position(|s| s.is_some_and(|s| s.rect.is_origin()))
    {
        Some(p) => Some(p),
        // No readable slot at the origin: the primary is a hole, placeable if it is the only one.
        None if holes == 1 => active.iter().position(Option::is_none),
        None if holes > 1 => return None,
        None => None,
    };

    let painted = paint_order(primary, active.len());
    let pairs = (0..active.len())
        .filter_map(|i| Some((active[i]?.clone(), active[painted[i]]?.clone())))
        .collect();

    Some(Plan {
        pairs,
        complete: holes == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: SlotRect = SlotRect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    const LEFT: SlotRect = SlotRect {
        left: -1080,
        top: -420,
        right: 0,
        bottom: 1500,
    };
    const RIGHT: SlotRect = SlotRect {
        left: 1920,
        top: 0,
        right: 3840,
        bottom: 1080,
    };

    fn active(path: &str, rect: SlotRect) -> SlotRead {
        SlotRead::from_parts(Some(path.into()), Some(rect), Some(format!("{path}.png")))
    }

    fn pairs(plan: &Plan) -> Vec<(&str, &str)> {
        plan.pairs
            .iter()
            .map(|(f, p)| (f.path.as_str(), p.path.as_str()))
            .collect()
    }

    #[test]
    fn primary_not_first_swaps_the_pair() {
        let plan = plan(&[active("L", LEFT), active("P", PRIMARY)], 2).unwrap();
        assert_eq!(pairs(&plan), [("L", "P"), ("P", "L")]);
        assert!(plan.complete);
    }

    #[test]
    fn primary_first_is_identity() {
        let plan = plan(&[active("P", PRIMARY), active("L", LEFT)], 2).unwrap();
        assert_eq!(pairs(&plan), [("P", "P"), ("L", "L")]);
    }

    #[test]
    fn a_failed_lookup_keeps_its_position() {
        // Filing L, R, P → painting P, L, R. Dropping R would pair L with P and P with L:
        // P's slot, painted on R, would receive L's image.
        let reads = [active("L", LEFT), SlotRead::Unknown, active("P", PRIMARY)];
        let plan = plan(&reads, 3).unwrap();
        assert_eq!(pairs(&plan), [("L", "P")]);
        assert!(!plan.complete);
    }

    #[test]
    fn a_stale_entry_drops_out_when_every_active_monitor_was_read() {
        let reads = [active("L", LEFT), SlotRead::Unknown, active("P", PRIMARY)];
        let plan = plan(&reads, 2).unwrap();
        assert_eq!(pairs(&plan), [("L", "P"), ("P", "L")]);
        assert!(plan.complete);
    }

    #[test]
    fn an_inactive_entry_is_not_a_monitor() {
        let reads = [
            active("L", LEFT),
            SlotRead::from_parts(
                Some("X".into()),
                Some(SlotRect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                }),
                None,
            ),
            active("P", PRIMARY),
        ];
        assert_eq!(pairs(&plan(&reads, 2).unwrap()), [("L", "P"), ("P", "L")]);
    }

    #[test]
    fn a_count_that_does_not_add_up_is_unusable() {
        let reads = [active("P", PRIMARY), SlotRead::Unknown, SlotRead::Unknown];
        assert_eq!(plan(&reads, 2), None);
    }

    #[test]
    fn an_unreadable_primary_is_placed_when_it_is_the_only_hole() {
        // Filing L, P, R → painting P, L, R: only R pairs two readable slots.
        let reads = [active("L", LEFT), SlotRead::Unknown, active("R", RIGHT)];
        let plan = plan(&reads, 3).unwrap();
        assert_eq!(pairs(&plan), [("R", "R")]);
    }

    #[test]
    fn paint_order_puts_the_primary_first() {
        assert_eq!(paint_order(Some(2), 3), [2, 0, 1]);
        assert_eq!(paint_order(Some(0), 3), [0, 1, 2]);
        assert_eq!(paint_order(None, 2), [0, 1]);
    }
}
