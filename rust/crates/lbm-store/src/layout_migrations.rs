//! How values written by older versions are read — port of
//! `Persistence/LayoutMigrations.cs`.
//!
//! Every reinterpretation of OLD stored data is gathered here, so the mapping stays a
//! plain field-for-field copy. A migration lives here when reading a value requires
//! knowing WHEN it was written; the shape-level compatibility (a bare number where an
//! object is now expected) belongs to the stores instead — [`border_side_json`] here,
//! `RegistryLayoutStore.ReadSide` on Windows — because it is the deserialization that
//! would otherwise fail.
//!
//! The migrations are independent of one another and are applied per value, in the
//! order the mapper reads the DTO. None of them writes: a migrated value only reaches
//! the store at the next save, when it is written in the current shape, and re-reading
//! it is then a permanent no-op. History (oldest first):
//!
//! 1. [`sections`] — pre-section-editor whole-edge resistance (one Move/Drag pair per
//!    edge) becomes the section that says the same thing.
//! 2. [`stored_model_size`], 0x0 case — pre-EDID-fallback placeholder size (#419) must
//!    not override the freshly computed one.
//! 3. [`normalize_stored_size`] — pre-5.4.1 model sizes were stored ORIENTED to the
//!    rotation at save time; they are transposed back to intrinsic (#507).
//!
//! The one-time top-up of the default exclusion list is a migration too, but it is
//! stateful (it reads and writes files and a version counter) and lives with the list
//! it migrates, in [`ExcludedListPersistence`](crate::ExcludedListPersistence).
//!
//! [`border_side_json`]: crate::border_side_json

use crate::layout_dtos::{BorderSectionDto, BorderSideDto};

/// C# `LayoutMigrations.Sections`: the sections an edge actually holds. Stored sections
/// win when there are any; otherwise an edge saved before the section editor carried a
/// single resistance over its whole length, and rather than keep that notion alongside
/// the sections it becomes the section that says the same thing — so an existing
/// setting stays in force AND shows up in the editor, where it can be split or trimmed
/// like any other.
///
/// `edge_length_mm` is the edge's current length, needed to give the migrated section
/// its extent. A non-positive length (nothing to span) drops the legacy value — see
/// [`legacy_whole_edge_section`].
pub fn sections(dto: &BorderSideDto, edge_length_mm: f64) -> Vec<BorderSectionDto> {
    if let Some(stored) = dto.sections.as_ref().filter(|s| !s.is_empty()) {
        return stored.clone();
    }
    legacy_whole_edge_section(dto, edge_length_mm)
        .into_iter()
        .collect()
}

/// C# `LayoutMigrations.LegacyWholeEdgeSection`: the whole-edge section standing for a
/// pre-section-editor resistance, or `None` when there is nothing to migrate.
///
/// CAVEAT (C#'s): a legacy resistance on an edge of unknown length is dropped rather
/// than migrated — the section needs an extent and there is none to give it. In
/// practice the length comes from the monitor's depth projection, which is known by the
/// time the layout loads; this only bites a monitor whose size could not be computed at
/// all, which has no working resistance either way.
pub fn legacy_whole_edge_section(
    dto: &BorderSideDto,
    edge_length_mm: f64,
) -> Option<BorderSectionDto> {
    // `edgeLengthMm <= 0`: false for a NaN length, which then spans the section.
    if edge_length_mm <= 0.0 {
        return None;
    }

    let r#move = dto.r#move.unwrap_or(0.0);
    let drag = dto.drag.unwrap_or(0.0);
    let move_block = dto.move_block.unwrap_or(false);
    let drag_block = dto.drag_block.unwrap_or(false);

    // A zero, unblocked edge is what "no resistance" has always looked like: migrating
    // it would litter every layout with meaningless full-edge sections.
    if r#move <= 0.0 && drag <= 0.0 && !move_block && !drag_block {
        return None;
    }

    Some(BorderSectionDto {
        from: Some(0.0),
        to: Some(edge_length_mm),
        r#move: Some(r#move),
        move_block: Some(move_block),
        drag: Some(drag),
        drag_block: Some(drag_block),
        ..Default::default()
    })
}

/// C# `LayoutMigrations.StoredModelSize`: the stored physical size to apply over the
/// freshly computed (intrinsic) one. `None` components mean "keep the computed value".
///
/// Versions predating the EDID-less size fallback persisted the bogus 0x0 GDI
/// placeholder for virtual displays (#419): a stored non-positive size must not
/// override the computed one. A complete stored size then goes through
/// [`normalize_stored_size`]; a half-valid one (one dimension only) is applied as-is,
/// since a single dimension says nothing about orientation.
pub fn stored_model_size(
    intrinsic_width: f64,
    intrinsic_height: f64,
    stored_width: Option<f64>,
    stored_height: Option<f64>,
) -> (Option<f64>, Option<f64>) {
    // `is > 0`: false for a NaN, like a missing value.
    let positive = |v: Option<f64>| v.filter(|v| *v > 0.0);
    match (positive(stored_width), positive(stored_height)) {
        (Some(w), Some(h)) => {
            let (w, h) = normalize_stored_size(intrinsic_width, intrinsic_height, w, h);
            (Some(w), Some(h))
        }
        (w, h) => (w, h),
    }
}

/// C# `LayoutMigrations.NormalizeStoredSize`: migration of pre-5.4.1 stored model
/// sizes (#507).
///
/// The model used to persist the size ORIENTED to the display's rotation at save time;
/// since 5.4.1 it stores the intrinsic panel size and the projection chain applies the
/// rotation downstream. A stored portrait-oriented size read as intrinsic gets the
/// rotation applied twice: the monitor that was portrait at save time renders with the
/// orientation inverted after the upgrade. The freshly computed model size is intrinsic
/// by construction: when the stored orientation contradicts it, transpose the stored
/// value — the portrait/landscape signal is robust to user-customized magnitudes (edits
/// keep the panel aspect via `FixedAspectRatio`), which are preserved. Square or
/// invalid sizes decide nothing. Once the layout is saved again the store holds the
/// intrinsic size and this is a permanent no-op.
pub fn normalize_stored_size(
    intrinsic_width: f64,
    intrinsic_height: f64,
    stored_width: f64,
    stored_height: f64,
) -> (f64, f64) {
    if intrinsic_width <= 0.0
        || intrinsic_height <= 0.0
        || intrinsic_width == intrinsic_height
        || stored_width == stored_height
    {
        return (stored_width, stored_height);
    }

    let intrinsic_portrait = intrinsic_height > intrinsic_width;
    let stored_portrait = stored_height > stored_width;

    if stored_portrait == intrinsic_portrait {
        (stored_width, stored_height)
    } else {
        (stored_height, stored_width)
    }
}
