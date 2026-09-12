//! Both shapes of a stored border-resistance edge — port of
//! `Persistence/BorderSideDtoJsonConverter.cs`.
//!
//! - the historical `"Left": 20`: one number meaning "resist any crossing by 20 mm",
//!   read as `Move == Drag == 20`;
//! - the current `"Left": { "Move": …, "Drag": …, "Sections": [ … ] }`.
//!
//! Without the first, an old layout would fail to deserialize, the store would read
//! it as absent, and every existing user would silently lose their whole layout on
//! first run. Only the current shape is ever written.
//!
//! The C# converter reads the object by hand, and its leniency is reproduced as is:
//! a `Move`/`Drag` that is not a number, or a `MoveBlock`/`DragBlock` that is not a
//! boolean, reads as absent instead of failing — unless that value is, or contains,
//! an object, which throws the C# reader off its position and fails the whole
//! document (see `tolerated` below).

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json_format::Finite;
use crate::layout_dtos::{BorderSectionDto, BorderSideDto};

const MOVE: &str = "Move";
const MOVE_BLOCK: &str = "MoveBlock";
const DRAG: &str = "Drag";
const DRAG_BLOCK: &str = "DragBlock";
const SECTIONS: &str = "Sections";

impl Serialize for BorderSideDto {
    /// C# `BorderSideDtoJsonConverter.Write`: always the object shape, absent members
    /// skipped, `Sections` only when it holds at least one section; then the unknown
    /// members (not in C#).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(value) = self.r#move {
            map.serialize_entry(MOVE, &Finite(value))?;
        }
        if let Some(value) = self.move_block {
            map.serialize_entry(MOVE_BLOCK, &value)?;
        }
        if let Some(value) = self.drag {
            map.serialize_entry(DRAG, &Finite(value))?;
        }
        if let Some(value) = self.drag_block {
            map.serialize_entry(DRAG_BLOCK, &value)?;
        }
        if let Some(sections) = self.sections.as_ref().filter(|s| !s.is_empty()) {
            map.serialize_entry(SECTIONS, sections)?;
        }
        for (name, value) in &self.extra {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for BorderSideDto {
    /// C# `BorderSideDtoJsonConverter.Read`: a number is the legacy shape, an object
    /// the current one, anything else fails the document. (A JSON `null` never gets
    /// here: the edges are `Option`s, as they are nullable in C#.)
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(BorderSideVisitor)
    }
}

/// The legacy shape: a single resistance governing every crossing.
fn legacy(value: f64) -> BorderSideDto {
    BorderSideDto {
        r#move: Some(value),
        drag: Some(value),
        ..BorderSideDto::default()
    }
}

struct BorderSideVisitor;

