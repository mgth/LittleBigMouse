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
/// **`Excluded` goes only when the caller has read it.** The list lives in its own file,
/// and a window that has not read it holds an empty one; sending that would erase the
/// user's exclusions. The field is optional and the agent leaves the list alone without
/// it (`world.rs`, `if let Some(excluded)`), so `None` says "I do not know" and means it.
///
/// `excluded_defaults_version` is `None` for the same reason: it belongs to the excluded
/// list, and the agent writes its own from its `ExcludedListPersistence` rather than from
/// the request.
pub fn save_options(options: &LayoutOptions, excluded: Option<&[String]>) -> (&'static str, Value) {
    let mut request = json!({
        "Options": to_global_options_dto(options, None),
        // Not one of the stored options: it *is* the session autostart, which the agent
        // aligns when it is present.
        "LoadAtStartup": options.load_at_startup,
    });
    // Only when the caller has actually read the list. `None` is not "no exclusions": it
    // is "I do not know", and the agent leaves the list alone for it.
    if let Some(excluded) = excluded {
        request["Excluded"] = json!(excluded);
    }
    ("SaveOptions", request)
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
    (
        "SaveLayout",
        json!({ "LayoutId": layout.id, "Document": document(layout) }),
    )
}

/// The document this window sends, for a save or for a preview.
///
/// One function because it must be one document: [`crate::saved::Reference`] decides
/// whether there is anything to save by comparing against it, and a reference built from
/// a *slightly* different document would say "unsaved" for a field Save does not send —
/// a Save button that never goes out.
pub fn document(layout: &lbm_layout::model::Layout) -> lbm_store::LayoutDocument {
    let mut document = lbm_store::LayoutDocument::of(layout);
    document.excluded = None;
    document
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

/// The excluded list at `file`, through the agent's own parser — or `None` when there is
/// no file.
///
/// **The file is checked first, and that is the whole subtlety.**
/// `ExcludedListPersistence::load` seeds the defaults and **writes** the file when it is
/// missing (`excluded_list_persistence.rs`: "First run: seed the defaults and write the
/// file the daemon reads"). Correct for the agent, which is the writer; a promise broken
/// for a window that creates nothing. So the write path is never entered, and a missing
/// file comes back as `None` — the agent will make it.
///
/// `None` is not an empty list anywhere downstream: it is "not read", which is why
/// [`save_options`] takes an `Option` and leaves the field out for it.
///
/// The parser is reused rather than reimplemented: what comes out of it is what the
/// daemon is handed, so the list the user edits and the list that filters are one list.
pub fn read_excluded(file: std::path::PathBuf) -> Option<Vec<String>> {
    if !file.is_file() {
        return None;
    }
    let store = lbm_store::JsonLayoutStore::new(lbm_store::lbm_paths::config_dir());
    let at = file.clone();
    let mut persistence = lbm_store::ExcludedListPersistence::new(move || at.clone());
    let mut list = Vec::new();
    match persistence.load(&store, &mut list, None) {
        Ok(()) => Some(list),
        Err(error) => {
            eprintln!("[lbm-app] the excluded list could not be read: {error}");
            None
        }
    }
}

/// The `ApplyTopology` request: move the real screens to match this layout.
///
/// The same document as a save, for the same reason and with the same excluded list
/// stripped out — the agent saves it before applying, because the system change triggers
/// a rebuild that loads the *saved* layout and the applied arrangement survives only if
/// it was written down first.
///
/// `adjust_scale` asks for the per-output scales that give every monitor the primary's
/// logical pitch. Linux only, and off unless the user asked: rescaling every monitor is
/// not what someone who wanted their screens rearranged necessarily meant.
pub fn apply_topology(
    layout: &lbm_layout::model::Layout,
    adjust_scale: bool,
) -> (&'static str, Value) {
    let (_, mut extra) = save_layout(layout);
    extra["AdjustScale"] = json!(adjust_scale);
    ("ApplyTopology", extra)
}

/// The same request, asked as a **dry run**: the agent answers with the command lines it
/// would have run and changes nothing — not the screens, and not the saved layout.
///
/// A separate function rather than a flag on [`apply_topology`], so that the call site
/// that moves the user's screens and the one that only asks cannot be confused for one
/// another by a boolean nobody reads.
pub fn dry_run_topology(
    layout: &lbm_layout::model::Layout,
    adjust_scale: bool,
) -> (&'static str, Value) {
    let (method, mut extra) = apply_topology(layout, adjust_scale);
    extra["DryRun"] = json!(true);
    (method, extra)
}
