//! The persistence engine — port of `Persistence/LayoutPersistence.cs`.
//!
//! The façade: what to load, in which order, and what counts as saved. The three
//! responsibilities it orchestrates live next door:
//!
//! - [`layout_dto_mapper`](crate::layout_dto_mapper): the whole model↔DTO mapping,
//!   both directions;
//! - [`layout_migrations`](crate::layout_migrations): how values written by older
//!   versions are read;
//! - [`ExcludedListPersistence`]: the excluded-processes file, its defaults and their
//!   one-time top-up.
//!
//! A platform provides a dumb [`LayoutStore`] (JSON files, the registry) plus the
//! [`PersistencePlatform`] hooks, and gets the complete behavior — no per-OS mapping
//! code, so the platforms cannot drift apart.
//!
//! # Failures
//!
//! The C# stores swallow every write failure, so a C# save always "succeeds" and marks
//! the layout saved even when nothing reached the disk (#589 went unseen that way).
//! Here the writes happen in the same order and all of them are attempted, but the
//! first failure is returned and the layout is then left unsaved. A failed store READ
//! is contained exactly as in C#: the layout loads as on a first run.

use std::fs;
use std::io;
use std::path::PathBuf;

use lbm_layout::model::{Layout, LayoutOptions};

use crate::excluded_list_persistence::ExcludedListPersistence;
use crate::layout_dto_mapper::{self as mapper};
use crate::layout_dtos::{LayoutDto, LayoutOptionsDto};
use crate::layout_store::{LayoutStore, LayoutStoreData};
use crate::lbm_paths;

/// The platform hooks of C# `LayoutPersistence`: what the engine asks the machine.
pub trait PersistencePlatform {
    /// C# `IsElevated`: whether the current process runs elevated (administrator /
    /// root). C#'s default is `Environment.IsPrivilegedProcess`.
    fn is_elevated(&self) -> bool;

    /// C# `IsAutostartScheduled`: whether the app is registered to start with the user
    /// session. `false` where not implemented (Linux).
    fn is_autostart_scheduled(&self, _layout: &Layout) -> bool {
        false
    }

    /// C# `SetAutostart`: align the session autostart with the options. A no-op where
    /// not implemented (Linux).
    fn set_autostart(&self, _layout: &Layout, _enabled: bool, _elevated: bool) {}
}

/// C# `LayoutPersistence.ExcludedListFile`: `Excluded.txt` in the data directory, which
/// is created on the way. It is a plain-text FILE the daemon reads, so it stays out of
/// the store.
pub fn default_excluded_list_file() -> PathBuf {
    let dir = lbm_paths::data_dir();
    // C# throws out of the load when the directory cannot be created; the file
    // operations that follow report it here.
    let _ = fs::create_dir_all(&dir);

    let file = dir.join("Excluded.txt");
    // Self-heal: a buggy earlier version created "Excluded.txt" as a *directory*.
    if file.is_dir() {
        let _ = fs::remove_dir_all(&file);
    }
    file
}

type ExcludedListFile = Box<dyn Fn() -> PathBuf + Send + Sync>;

/// C# `LayoutPersistence`: loads a layout from a store and saves it back.
pub struct LayoutPersistence<S, P> {
    store: S,
    platform: P,
    excluded: ExcludedListPersistence<ExcludedListFile>,
    is_loading: bool,
}

impl<S: LayoutStore, P: PersistencePlatform> LayoutPersistence<S, P> {
    /// The engine over `store`, with the excluded-processes file at its usual place
    /// ([`default_excluded_list_file`]).
    pub fn new(store: S, platform: P) -> Self {
        Self::with_excluded_list_file(store, platform, default_excluded_list_file)
    }

