//! Storage documents — port of `Persistence/LayoutDtos.cs`.
//!
//! Shared by every backend: the JSON files here, the registry on Windows. Every field
//! is optional: `None` means "absent from the store", and the engine then keeps the
//! live model value. The JSON names are load-bearing — they are the C# property names,
//! which `System.Text.Json` writes as declared (no naming policy) and matches
//! case-sensitively — so renaming one is a data migration, not a refactoring.
//!
//! Serde mirrors `JsonLayoutStore`'s `JsonSerializerOptions` field by field:
//!
//! - `DefaultIgnoreCondition = WhenWritingNull` → `skip_serializing_if = "Option::is_none"`
//!   on every optional member. A non-optional member is always written, like the C#
//!   `LayoutDto.Monitors`, which is written as `{}` when empty;
//! - declaration order is kept, so members come out in the C# order;
//! - numbers: `double?` → `Option<f64>`, `int?` → `Option<i32>` (a C# `int` refuses
//!   `1.0`, and so does an `i32`); a NaN or an infinity fails the write, as the C#
//!   serializer throws on them, instead of being written as `null`;
//! - unknown members, which `System.Text.Json` skips, land in `extra` and are written
//!   back after the known ones (see the crate documentation).
//!
//! Dictionaries are [`IndexMap`]s: a C# `Dictionary` enumerates in insertion order,
//! which is the file order after a read, and the C# writer emits them that way.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::json_format::finite;

/// The members of a stored JSON object that its DTO does not know, in file order.
///
/// Not in the C# DTOs, which drop them on read. Filled by the reader, written back
/// after the known members. The reader never routes a known member name here; a key
/// inserted by hand under a known name would be written twice.
pub type UnknownMembers = Map<String, Value>;

/// C# `GlobalOptionsDto`: app-level options, stored once (registry root key /
/// `options.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GlobalOptionsDto {
    /// C# `GlobalOptionsDto.Priority`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// C# `GlobalOptionsDto.PriorityUnhooked`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_unhooked: Option<String>,
    /// C# `GlobalOptionsDto.HomeCinema`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_cinema: Option<bool>,
    /// C# `GlobalOptionsDto.VcpControl`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vcp_control: Option<bool>,
    /// C# `GlobalOptionsDto.Pinned`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    /// C# `GlobalOptionsDto.AutoUpdate`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    /// C# `GlobalOptionsDto.StartMinimized`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_minimized: Option<bool>,
    /// C# `GlobalOptionsDto.StartElevated`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_elevated: Option<bool>,
    /// C# `GlobalOptionsDto.DebugTools`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_tools: Option<bool>,
    /// C# `GlobalOptionsDto.ExperimentalFeatures`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_features: Option<bool>,
    /// C# `GlobalOptionsDto.ShowMonitorActionWarning`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_monitor_action_warning: Option<bool>,
    /// C# `GlobalOptionsDto.BorderValues`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_values: Option<String>,
    /// C# `GlobalOptionsDto.RescueShortcut`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rescue_shortcut: Option<String>,
    /// C# `GlobalOptionsDto.HideTrayIcon`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_tray_icon: Option<bool>,
    /// C# `GlobalOptionsDto.ExcludedDefaultsVersion`: the version of the
    /// excluded-defaults top-up already applied (see
    /// [`ExcludedListPersistence`](crate::ExcludedListPersistence)). Not mapped to the
    /// options model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_defaults_version: Option<i32>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `LayoutDto`: one layout document (registry `Layouts\{id}` key /
