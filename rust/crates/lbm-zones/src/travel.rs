//! Port of the free functions `Reachable` / `Travel` from `Engine/Zone.cpp` —
//! the sequence of clip rectangles the cursor must pass through to travel from a
//! source pixel rect to a target pixel rect without escaping the desktop.

use crate::geometry::Rect;

/// C++ `Reachable`.
pub fn reachable(source: Rect<i32>, target: Rect<i32>) -> Vec<Rect<i32>> {
    let left = target.left().max(source.left());
    let right = target.right().min(source.right());
    let top = target.top().max(source.top());
    let bottom = target.bottom().min(source.bottom());

    if left >= right {
        if top >= bottom {
            return vec![source];
        }
        let start = Rect::new(source.left(), top, source.width(), bottom - top);
        let dest = Rect::new(target.left(), top, target.width(), bottom - top);
        return vec![start, dest];
    }

    if top >= bottom {
        let start = Rect::new(left, source.top(), right - left, source.height());
        let dest = Rect::new(left, target.top(), right - left, target.height());
        return vec![start, dest];
    }

    let start = Rect::new(left, top, right - left, bottom - top);
    vec![start, start]
}

/// C++ `Travel`.
pub fn travel(source: Rect<i32>, target: Rect<i32>, allowed: &[Rect<i32>]) -> Vec<Rect<i32>> {
    let reachable = reachable(source, target);
    if reachable.len() > 1 {
        return reachable;
    }
    if allowed.is_empty() {
        return Vec::new();
    }

    for next in allowed {
        let new_allowed: Vec<Rect<i32>> = allowed
            .iter()
            .copied()
            .filter(|value| {
                value.left() != next.left()
                    && value.top() != next.top()
                    && value.right() != next.right()
                    && value.bottom() != next.bottom()
            })
            .collect();

        let tail = travel(*next, target, &new_allowed);
        if tail.is_empty() {
            continue;
        }

        let mut trav = travel(source, *next, &new_allowed);
        if !trav.is_empty() {
            trav.extend(tail);
            return trav;
        }
    }

    Vec::new()
}