    /// The engine with the excluded-processes file elsewhere (C#: the overridable
    /// `ExcludedListFile`), asked again at every load and write.
    pub fn with_excluded_list_file(
        store: S,
        platform: P,
        excluded_list_file: impl Fn() -> PathBuf + Send + Sync + 'static,
    ) -> Self {
        LayoutPersistence {
            store,
            platform,
            excluded: ExcludedListPersistence::new(Box::new(excluded_list_file)),
            is_loading: false,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// C# `IsLoading`: true while [`load`](Self::load) runs.
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    //==================//
    // Load             //
    //==================//

    /// C# `Load`: apply what the store holds for `layout` — global options, the
    /// excluded list, the layout's options, then per monitor its model and its own
    /// state — and mark everything saved.
    ///
    /// A virtual layout is refused: its state comes from its export, and only from it.
    /// Fails where C# throws out of the load: an `Excluded.txt` that exists but cannot
    /// be read, the global options then applied and nothing else.
    pub fn load(&mut self, layout: &mut Layout) -> io::Result<()> {
        // Reading the local store for a virtual layout would apply options persisted for
        // whatever LOCAL layout shares the client's id — foreign data over foreign data.
        if refuse_virtual(layout, "Load") {
            return Ok(());
        }

        let was_loading = std::mem::replace(&mut self.is_loading, true);
        let result = self.load_store(layout);
        self.is_loading = was_loading;
        result
    }

    fn load_store(&mut self, layout: &mut Layout) -> io::Result<()> {
        let mut data = self.read_store(layout);

        let autostart = self.platform.is_autostart_scheduled(layout);
        let elevated = self.platform.is_elevated();
        layout.edit_options(|o| {
            o.load_at_startup = autostart;
            o.elevated = elevated;
        });

        // Global options first, then the per-layout ones: the layout overrides the app
        // level (Priority/PriorityUnhooked exist on both sides).
        layout.edit_options(|o| mapper::apply_global_options(o, data.global_options.as_ref()));

        let mut excluded = Vec::new();
        let read = self
            .excluded
            .load(&self.store, &mut excluded, data.global_options.as_mut());
        layout.edit_options(|o| o.excluded_list = excluded);
        read?;

        let layout_options = data.layout.as_ref().and_then(|l| l.options.as_ref());
        layout.edit_options(|o| mapper::apply_layout_options(o, layout_options));

        let monitors: Vec<(String, String)> = layout
            .monitors()
            .iter()
            .map(|m| (m.id.clone(), m.model.clone()))
            .collect();
        for (id, model) in &monitors {
            // Model before monitor: the monitor mapping reads the physical size the
            // model just restored (edge lengths, whole-edge resistance migration).
            if let Some(dto) = data.models.get(model) {
                mapper::apply_model(layout, model, dto);
            }
            if let Some(dto) = data.layout.as_ref().and_then(|l| l.monitors.get(id)) {
                mapper::apply_monitor(layout, id, dto);
            }
        }

        // Everything saved, even on a first run with no stored data, so that the next
        // edit is a change from a saved state (C#: MarkSaved on every monitor, then the
        // options and the layout).
        layout.mark_saved();
        layout.parse_physical_monitors();
        Ok(())
    }

    /// C# `ReadStore`: the store read, with its failure contained — a store that cannot
    /// be read must not keep the app from starting (#589). The layout then loads as on
    /// a first run, placed from the system configuration, nothing saved.
    fn read_store(&self, layout: &Layout) -> LayoutStoreData {
        let mut pnp_codes: Vec<&str> = Vec::new();
        for monitor in layout.monitors() {
            if !pnp_codes.contains(&monitor.model.as_str()) {
                pnp_codes.push(&monitor.model);
            }
        }
        self.store
            .read(&layout.id, &pnp_codes)
            .unwrap_or_else(|error| {
                eprintln!(
                    "[LittleBigMouse] Layout store read failed for '{}': {error}",
                    layout.id
                );
                LayoutStoreData::default()
            })
    }

    //==================//
    // Save             //
    //==================//

    /// C# `Save`: the global options and the excluded list, the layout document, its
    /// models (merged into the stored ones), then everything marked saved. Returns
    /// `false` for a virtual layout, which is never saved locally.
    pub fn save(&self, layout: &mut Layout) -> io::Result<bool> {
        if refuse_virtual(layout, "Save") {
            return Ok(false);
        }

        self.platform.set_autostart(
            layout,
            layout.options.load_at_startup,
            layout.options.start_elevated,
        );

        let global = self.save_global_options(&layout.options);
        let document = self
            .store
            .write_layout(&layout.id, &mapper::to_layout_dto(layout));
        let models = self.store.write_models(&mapper::to_model_dtos(layout));
        global.and(document).and(models)?;

        layout.mark_saved();
        Ok(true)
    }

    /// C# `SaveEnabled`: store the layout's `Enabled` option alone, by read-modify-write
    /// of its document — everything else stored for the layout is kept (members this
    /// version does not know included), so the engine can be toggled without a full
    /// save. Runs on every stop; returns `false` for a virtual layout, which would
    /// otherwise CREATE a store entry named after the client's monitor combination.
    pub fn save_enabled(&self, layout: &Layout) -> io::Result<bool> {
        if refuse_virtual(layout, "SaveEnabled") {
            return Ok(false);
        }

        let mut dto: LayoutDto = self.store.read(&layout.id, &[])?.layout.unwrap_or_default();
        let options = dto.options.get_or_insert_with(LayoutOptionsDto::default);
        options.enabled = Some(layout.options.enabled);

        // Everything else read is written back as-is, EXCEPT the two app-level options a
        // layout may still carry: re-emitting them here would put back what a full save
        // just migrated away (see `to_layout_options_dto`).
        options.priority = None;
        options.priority_unhooked = None;

        self.store.write_layout(&layout.id, &dto)?;

        self.platform.set_autostart(
            layout,
            layout.options.load_at_startup,
            layout.options.start_elevated,
        );
        Ok(true)
    }

    /// C# `SaveLive`: the app-level options alone, and the excluded list.
    pub fn save_live(&self, options: &LayoutOptions) -> io::Result<()> {
        self.save_global_options(options)
    }

    fn save_global_options(&self, o: &LayoutOptions) -> io::Result<()> {
        let options = self
            .store
            .write_global_options(&mapper::to_global_options_dto(
                o,
                self.excluded.applied_defaults_version(),
            ));
        let excluded = self.excluded.write(&o.excluded_list);
        options.and(excluded)
    }
}

/// C# `RefuseVirtual`: the single choke point keeping virtual (foreign) layouts out of
/// the local store and the autostart scheduling. Guarding here rather than at the call
/// sites means no path — current or future — can leak a client's configuration into
/// this machine's state.
fn refuse_virtual(layout: &Layout, operation: &str) -> bool {
    if !layout.is_virtual() {
        return false;
    }
    eprintln!(
        "[LittleBigMouse] {operation} refused: '{}' is a virtual layout ({:?})",
        layout.id, layout.source_kind
    );
    true
}
