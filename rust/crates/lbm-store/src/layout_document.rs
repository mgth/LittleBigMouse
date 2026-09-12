//! A layout as a frontend hands it to the agent (v6: the agent is the only writer, the
//! frontends send what they would have saved).
//!
//! It is the documents a save writes — the app-level options, the layout document, its
//! models ([`LayoutPersistence::save`](crate::LayoutPersistence::save)) — plus the
//! excluded list, which lives in its own file. Applied to the agent's layout for the
//! same displays, it gives the layout a save then a load would have given: every value
//! the store keeps comes through, and nothing else is needed.
//!
//! On the wire it is JSON in the store's own shapes, PascalCase:
//!
//! ```text
//! {"GlobalOptions": {…}, "Layout": {"Options": {…}, "Monitors": {…}}, "Models": {…},
//!  "Excluded": ["…"]}
//! ```

use indexmap::IndexMap;
use lbm_layout::model::Layout;
use serde::{Deserialize, Serialize};

use crate::layout_dto_mapper as mapper;
use crate::layout_dtos::{GlobalOptionsDto, LayoutDto, ModelDto};

/// What a save of a layout writes, as one document.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_options: Option<GlobalOptionsDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutDto>,
    /// Keyed by PnP code.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub models: IndexMap<String, ModelDto>,
    /// The excluded processes; absent: left as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Vec<String>>,
}

impl LayoutDocument {
    /// What a save of `layout` would write (the mapper calls of a save).
    pub fn of(layout: &Layout) -> Self {
        LayoutDocument {
            // The defaults version belongs to the store, not to the layout.
            global_options: Some(mapper::to_global_options_dto(&layout.options, None)),
            layout: Some(mapper::to_layout_dto(layout)),
            models: mapper::to_model_dtos(layout),
            excluded: Some(layout.options.excluded_list.clone()),
        }
    }

    /// Applies it to `layout` as a load applies the store: the app-level options, the
    /// excluded list, then the layout's own options and per monitor its model and its
    /// state. What it leaves out is left as it is. Not marked saved: it is an edit,
    /// saved when the caller saves it.
    pub fn apply(&self, layout: &mut Layout) {
        layout.edit_options(|o| mapper::apply_global_options(o, self.global_options.as_ref()));
        if let Some(excluded) = &self.excluded {
            layout.edit_options(|o| o.excluded_list = excluded.clone());
        }
        mapper::apply_layout(layout, self.layout.as_ref(), &self.models);
        layout.parse_physical_monitors();
    }
}
