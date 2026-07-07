//! Port of `geo::rect_side`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    None,
    Left,
    Top,
    Right,
    Bottom,
}