impl<'de> Visitor<'de> for BorderSideVisitor {
    type Value = BorderSideDto;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a border resistance side: a number (legacy shape) or an object")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<BorderSideDto, E> {
        Ok(legacy(value as f64))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<BorderSideDto, E> {
        Ok(legacy(value as f64))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<BorderSideDto, E> {
        Ok(legacy(value))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<BorderSideDto, A::Error> {
        let mut dto = BorderSideDto::default();
        // A repeated member overwrites the previous one, as in the C# loop.
        while let Some(name) = map.next_key::<String>()? {
            match name.as_str() {
                MOVE => dto.r#move = nullable_double(MOVE, map.next_value()?)?,
                MOVE_BLOCK => dto.move_block = nullable_bool(MOVE_BLOCK, map.next_value()?)?,
                DRAG => dto.drag = nullable_double(DRAG, map.next_value()?)?,
                DRAG_BLOCK => dto.drag_block = nullable_bool(DRAG_BLOCK, map.next_value()?)?,
                SECTIONS => dto.sections = map.next_value::<Option<Vec<BorderSectionDto>>>()?,
                _ => {
                    dto.extra.insert(name, map.next_value()?);
                }
            }
        }
        Ok(dto)
    }
}

/// C# `BorderSideDtoJsonConverter.ReadNullableDouble`: a number, or absent.
fn nullable_double<E: de::Error>(name: &str, value: Value) -> Result<Option<f64>, E> {
    match value {
        Value::Number(number) => Ok(number.as_f64()),
        other => tolerated(name, &other).map(|()| None),
    }
}

/// C# `BorderSideDtoJsonConverter.ReadNullableBool`: a boolean, or absent.
fn nullable_bool<E: de::Error>(name: &str, value: Value) -> Result<Option<bool>, E> {
    match value {
        Value::Bool(flag) => Ok(Some(flag)),
        other => tolerated(name, &other).map(|()| None),
    }
}

/// Whether the C# converter survives a wrong-typed `Move`/`MoveBlock`/`Drag`/
/// `DragBlock` value. It never skips such a value: it reads on, token by token, as if
/// the value's content were members of the side. An array of scalars goes by
/// harmlessly; the first object inside it (or the value being an object) makes the
/// converter return at that object's end, one level too deep, and `System.Text.Json`
/// then fails the whole document ("read too much or not enough").
fn tolerated<E: de::Error>(name: &str, value: &Value) -> Result<(), E> {
    if holds_object(value) {
        Err(E::custom(format!(
            "border resistance member {name} holds an object, which the C# reader cannot skip"
        )))
    } else {
        Ok(())
    }
}

/// Whether `value` is an object or an array with an object somewhere inside.
fn holds_object(value: &Value) -> bool {
    match value {
        Value::Object(_) => true,
        Value::Array(items) => items.iter().any(holds_object),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn read(value: Value) -> Result<BorderSideDto, serde_json::Error> {
        BorderSideDto::deserialize(value)
    }

    #[test]
    fn a_bare_number_is_the_legacy_shape() {
        for value in [json!(20), json!(-3), json!(2.5)] {
            let side = read(value.clone()).unwrap();
            let expected = value.as_f64();
            assert_eq!(side.r#move, expected);
            assert_eq!(side.drag, expected);
            assert_eq!(side.move_block, None);
            assert_eq!(side.drag_block, None);
            assert_eq!(side.sections, None);
        }
    }

    #[test]
    fn other_tokens_fail() {
        for value in [json!("20"), json!(true), json!([20])] {
            assert!(read(value).is_err());
        }
    }

    #[test]
    fn wrong_typed_scalars_read_as_absent() {
        let side = read(json!({
            "Move": "fast", "MoveBlock": 1, "Drag": null, "DragBlock": [1, [2]]
        }))
        .unwrap();
        assert_eq!(side, BorderSideDto::default());
    }

    #[test]
    fn wrong_typed_containers_holding_an_object_fail() {
        for value in [json!({}), json!([1, {"Drag": 5}]), json!([[{}]])] {
            assert!(read(json!({ "Move": value.clone() })).is_err());
            assert!(read(json!({ "DragBlock": value })).is_err());
        }
    }

    #[test]
    fn sections_must_be_an_array_of_objects() {
        assert!(read(json!({ "Sections": 3 })).is_err());
        assert!(read(json!({ "Sections": [null] })).is_err());
        assert_eq!(read(json!({ "Sections": null })).unwrap().sections, None);
    }

    #[test]
    fn empty_sections_are_not_written() {
        let side = BorderSideDto {
            sections: Some(Vec::new()),
            ..BorderSideDto::default()
        };
        assert_eq!(serde_json::to_value(&side).unwrap(), json!({}));
    }

    #[test]
    fn the_current_shape_is_written_in_the_csharp_order() {
        let mut side = read(json!({
            "Future": 1, "Sections": [{ "To": 3 }], "DragBlock": true,
            "Drag": 2, "MoveBlock": false, "Move": 1
        }))
        .unwrap();
        side.extra.insert("Later".into(), json!("x"));
        let written = serde_json::to_value(&side).unwrap();
        let names: Vec<&str> = written
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names,
            [
                "Move",
                "MoveBlock",
                "Drag",
                "DragBlock",
                "Sections",
                "Future",
                "Later"
            ]
        );
        assert_eq!(written["Sections"], json!([{ "To": 3.0 }]));
    }

    #[test]
    fn a_non_finite_resistance_fails_the_write() {
        let side = BorderSideDto {
            drag: Some(f64::INFINITY),
            ..BorderSideDto::default()
        };
        assert!(serde_json::to_string(&side).is_err());
    }
}
