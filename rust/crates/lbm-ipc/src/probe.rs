//! Correlated, read-only edge reports for an editor. Legacy Probe/Probed stays XML.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Report {
    pub layout_id: String,
    pub layout_revision: u64,
    pub zones: Vec<Zone>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Zone {
    pub id: i32,
    pub name: String,
    pub device_id: String,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Edge {
    pub side: Side,
    pub runs: Vec<Run>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Run {
    /// Inclusive desktop pixel coordinates along the edge.
    pub from: i32,
    pub to: i32,
    /// None is a wall. Otherwise the id of the destination main zone.
    pub target: Option<i32>,
}
