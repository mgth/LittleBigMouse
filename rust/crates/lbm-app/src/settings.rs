//! What the window asks the agent to keep: the app-wide options.
//!
//! Here rather than in `main.rs` so that a test can check the **same** request the window
//! sends. A request built twice — once for the window, once for the test — is a test that
//! agrees with itself, which is the failure mode this whole crate was written to avoid.

use lbm_layout::model::LayoutOptions;
use lbm_store::layout_dto_mapper::to_global_options_dto;
use serde_json::{json, Value};

/// The `SaveOptions` request for these options, as the agent's API spells it.
///
/// **The whole set goes, not the field that changed**: `SaveOptions` takes a whole
/// `GlobalOptionsDto` and the agent applies it over its own. Worth knowing what that
/// means — the window read `options.json` when it started, so if something else has
/// changed a setting since (the tray, another frontend), saving here writes this window's
/// whole picture back over it. The C# `SaveLive` has the same shape; anything narrower
/// would be a change to the wire protocol.
///
/// **`Excluded` is deliberately absent.** The excluded list lives in its own file and
/// this window never reads it, so sending the empty list it holds would erase the user's
/// exclusions. The field is optional and the agent leaves the list alone without it
/// (`world.rs`, `if let Some(excluded)`).
///
/// `excluded_defaults_version` is `None` for the same reason: it belongs to the excluded
/// list, and the agent writes its own from its `ExcludedListPersistence` rather than from
/// the request.
pub fn save_options(options: &LayoutOptions) -> (&'static str, Value) {
    (
        "SaveOptions",
        json!({
            "Options": to_global_options_dto(options, None),
            // Not one of the stored options: it *is* the session autostart, which the
            // agent aligns when it is present.
            "LoadAtStartup": options.load_at_startup,
        }),
    )
}
