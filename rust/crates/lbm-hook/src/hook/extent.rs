//! How far the zones in play reach: the rectangle a master's virtual cursor may move in,
//! and the one of this machine's own zones. Shared by the masters that keep a virtual
//! cursor of their own (evdev on Linux, the low-level hooks on Windows).

use crate::geometry::Rect;

/// `(every zone in play, this machine's zones)`, each as one rectangle. One pass, no
/// allocation: a master calls it every cycle.
pub(crate) fn extents(layout: &crate::zones::ZonesLayout) -> (Rect<i32>, Rect<i32>) {
    // (left, top, right, bottom), or None while nothing was seen.
    type Union = Option<(i32, i32, i32, i32)>;
    fn grow(union: &mut Union, z: Rect<i32>) {
        let (l, t, r, b) = union.get_or_insert((z.left(), z.top(), z.right(), z.bottom()));
        *l = (*l).min(z.left());
        *t = (*t).min(z.top());
        *r = (*r).max(z.right());
        *b = (*b).max(z.bottom());
    }
    fn rect(union: Union) -> Rect<i32> {
        match union {
            Some((l, t, r, b)) => Rect::new(l, t, r - l, b - t),
            None => Rect::new(0, 0, 1920, 1080),
        }
    }
    let (mut all, mut local): (Union, Union) = (None, None);
    for &id in &layout.main_zones {
        let zone = &layout.arena[id];
        grow(&mut all, zone.pixels_bounds());
        if zone.host.is_none() {
            grow(&mut local, zone.pixels_bounds());
        }
    }
    (rect(all), rect(local))
}
