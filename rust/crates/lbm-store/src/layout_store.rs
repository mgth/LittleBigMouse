//! The storage backend contract — port of `Persistence/ILayoutStore.cs`.

use std::io;

use indexmap::IndexMap;

use crate::layout_dtos::{GlobalOptionsDto, LayoutDto, ModelDto};

/// C# `ILayoutStore`: a dumb storage backend. It moves DTOs in and out of the OS store
/// (JSON files, the registry on Windows) and knows nothing about the layout model; all
/// mapping and semantics live in the engine, once.
///
/// Where the C# methods return nothing and a backend reports trouble by throwing (or,
/// for the JSON store, not at all), these return an [`io::Result`]: the caller decides
/// whether a failed save matters. The C# engine never learns about a failed write —
/// which is how #589 went unseen — and treats a failed read as a first run.
pub trait LayoutStore {
    /// C# `ILayoutStore.Read`: one read of everything relevant to a layout — the global
    /// options, the layout document and the stored models for the given PnP codes.
    /// Absent data comes back as `None` (or a missing map entry). Reads are PURE: they
    /// must never create keys nor seed values in the store.
    fn read(&self, layout_id: &str, pnp_codes: &[&str]) -> io::Result<LayoutStoreData>;

    /// C# `ILayoutStore.WriteGlobalOptions`: write the app-level options. "Null fields
    /// are skipped, never deleted", says the C# contract — true of the registry store,
    /// not of the JSON one, which replaces the whole document (see
    /// [`JsonLayoutStore`](crate::JsonLayoutStore)).
    fn write_global_options(&self, options: &GlobalOptionsDto) -> io::Result<()>;

    /// C# `ILayoutStore.WriteLayout`: write one layout document (options + monitors).
    /// Null fields are skipped, with the same caveat as
    /// [`write_global_options`](Self::write_global_options).
    fn write_layout(&self, layout_id: &str, layout: &LayoutDto) -> io::Result<()>;

    /// C# `ILayoutStore.WriteModels`: upsert the given monitor models; stored models not
    /// listed are left untouched.
    fn write_models(&self, models: &IndexMap<String, ModelDto>) -> io::Result<()>;
}

/// C# `LayoutStoreData`: what [`LayoutStore::read`] returns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutStoreData {
    /// C# `LayoutStoreData.GlobalOptions`.
    pub global_options: Option<GlobalOptionsDto>,
    /// C# `LayoutStoreData.Layout`.
    pub layout: Option<LayoutDto>,
    /// C# `LayoutStoreData.Models`, keyed by PnP code.
    pub models: IndexMap<String, ModelDto>,
}
