//! The one-time import of the Windows registry into the JSON store (decision D2 of the
//! v6 plan: one store format everywhere, no more 255-character cap, files readable for
//! support).
//!
//! Everything the v5 registry holds is copied: the global options, every layout under
//! `Layouts`, every model under `monitors`. The registry is only read, and stays as it
//! was, so going back to 5.x loses nothing. Values are read exactly as the C#
//! `RegistryLayoutStore` reads them, legacy locations included
//! ([`registry_layout_store`]); what the documents then mean is the engine's business,
//! as for any store (the whole-edge resistances, for instance, are migrated at load).
//!
//! # Store names
//!
//! A layout is found in the JSON store by its id, so the import needs each layout's id,
//! and the registry only keeps a name derived from it: the id itself when it fits in
//! 255 characters, a head and a SHA-256 otherwise (#589). The JSON store caps names
//! shorter (room for `.json`), so a name that is a digest cannot just be carried over.
//! The id is rebuilt from what the layout stores, the way `ComputeId` built it — every
//! monitor id, suffixed `_<n>` when its active source is turned `n` quarter turns,
//! sorted by the invariant culture, joined with `+` — and kept only when it gives back
//! the registry name. A digest name whose id cannot be rebuilt is not imported and is
//! reported; any other name is the id.
//!
//! # Global options
//!
//! A few global options once lived in the layout key, and C# reads them from the key of
//! the layout being loaded when the root has none. The import does the same with the
//! layout current at the first launch; after the first C# save the root value shadows
//! the others anyway.

use std::io;

use indexmap::IndexMap;
use lbm_layout::collation::invariant_compare;

use crate::layout_dtos::{LayoutDto, ModelDto};
use crate::layout_store::LayoutStore;
use crate::layout_store_key::{key_for, MAX_LENGTH};
use crate::registry_layout_store::{
    layout_key_path, read_global_options, read_layout, read_model, RegistryKey,
};

/// What an import copied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegistryImport {
    /// Ids of the layouts written to the JSON store, in registry order.
    pub layouts: Vec<String>,
    /// PnP codes of the models written.
    pub models: Vec<String>,
    /// Registry layout names left out, each with the reason.
    pub skipped: Vec<(String, String)>,
}

/// The id a stored layout document was saved under, as `ComputeId` computed it: the
/// monitors' ids, suffixed `_<n>` when the active source is turned, sorted by the
/// invariant culture (stable), joined with `+`.
pub fn stored_layout_id(layout: &LayoutDto) -> String {
    let mut ids: Vec<String> = layout
        .monitors
        .iter()
        .map(|(id, monitor)| {
            let orientation = monitor
                .active_source
                .as_ref()
                .and_then(|active| monitor.sources.as_ref()?.get(active)?.orientation)
                .unwrap_or(0);
            match orientation {
                0 => id.clone(),
                o => format!("{id}_{o}"),
            }
        })
        .collect();
    ids.sort_by(|a, b| invariant_compare(a, b));
    ids.join("+")
}

/// The id of the layout stored under the registry name `name`, or why there is none.
fn layout_id(name: &str, layout: &LayoutDto) -> Result<String, String> {
    let rebuilt = stored_layout_id(layout);
    if key_for(&rebuilt) == name {
        return Ok(rebuilt);
    }
    // `LayoutStoreKey`'s digest form: a head, '~', 64 hex digits, 255 in all.
    let digest = name.encode_utf16().count() == MAX_LENGTH
        && name
            .rsplit_once('~')
            .is_some_and(|(_, hex)| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
    if digest {
        return Err(format!(
            "stored under a digest of its id, and the id rebuilt from its monitors ('{rebuilt}') does not match it"
        ));
    }
    Ok(name.to_owned())
}

/// Copies the registry tree under `root` (`HKCU\SOFTWARE\Mgth\LittleBigMouse`) into
/// `target`. `current_layout_id` is the layout about to be loaded, whose key supplies
/// the legacy global options. Every document is attempted; the first write failure is
/// returned after the others.
pub fn import_registry<K: RegistryKey, S: LayoutStore + ?Sized>(
    root: &K,
    current_layout_id: &str,
    target: &S,
) -> io::Result<RegistryImport> {
    let mut report = RegistryImport::default();
    let mut result = Ok(());
    let mut record = |r: io::Result<()>| {
        if result.is_ok() {
            result = r;
        }
    };

    if let Some(layouts) = root.open("Layouts") {
        for name in layouts.subkey_names() {
            let Some(key) = layouts.open(&name) else {
                continue;
            };
            let layout = read_layout(&key);
            match layout_id(&name, &layout) {
                Ok(id) => {
                    record(target.write_layout(&id, &layout));
                    report.layouts.push(id);
                }
                Err(reason) => report.skipped.push((name, reason)),
            }
        }
    }

    if let Some(monitors) = root.open("monitors") {
        let mut models: IndexMap<String, ModelDto> = IndexMap::new();
        for pnp_code in monitors.subkey_names() {
            if let Some(key) = monitors.open(&pnp_code) {
                models.insert(pnp_code.clone(), read_model(&key));
                report.models.push(pnp_code);
            }
        }
        if !models.is_empty() {
            record(target.write_models(&models));
        }
    }

    let current = root.open(&layout_key_path(current_layout_id));
    record(target.write_global_options(&read_global_options(root, current.as_ref())));

    result.map(|()| report)
}