/// `layouts/{key}.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutDto {
    /// C# `LayoutDto.Options`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<LayoutOptionsDto>,
    /// C# `LayoutDto.Monitors`, keyed by monitor id. Never null in C# either (it is
    /// initialised empty), so it is always written, `{}` when empty.
    ///
    /// An explicit `"Monitors": null` fails the read of the whole document here. C#
    /// reads it as a null dictionary and then throws in the mapper, at load.
    #[serde(default)]
    pub monitors: IndexMap<String, MonitorDto>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `LayoutOptionsDto`: per-layout options.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutOptionsDto {
    /// C# `LayoutOptionsDto.AllowOverlaps`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_overlaps: Option<bool>,
    /// C# `LayoutOptionsDto.AllowDiscontinuity`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_discontinuity: Option<bool>,
    /// C# `LayoutOptionsDto.Algorithm`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    /// C# `LayoutOptionsDto.MinimalEdgeOverlap`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub minimal_edge_overlap: Option<f64>,
    /// C# `LayoutOptionsDto.MaxTravelDistance`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub max_travel_distance: Option<f64>,
    /// C# `LayoutOptionsDto.FreelookCheckInterval`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub freelook_check_interval: Option<f64>,
    /// C# `LayoutOptionsDto.FreelookEnabled`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freelook_enabled: Option<bool>,
    /// C# `LayoutOptionsDto.LoopX`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_x: Option<bool>,
    /// C# `LayoutOptionsDto.LoopY`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_y: Option<bool>,
    /// C# `LayoutOptionsDto.Enabled`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// C# `LayoutOptionsDto.AdjustPointer`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjust_pointer: Option<bool>,
    /// C# `LayoutOptionsDto.AdjustSpeed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjust_speed: Option<bool>,
    /// C# `LayoutOptionsDto.Priority` — READ-ONLY legacy. An app-level option (see
    /// [`GlobalOptionsDto`]) that older versions also stored per layout; both
    /// locations load into the same options property and only the app-level one is
    /// written from now on. Kept so a layout that still carries it keeps its priority
    /// on upgrade.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// C# `LayoutOptionsDto.PriorityUnhooked` — READ-ONLY legacy, like
    /// [`priority`](Self::priority).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_unhooked: Option<String>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `MonitorDto`: one monitor of a layout.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MonitorDto {
    /// C# `MonitorDto.XLocationInMm`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub x_location_in_mm: Option<f64>,
    /// C# `MonitorDto.YLocationInMm`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub y_location_in_mm: Option<f64>,
    /// C# `MonitorDto.PhysicalRatioX`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub physical_ratio_x: Option<f64>,
    /// C# `MonitorDto.PhysicalRatioY`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub physical_ratio_y: Option<f64>,
    /// C# `MonitorDto.BorderResistance`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_resistance: Option<BorderResistanceDto>,
    /// C# `MonitorDto.Borders`: per-monitor bezel borders. PRESENCE is the flag:
    /// `Some` means the monitor owns its borders (BordersCustomized); an uncustomized
    /// monitor mirrors its model live and stores nothing here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<BordersDto>,
    /// C# `MonitorDto.ActiveSource`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_source: Option<String>,
    /// C# `MonitorDto.SerialNumber` — WRITE-ONLY: saved from the live model, never
    /// applied back (the running value comes from the EDID). Stored because it says
    /// which physical panel was behind which id in a dumped configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    /// C# `MonitorDto.ExcludedFromLayout`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_from_layout: Option<bool>,
    /// C# `MonitorDto.Sources`, keyed by source id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<IndexMap<String, SourceDto>>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `SourceDto`: a display source's pixel geometry, stored to be restored when the
/// monitor is re-attached to the desktop (only applied on load while the source is
/// detached).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SourceDto {
    /// C# `SourceDto.PixelX`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub pixel_x: Option<f64>,
    /// C# `SourceDto.PixelY`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub pixel_y: Option<f64>,
    /// C# `SourceDto.PixelWidth`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub pixel_width: Option<f64>,
    /// C# `SourceDto.PixelHeight`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub pixel_height: Option<f64>,
    /// C# `SourceDto.Orientation`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<i32>,
    /// C# `SourceDto.DisplayName` — WRITE-ONLY, like
    /// [`MonitorDto::serial_number`]: it comes from the OS at every start and is saved
    /// for what it says about a stored configuration, not to be restored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// C# `SourceDto.Primary` — WRITE-ONLY, like [`display_name`](Self::display_name):
    /// which display is primary is the desktop's business.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `BordersDto`: bezel widths, in millimetres.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BordersDto {
    /// C# `BordersDto.Left`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub left: Option<f64>,
    /// C# `BordersDto.Top`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub top: Option<f64>,
    /// C# `BordersDto.Right`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub right: Option<f64>,
    /// C# `BordersDto.Bottom`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub bottom: Option<f64>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `BorderResistanceDto`: border resistance, one entry per edge.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BorderResistanceDto {
    /// C# `BorderResistanceDto.Left`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<BorderSideDto>,
    /// C# `BorderResistanceDto.Top`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<BorderSideDto>,
    /// C# `BorderResistanceDto.Right`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<BorderSideDto>,
    /// C# `BorderResistanceDto.Bottom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<BorderSideDto>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `BorderSideDto`: one edge's resistances.
///
/// Layouts saved before the move/drag split stored a bare number here instead of an
/// object; its (de)serialization, in [`border_side_json`](crate::border_side_json),
/// reads both shapes and writes the current one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BorderSideDto {
    /// C# `BorderSideDto.Move` (JSON `Move`).
    pub r#move: Option<f64>,
    /// C# `BorderSideDto.MoveBlock`.
    pub move_block: Option<bool>,
    /// C# `BorderSideDto.Drag`.
    pub drag: Option<f64>,
    /// C# `BorderSideDto.DragBlock`.
    pub drag_block: Option<bool>,
    /// C# `BorderSideDto.Sections`. An empty list is not written (C# writes the
    /// member only when it holds a section), so it reads back as `None`.
    pub sections: Option<Vec<BorderSectionDto>>,
    /// Members this version does not know (not in C#: the converter skips them).
    pub extra: UnknownMembers,
}

/// C# `BorderSectionDto`: one stretch of an edge with its own resistances.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BorderSectionDto {
    /// C# `BorderSectionDto.From`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub from: Option<f64>,
    /// C# `BorderSectionDto.To`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub to: Option<f64>,
    /// C# `BorderSectionDto.Move`.
    #[serde(
        rename = "Move",
        skip_serializing_if = "Option::is_none",
        serialize_with = "finite"
    )]
    pub r#move: Option<f64>,
    /// C# `BorderSectionDto.MoveBlock`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_block: Option<bool>,
    /// C# `BorderSectionDto.Drag`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub drag: Option<f64>,
    /// C# `BorderSectionDto.DragBlock`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drag_block: Option<bool>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}

/// C# `ModelDto`: per-model (PnP code) physical size, shared across layouts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ModelDto {
    /// C# `ModelDto.Width`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub width: Option<f64>,
    /// C# `ModelDto.Height`.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "finite")]
    pub height: Option<f64>,
    /// C# `ModelDto.Borders`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<BordersDto>,
    /// C# `ModelDto.PnpName`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pnp_name: Option<String>,
    /// Members this version does not know (not in C#).
    #[serde(flatten)]
    pub extra: UnknownMembers,
}
