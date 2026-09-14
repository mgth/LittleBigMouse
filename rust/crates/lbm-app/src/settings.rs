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

/// The `SaveLayout` request for this layout — the Save button.
///
/// **`Excluded` is stripped, and that is the whole point of this function.**
/// `LayoutDocument::of` fills it from `layout.options.excluded_list`, which is the right
/// thing for the agent (it has read the list) and a destructive thing for this window
/// (it has not). Sending the empty list it holds would erase the user's exclusions on the
/// first Save. The field is optional and the document's own doc says what absent means:
/// "the excluded processes; absent: left as they are".
///
/// The same trap as `SaveOptions`, one level deeper and easier to miss, because here the
/// field is filled in for you.
pub fn save_layout(layout: &lbm_layout::model::Layout) -> (&'static str, Value) {
    let mut document = lbm_store::LayoutDocument::of(layout);
    document.excluded = None;
    (
        "SaveLayout",
        json!({ "LayoutId": layout.id, "Document": document }),
    )
}

/// How often a live preview is sent at most: `LiveLayoutUpdater.Interval`.
///
/// "Short enough that adjusting a border feels immediate, long enough that dragging a
/// monitor never turns into a burst of layout swaps." A rate limit, not a poll.
pub const PREVIEW_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// The `Preview` request: the layout the user is editing, run but not written.
///
/// The same document as [`save_layout`], excluded list stripped for the same reason, and
/// the same one the agent would have saved — a preview differs from a save in what the
/// agent *does* with it, not in what it is.
pub fn preview(layout: &lbm_layout::model::Layout) -> (&'static str, Value) {
    let (_, extra) = save_layout(layout);
    ("Preview", extra)
}

/// The `EndPreview` request: the agent goes back to the layout it had.
pub fn end_preview() -> (&'static str, Value) {
    ("EndPreview", json!({}))
}
