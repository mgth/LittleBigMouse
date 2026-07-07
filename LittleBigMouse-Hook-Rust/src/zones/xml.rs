//! roxmltree attribute getters — port of `Xml/XmlHelper`.
//!
//! Incoming values use C# `InvariantCulture`: booleans `True`/`False`, doubles
//! like `1.7976931348623157E+308` (= `f64::MAX`), ints down to `-2147483648`
//! (= `i32::MIN`). Missing attribute -> the C++ default (`0` / `0.0` / `""`).

use roxmltree::Node;

use crate::geometry::Rect;

/// C++ `XmlHelper::GetBool`.
pub fn get_bool(node: Node, name: &str, default: bool) -> bool {
    match node.attribute(name) {
        Some(v) => {
            if default {
                v != "False"
            } else {
                v == "True"
            }
        }
        None => default,
    }
}

/// C++ `XmlHelper::GetLong` (Win32 `long` == `i32`).
pub fn get_i32(node: Node, name: &str) -> i32 {
    node.attribute(name)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// C++ `XmlHelper::GetDouble`.
pub fn get_f64(node: Node, name: &str) -> f64 {
    node.attribute(name)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0)
}

/// C++ `XmlHelper::GetString`.
pub fn get_string(node: Node, name: &str) -> String {
    node.attribute(name).unwrap_or("").to_string()
}

pub fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|c| c.has_tag_name(name))
}

/// C++ `XmlHelper::GetRectLong` — the `<name><Rect Left Top Width Height/></name>`
/// nesting.
pub fn get_rect_i32(parent: Node, name: &str) -> Rect<i32> {
    child(parent, name)
        .and_then(|el| child(el, "Rect"))
        .map(|r| {
            Rect::new(
                get_i32(r, "Left"),
                get_i32(r, "Top"),
                get_i32(r, "Width"),
                get_i32(r, "Height"),
            )
        })
        .unwrap_or_else(Rect::empty)
}

/// C++ `XmlHelper::GetRectDouble`.
pub fn get_rect_f64(parent: Node, name: &str) -> Rect<f64> {
    child(parent, name)
        .and_then(|el| child(el, "Rect"))
        .map(|r| {
            Rect::new(
                get_f64(r, "Left"),
                get_f64(r, "Top"),
                get_f64(r, "Width"),
                get_f64(r, "Height"),
            )
        })
        .unwrap_or_else(Rect::empty)
}
