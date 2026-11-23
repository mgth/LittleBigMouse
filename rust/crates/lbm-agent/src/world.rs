//! What the agent acts on: the displays, the stored profiles and the current layout.
//!
//! [`AgentWorld`] is the [`World`] the reconciler reads, plus what the runtime needs to
//! carry out its effects: the zones to hand the hook, and the two kinds of save.
//! [`SystemWorld`] is the real one — the displays found by `lbm-display` ([`Discovery`]),
//! the layout built from them with the profile the persistence engine loads (C#:
//! `LinuxLayoutFactory.Create`, `WindowsLayoutBuilder`).

use std::io;

use lbm_layout::model::{Layout, LayoutOptions};
use lbm_layout::zoning::compute_zones;
use lbm_store::layout_dto_mapper::apply_global_options;
use lbm_store::wallpaper_settings::{self, LayoutWallpaperSettings};
use lbm_store::{
    GlobalOptionsDto, LayoutDocument, LayoutPersistence, LayoutStore, PersistencePlatform,
};

use crate::autostart::XdgAutostart;
use crate::discovery::Discovery;
use crate::gap_guard::{run_kscreen_doctor, GapGuard};
use crate::peers::RemoteScreens;
use crate::reconcile::{LayoutState, World};

/// The world the runtime drives.
pub trait AgentWorld: World {
    fn vcp_target(&self, _monitor: &str) -> Result<crate::vcp::Target, String> {
        Err("VCP controls are not available on this display backend.".into())
    }
    /// Windows placements must not run against a live hook or preview.
    fn topology_requires_stopped_engine(&self) -> bool {
        false
    }
    fn display_outputs(&mut self) -> Result<lbm_ipc::displays::Outputs, String> {
        Err("monitor actions are not available on this display backend".into())
    }
    fn change_output(
        &mut self,
        _connector: &str,
        _change: lbm_ipc::displays::Change,
        _signature: &str,
    ) -> Result<lbm_ipc::displays::Outputs, String> {
        Err("monitor actions are not available on this display backend".into())
    }
    /// The zones to hand the hook — the previewed layout's while there is one, else the
    /// current layout's — serialized, and whether the layout is foreign (simulated:
    /// loaded, never run). `None` before the first layout.
    fn zones(&self) -> Option<(String, bool)>;

    /// The desktop those zones sit on, in pixels — the union of the attached screens,
    /// including any the layout excludes. `None` before the first layout.
    ///
    /// The hook cannot work this out: it is given the zones and nothing else, and the
    /// zones stop at the layout's edge. Only the agent enumerates the outputs.
    fn desktop(&self) -> Option<lbm_ipc::protocol::Desktop>;

    /// This machine's screens as the multi-PC mode reports them to a master. Asked
    /// only while the mode is on, after a rebuild: none by default.
    fn peer_displays(&mut self) -> Vec<lbm_ipc::peer::PeerDisplay> {
        Vec::new()
    }

    /// The screens of the machines this one is the master of, for the next rebuild to
    /// merge (multi-PC). Whether they differ from those held: a difference is a display
    /// change. A world that merges nothing ignores them.
    fn set_remote_screens(&mut self, _remotes: Vec<RemoteScreens>) -> bool {
        false
    }

    /// Shows `source` on the shared panel `monitor` of the current layout, and saves.
    /// Refused as [`edit`](Self::edit) is for another layout than the current one.
    /// What the choice asks of the monitor's input, when the source's input value is
    /// known (`crate::input_switch`): the caller carries it out, away from its loop.
    fn set_active_source(
        &mut self,
        _layout_id: &str,
        _monitor: &str,
        _source: &str,
    ) -> Result<Option<crate::input_switch::InputSwitch>, String> {
        Err("this agent shows no other machine's screens".to_owned())
    }

    /// The panels whose input to read after a rebuild (`input_switch::reads`); none by
    /// default.
    fn input_reads(&self, _publishing: bool) -> Vec<crate::input_switch::InputRead> {
        Vec::new()
    }

    /// What those reads found; saved when it changed something.
    fn learn_inputs(&mut self, _readings: &[(crate::input_switch::InputRead, u16)]) {}

    /// This machine's way to its monitor `display` (the id it gives it to a master),
    /// for a master's `SelectInput`: refused for a display it does not have.
    fn select_input_target(&self, _display: &str) -> Result<crate::vcp::Target, String> {
        Err("this agent switches no monitor".to_owned())
    }

    /// The layout's monitor that this machine reports to a master as `display`.
    fn display_monitor(&self, _display: &str) -> Option<String> {
        None
    }

    /// Nobody could switch `monitor` to `source`: a source that is not on its machine's
    /// desktop, chosen for the switch to bring it there, is given up for one that is.
    /// Whether the panel now shows another source.
    fn switch_failed(&mut self, _monitor: &str, _source: &str) -> bool {
        false
    }

    /// Persists the current layout's Enabled alone.
    fn save_enabled(&mut self) -> io::Result<()>;

    /// Persists the whole current layout.
    fn save_layout(&mut self) -> io::Result<()>;

    /// The current layout's id, for the frontends.
    fn layout_id(&self) -> Option<String> {
        None
    }

    /// Applies a frontend's edit to the current layout (not saved). Refused for another
    /// layout than the current one — the displays changed under the editor — and for a
    /// foreign one.
    fn edit(&mut self, _layout_id: &str, _document: &LayoutDocument) -> Result<(), String> {
        Err("this agent takes no edits".to_owned())
    }

    /// Compile a copy of the displayed edit, without affecting the stored/previewed layout.
    fn probe_zones(
        &mut self,
        _layout_id: &str,
        _document: &LayoutDocument,
    ) -> Result<String, String> {
        Err("this agent cannot probe an edited layout".to_owned())
    }

    /// The layout to preview: the current one with the edit applied, the current one
    /// left as it is. Refused as [`edit`](Self::edit) is.
    /// Apply the layout to the system display configuration. The screens really move —
    /// unless `how` is a dry run, which writes **nothing at all**, the layout included.
    ///
    /// Gives back every command line, in order: what was run, or what would be.
    fn apply_topology(
        &mut self,
        _layout_id: &str,
        _document: &LayoutDocument,
        _adjust_scale: bool,
        _how: lbm_display::linux::topology::How,
    ) -> Result<Vec<String>, String> {
        Err("this agent cannot change the display topology".to_owned())
    }

    fn set_preview(&mut self, _layout_id: &str, _document: &LayoutDocument) -> Result<(), String> {
        Err("this agent takes no previews".to_owned())
    }

    /// What the desktop should show for the layout now held: one target per screen,
    /// with any span slices cut (v6: the agent applies the wallpaper, so that a display
    /// change puts it back whether or not a window is open).
    fn wallpaper(&self) -> Vec<crate::desktop::ScreenWallpaper> {
        Vec::new()
    }
    /// Capture work so routine rebuilds also render outside the resident event loop.
    fn saved_wallpaper_job(&self) -> Option<crate::wallpaper_job::Job> {
        None
    }
    fn wallpaper_settings(&mut self, _id: &str) -> Result<LayoutWallpaperSettings, String> {
        Err("this agent keeps no wallpaper".into())
    }
    fn wallpaper_job(
        &mut self,
        _id: &str,
        _settings: LayoutWallpaperSettings,
    ) -> Result<crate::wallpaper_job::Job, String> {
        Err("this agent keeps no wallpaper".into())
    }

    /// Records a frontend's wallpaper settings for `layout_id` and answers what the
    /// desktop should then show. Refused for another layout than the current one, as
    /// [`edit`](Self::edit) is: the settings are keyed by layout.
    fn save_wallpaper(
        &mut self,
        _layout_id: &str,
        _settings: LayoutWallpaperSettings,
    ) -> Result<Vec<crate::desktop::ScreenWallpaper>, String> {
        Err("this agent keeps no wallpaper".to_owned())
    }

    /// Records the app-level options and the excluded list (C#: `SaveLive`), and aligns
    /// the session autostart when `load_at_startup` says so (C#: `UpdateSchedule`).
    fn save_options(
        &mut self,
        _options: Option<&GlobalOptionsDto>,
        _excluded: Option<&[String]>,
        _load_at_startup: Option<bool>,
    ) -> Result<(), String> {
        Err("this agent keeps no options".to_owned())
    }

    /// The app-level options a frontend shows: whether the session starts the agent, and
    /// whether the user asked for no tray icon.
    fn app_options(&self) -> Option<(bool, bool)> {
        None
    }

    /// The rescue shortcut the options name.
    fn rescue_shortcut(&self) -> Option<String> {
        None
    }

    /// Whether the user asked for the hook to belong to this agent — to let go of the
    /// mice and leave when the connection ends. `None` before there are options to ask.
    fn bound_to_agent(&self) -> Option<bool> {
        None
    }

    /// The processes the engine stands aside for, as the user's list spells them.
    /// `None` before there are options to ask — which is not the same as an empty list,
    /// and must not be sent as one.
    fn excluded(&self) -> Option<Vec<String>> {
        None
    }

    /// The engine's topology prologue (the KWin gaps, `gap_guard`): whether it moved
    /// outputs — then the layout at hand describes a desktop that is going away, and the
    /// Start waits for the display change that follows.
    fn prepare_for_engine(&mut self) -> bool {
        false
    }

    /// The epilogue, once the engine stopped: whether it moved outputs back.
    fn restore_after_engine(&mut self) -> bool {
        false
    }

    /// At startup: put back what a previous run left gapped; whether it moved outputs.
    fn recover_stale(&mut self) -> bool {
        false
    }
}

/// The real world: discovery, profiles, the layout.
pub struct SystemWorld<S, P> {
    discovery: Discovery,
    persistence: LayoutPersistence<S, P>,
    /// Run once, before the first profile is loaded (Windows: the registry import).
    first_load: Option<fn(&str, &S)>,
    layout: Option<Layout>,
    /// A frontend's edit of `layout`, being previewed.
    preview: Option<Layout>,
    gaps: Option<GapGuard>,
    /// Where the wallpaper settings are read and written, and where the span slices go.
    /// `None` leaves the desktop alone — a scratch agent has no business repainting the
    /// user's background.
    wallpaper: Option<WallpaperPaths>,
    /// The machines this one is the master of, with their screens: merged into every
    /// rebuild (multi-PC). Empty unless the mode is on and a remote is linked.
    remotes: Vec<RemoteScreens>,
    /// Panels the user switched to a source that was not on its machine's desktop yet,
    /// by monitor: the input switch is what brings it there, and until it has, the
    /// choice must survive the rebuilds the switch itself sets off — the rule "attached
    /// to one machine only, that one is shown" would otherwise take it back before the
    /// other machine has reported the panel.
    switching: Vec<(String, String)>,
}

/// The two places the wallpaper lives.
#[derive(Clone, Debug)]
struct WallpaperPaths {
    /// `wallpaper.json`, the C# plugin's own file.
    settings: std::path::PathBuf,
    /// Where the cut slices go (`<data>/wallpapers`).
    slices: std::path::PathBuf,
}

impl<S: LayoutStore, P: PersistencePlatform> SystemWorld<S, P> {
    /// The displays through `discovery`, the profiles through `persistence`.
    pub fn new(discovery: Discovery, persistence: LayoutPersistence<S, P>) -> Self {
        SystemWorld {
            discovery,
            persistence,
            first_load: None,
            layout: None,
            preview: None,
            gaps: None,
            wallpaper: None,
            remotes: Vec::new(),
            switching: Vec::new(),
        }
    }

    /// Saves a change the agent made on its own (a learnt input, a switch given up). At
    /// once, as a chosen source is: every rebuild starts again from the store, and an
    /// input value is needed precisely when its source is no longer shown to be read.
    fn save_own_change(&mut self) {
        let Some(layout) = self.layout.as_mut() else {
            return;
        };
        if let Err(error) = self.persistence.save(layout) {
            eprintln!("[lbm-agent] could not save the layout: {error}");
        }
    }

    /// After a rebuild: the choices waiting for their switch are kept over the rule
    /// that would undo them, and given up once their source is on its machine's
    /// desktop (the rule keeps it then) or gone from the layout.
    fn keep_switching(&mut self) {
        let Some(layout) = self.layout.as_mut() else {
            self.switching.clear();
            return;
        };
        self.switching.retain(|(monitor, source)| {
            let waiting = layout
                .monitor(monitor)
                .is_some_and(|m| m.sources.contains(source))
                && layout
                    .source(source)
                    .is_some_and(|s| !s.source.attached_to_desktop);
            if waiting {
                layout.set_active_source(monitor, Some(source.clone()));
            }
            waiting
        });
    }

    /// Take charge of the desktop background: the settings file, and where the slices go.
    /// Without this the agent reads and writes neither.
    pub fn with_wallpaper(
        mut self,
        settings: std::path::PathBuf,
        slices: std::path::PathBuf,
    ) -> Self {
        self.wallpaper = Some(WallpaperPaths { settings, slices });
        self
    }

    /// Runs `before` once, with the id of the layout about to be loaded and the store,
    /// before the first profile is loaded (D2: the one-time registry import).
    pub fn before_first_load(mut self, before: fn(&str, &S)) -> Self {
        self.first_load = Some(before);
        self
    }

    /// Opens the KWin gaps around the engine (a real session only: they move the
    /// user's outputs).
    pub fn with_gap_guard(mut self, gaps: GapGuard) -> Self {
        self.gaps = Some(gaps);
        self
    }

    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    /// The current layout, if `layout_id` names it and it takes edits.
    fn editable(&mut self, layout_id: &str) -> Result<&mut Layout, String> {
        let layout = self.layout.as_mut().ok_or("no layout yet")?;
        if layout.id != layout_id {
            return Err(format!(
                "the layout {layout_id} is not the current one ({}): the displays changed",
                layout.id
            ));
        }
        if layout.is_virtual() {
            return Err("a foreign layout is shown, never edited".to_owned());
        }
        Ok(layout)
    }

    /// The current layout, if `layout_id` names it, it takes edits, and `document` was
    /// made for the same machines.
    ///
    /// The id alone does not say so: a remote whose screens are all panels this
    /// machine has adds no monitor, so the id stays the same whether it is linked or
    /// not. A document made while it was (its sources say which machine they are on)
    /// applied to the layout made after it left, or the other way round, would be an
    /// edit of a desktop that is not there.
    fn editable_by(
        &mut self,
        layout_id: &str,
        document: &LayoutDocument,
    ) -> Result<&mut Layout, String> {
        let layout = self.editable(layout_id)?;
        if let Some(stored) = &document.layout {
            let mut theirs: Vec<String> = stored
                .monitors
                .values()
                .filter_map(|m| m.sources.as_ref())
                .flat_map(|sources| sources.values())
                .filter_map(|s| s.host.clone())
                .collect();
            theirs.sort();
            theirs.dedup();
            if theirs != layout.hosts() {
                return Err(format!(
                    "the layout {layout_id} was edited with other machines than are linked \
                     now: the displays changed"
                ));
            }
        }
        Ok(layout)
    }

    fn outputs(&self) -> Vec<lbm_layout::linux::LinuxMonitor> {
        self.discovery.outputs()
    }
}

impl<S: LayoutStore, P: PersistencePlatform> World for SystemWorld<S, P> {
    /// This machine's signature, followed, while remotes are merged, by a hash of
    /// their screens: a remote linking, leaving or changing its screens has to be a
    /// change here, or the settle loop would take the rebuild for one with nothing to
    /// do. Unchanged when there is no remote, so nothing is rebuilt on that account.
    fn display_signature(&mut self) -> String {
        let local = self.discovery.signature();
        if self.remotes.is_empty() {
            return local;
        }
        let remotes = serde_json::to_string(&self.remotes).unwrap_or_default();
        format!(
            "{local}|remotes:{}",
            lbm_ipc::protocol::fingerprint(&remotes)
        )
    }

    fn rebuild_layout(&mut self) {
        self.preview = None;
        let mut layout = Layout::new(LayoutOptions::default());
        let first_load = self.first_load.take();
        let persistence = &mut self.persistence;
        // Merged without the input values the remotes read: those are adopted below,
        // for the sources shown only (`input_switch::adopt_published`).
        let remotes = crate::input_switch::without_inputs(&self.remotes);
        let loaded = self.discovery.populate(&mut layout, |l| {
            // The remotes go in between this machine's screens and the id, which is
            // where `populate` calls the load: the id is then the one of every monitor
            // shown, so the profile loaded is the one of this set of machines, and the
            // store restores the remote monitors as it restores the others.
            merge_remotes(l, &remotes);
            if let Some(before) = first_load {
                before(&l.id, persistence.store());
            }
            persistence.load(l)
        });
        match loaded {
            Ok(()) => {
                self.layout = Some(layout);
                self.keep_switching();
                let adopted = self.layout.as_mut().is_some_and(|layout| {
                    crate::input_switch::adopt_published(layout, &self.remotes)
                });
                if adopted {
                    self.save_own_change();
                }
            }
            // C#: the factory throws, the handler logs, the previous layout stays.
            Err(error) => eprintln!("[lbm-agent] layout rebuild failed: {error}"),
        }
    }

    fn layout(&self) -> Option<LayoutState> {
        self.layout.as_ref().map(|l| LayoutState {
            enabled: l.options.enabled,
            is_virtual: l.is_virtual(),
            saved: l.saved(),
        })
    }

    fn set_enabled(&mut self, enabled: bool) {
        if let Some(layout) = &mut self.layout {
            layout.edit_options(|o| o.enabled = enabled);
        }
    }

    fn end_preview(&mut self) {
        self.preview = None;
    }

    fn wanted_fingerprint(&mut self) -> Option<String> {
        // The same document `zones()` would send, hashed: comparing anything else
        // would be comparing a summary of the layout with the layout.
        self.zones()
            .map(|(zones, _)| lbm_ipc::protocol::fingerprint(&zones))
    }
}

impl<S: LayoutStore, P: PersistencePlatform> AgentWorld for SystemWorld<S, P> {
    fn peer_displays(&mut self) -> Vec<lbm_ipc::peer::PeerDisplay> {
        self.discovery.peer_displays()
    }
    fn set_remote_screens(&mut self, remotes: Vec<RemoteScreens>) -> bool {
        if self.remotes == remotes {
            return false;
        }
        self.remotes = remotes;
        true
    }
    fn set_active_source(
        &mut self,
        layout_id: &str,
        monitor: &str,
        source: &str,
    ) -> Result<Option<crate::input_switch::InputSwitch>, String> {
        self.editable(layout_id)?;
        let layout = self.layout.as_mut().ok_or("no layout yet")?;
        let shown = layout
            .monitor(monitor)
            .ok_or_else(|| format!("{monitor} is not a monitor of this layout"))?;
        if !shown.sources.iter().any(|s| s == source) {
            return Err(format!("{source} is not a source of {monitor}"));
        }
        let previous = shown.active_source.clone();
        let (attached, value) = layout
            .source(source)
            .map(|s| (s.source.attached_to_desktop, s.source.input_source))
            .unwrap_or_default();
        // A source its machine's desktop does not hold has no zone: choosing it takes
        // the panel out of the engine until it is there. Only switching the monitor to
        // it can bring it there, so it is refused unless its input value is known.
        if !attached && value.is_none() {
            return Err(format!(
                "{source} is not on its machine's desktop, and its input is not known: that machine cannot show {monitor}"
            ));
        }
        let switch =
            crate::input_switch::plan(layout, &self.remotes, monitor, source, previous.as_deref());
        layout.set_active_source(monitor, Some(source.to_owned()));
        self.persistence.save(layout).map_err(|e| e.to_string())?;
        // The hook runs the preview while there is one. The choice is the user's, not
        // part of the experiment, so the preview shows it too.
        if let Some(preview) = self.preview.as_mut() {
            preview.set_active_source(monitor, Some(source.to_owned()));
        }
        self.switching.retain(|(m, _)| m != monitor);
        if !attached {
            self.switching.push((monitor.to_owned(), source.to_owned()));
        }
        Ok(switch)
    }
    fn input_reads(&self, publishing: bool) -> Vec<crate::input_switch::InputRead> {
        self.layout
            .as_ref()
            .map(|layout| crate::input_switch::reads(layout, publishing))
            .unwrap_or_default()
    }
    fn learn_inputs(&mut self, readings: &[(crate::input_switch::InputRead, u16)]) {
        let learnt = self
            .layout
            .as_mut()
            .is_some_and(|layout| crate::input_switch::learn(layout, readings));
        if learnt {
            self.save_own_change();
        }
    }
    fn select_input_target(&self, display: &str) -> Result<crate::vcp::Target, String> {
        let layout = self.layout.as_ref().ok_or("no layout yet")?;
        let monitor = display_monitor(layout, display)
            .ok_or_else(|| format!("{display} is not a monitor of this machine"))?;
        // Only a monitor wired to this machine can be reached (`vcp::target`).
        crate::vcp::target(layout, &monitor)
    }
    fn display_monitor(&self, display: &str) -> Option<String> {
        display_monitor(self.layout.as_ref()?, display)
    }
    fn switch_failed(&mut self, monitor: &str, source: &str) -> bool {
        let waiting = self
            .switching
            .iter()
            .any(|(m, s)| m == monitor && s == source);
        if !waiting {
            return false;
        }
        self.switching.retain(|(m, _)| m != monitor);
        let Some(layout) = self.layout.as_mut() else {
            return false;
        };
        // The rule picks a source that is on its machine's desktop, as it would have.
        layout.resolve_active_sources();
        let now = layout
            .monitor(monitor)
            .and_then(|m| m.active_source.clone());
        if now.as_deref() == Some(source) {
            return false;
        }
        if let Some(preview) = self.preview.as_mut() {
            preview.set_active_source(monitor, now);
        }
        self.save_own_change();
        true
    }
    fn vcp_target(&self, monitor: &str) -> Result<crate::vcp::Target, String> {
        crate::vcp::target(self.layout.as_ref().ok_or("No current layout.")?, monitor)
    }
    fn wallpaper(&self) -> Vec<crate::desktop::ScreenWallpaper> {
        let (Some(paths), Some(layout)) = (self.wallpaper.as_ref(), self.layout.as_ref()) else {
            return Vec::new();
        };
        // A foreign layout describes someone else's desktop: nothing here is ours to paint.
        if layout.is_virtual() {
            return Vec::new();
        }
        let all = wallpaper_settings::load(&paths.settings);
        let Some(settings) = all.get(&layout.id).filter(|s| s.has_content()) else {
            return Vec::new();
        };
        crate::wallpaper_job::screens(layout, settings, &paths.slices).unwrap_or_else(|error| {
            eprintln!("[lbm-agent] wallpaper: {error}");
            Vec::new()
        })
    }
    fn saved_wallpaper_job(&self) -> Option<crate::wallpaper_job::Job> {
        let paths = self.wallpaper.as_ref()?;
        let layout = self.layout.as_ref()?;
        if layout.is_virtual() {
            return None;
        }
        let settings = wallpaper_settings::load(&paths.settings).remove(&layout.id)?;
        if !settings.has_content() {
            return None;
        }
        Some(crate::wallpaper_job::Job {
            layout: layout.clone(),
            settings,
            store: paths.settings.clone(),
            images: paths.slices.clone(),
        })
    }
    fn wallpaper_settings(&mut self, id: &str) -> Result<LayoutWallpaperSettings, String> {
        self.editable(id)?;
        let paths = self
            .wallpaper
            .as_ref()
            .ok_or("this agent keeps no wallpaper")?;
        Ok(wallpaper_settings::load_checked(&paths.settings)
            .map_err(|e| e.to_string())?
            .remove(id)
            .unwrap_or_default())
    }
    fn wallpaper_job(
        &mut self,
        id: &str,
        settings: LayoutWallpaperSettings,
    ) -> Result<crate::wallpaper_job::Job, String> {
        self.editable(id)?;
        let paths = self
            .wallpaper
            .as_ref()
            .ok_or("this agent keeps no wallpaper")?;
        Ok(crate::wallpaper_job::Job {
            layout: self.layout.as_ref().ok_or("No layout")?.clone(),
            settings,
            store: paths.settings.clone(),
            images: paths.slices.clone(),
        })
    }

    fn save_wallpaper(
        &mut self,
        layout_id: &str,
        settings: LayoutWallpaperSettings,
    ) -> Result<Vec<crate::desktop::ScreenWallpaper>, String> {
        let Some(paths) = self.wallpaper.clone() else {
            return Err("this agent keeps no wallpaper".to_owned());
        };
        // The same rule as an edit: the settings are keyed by layout, and a frontend
        // whose displays have changed under it must not write over the new one.
        self.editable(layout_id)?;
        crate::wallpaper_cache::save_settings(&paths.settings, layout_id, settings)
            .map_err(|error| format!("the wallpaper settings could not be written: {error}"))?;
        Ok(self.wallpaper())
    }

    fn zones(&self) -> Option<(String, bool)> {
        self.preview
            .as_ref()
            .or(self.layout.as_ref())
            .map(|l| (compute_zones(l).serialize(), l.is_virtual()))
    }

    fn desktop(&self) -> Option<lbm_ipc::protocol::Desktop> {
        let bounds = local_desktop(self.preview.as_ref().or(self.layout.as_ref())?)?;
        // Truncated the way the zones' own pixel rectangles are written to the wire,
        // so the two describe the same grid.
        Some(lbm_ipc::protocol::Desktop {
            left: bounds.left() as i32,
            top: bounds.top() as i32,
            width: bounds.width() as i32,
            height: bounds.height() as i32,
        })
    }

    fn save_enabled(&mut self) -> io::Result<()> {
        match &self.layout {
            Some(layout) => self.persistence.save_enabled(layout).map(|_| ()),
            None => Ok(()),
        }
    }

    fn save_layout(&mut self) -> io::Result<()> {
        match &mut self.layout {
            Some(layout) => self.persistence.save(layout).map(|_| ()),
            None => Ok(()),
        }
    }

    fn layout_id(&self) -> Option<String> {
        self.layout.as_ref().map(|l| l.id.clone())
    }

    fn edit(&mut self, layout_id: &str, document: &LayoutDocument) -> Result<(), String> {
        document.apply(self.editable_by(layout_id, document)?);
        Ok(())
    }

    fn probe_zones(
        &mut self,
        layout_id: &str,
        document: &LayoutDocument,
    ) -> Result<String, String> {
        let mut copy = self.editable_by(layout_id, document)?.clone();
        document.apply(&mut copy);
        Ok(compute_zones(&copy).serialize())
    }

    fn display_outputs(&mut self) -> Result<lbm_ipc::displays::Outputs, String> {
        match self.discovery {
            Discovery::Linux(backend) => backend.connected_outputs().map(crate::displays::outputs),
            #[cfg(windows)]
            Discovery::Windows => lbm_display::windows::topology::connected_outputs()
                .and_then(crate::displays::windows),
            _ => Err("Monitor actions are not available on this display backend.".into()),
        }
    }

    fn change_output(
        &mut self,
        connector: &str,
        change: lbm_ipc::displays::Change,
        signature: &str,
    ) -> Result<lbm_ipc::displays::Outputs, String> {
        match self.discovery {
            Discovery::Linux(backend) => {
                use lbm_display::linux::topology::{self, Change};
                let action = match change {
                    lbm_ipc::displays::Change::Primary => Change::Primary,
                    lbm_ipc::displays::Change::Attach => Change::Attach,
                    lbm_ipc::displays::Change::Detach => Change::Detach,
                };
                topology::change_checked(backend, action, connector, signature)
                    .map(crate::displays::outputs)
            }
            #[cfg(windows)]
            Discovery::Windows => {
                use lbm_display::windows::topology::{self, Change};
                let action = match change {
                    lbm_ipc::displays::Change::Primary => Change::Primary,
                    lbm_ipc::displays::Change::Attach => Change::Attach,
                    lbm_ipc::displays::Change::Detach => Change::Detach,
                };
                topology::change_checked(action, connector, signature)
                    .and_then(crate::displays::windows)
            }
            _ => Err("Monitor actions are not available on this display backend.".into()),
        }
    }

    /// Save the edit, then move the screens to match it.
    ///
    /// **Saved first, and the C# says why**: the system change triggers a rebuild that
    /// re-imports the system layout and then loads the saved one, so the physical layout
    /// the user just applied survives only if it was written down first.
    ///
    /// `before_apply` is where the engine's 1px gaps are closed. `lbm-display` cannot do
    /// it — the gap guard is the agent's (D7) and a frontend must not link the agent —
    /// which is the whole reason this request exists rather than the frontend applying
    /// for itself. Restoring deletes the journal, so the old positions can never be
    /// "restored" over the ones applied here; an engine still running re-gaps the new
    /// topology on the rebuild that follows.
    #[cfg(target_os = "linux")]
    fn apply_topology(
        &mut self,
        layout_id: &str,
        document: &LayoutDocument,
        adjust_scale: bool,
        how: lbm_display::linux::topology::How,
    ) -> Result<Vec<String>, String> {
        use lbm_display::linux::topology::How;
        let dry = how == How::DryRun;
        // The layout is borrowed for this block only, so the gap guard below is free.
        let wanted: Vec<lbm_display::linux::topology::Wanted> = {
            document.apply(self.editable_by(layout_id, document)?);
            let layout = self.layout.as_mut().ok_or("no layout yet")?;
            // **A dry run does not save either.** The save is not a detail of applying,
            // it is a write to the user's profile — and a preview that rewrote the thing
            // it was previewing would be the worst kind of surprise. The edit stays on
            // this agent's in-memory copy, exactly as `edit` leaves it.
            if !dry {
                self.persistence.save(layout).map_err(|e| e.to_string())?;
            }
            layout
                .compute_pixel_locations_from_physical(adjust_scale)
                .into_iter()
                .filter_map(|(source_id, placement)| {
                    // The connector is what both tools name an output by, and what the
                    // Linux factory puts in `interface_path` (`linux.rs`, `AddMonitor`).
                    let connector = layout.source(&source_id)?.source.interface_path.clone()?;
                    Some(lbm_display::linux::topology::Wanted {
                        connector,
                        x: placement.pixel_bounds.left(),
                        y: placement.pixel_bounds.top(),
                        scale: placement.scale,
                    })
                })
                .collect()
        };
        if wanted.is_empty() {
            return Err("no attached screen to place".to_owned());
        }

        let Some(backend) = lbm_display::linux::Backend::detect() else {
            return Err("no display backend to apply through".to_owned());
        };
        let gaps = &self.gaps;
        lbm_display::linux::topology::apply(backend, &wanted, how, |before| {
            let Some(gaps) = gaps else {
                return Vec::new();
            };
            // The gap guard's own writes go before everything else, so a dry run that
            // left them out would understate what pressing Apply does.
            //
            // **Two paths, and not out of laziness.** `restore` deletes the journal when
            // it succeeds; a dry run that called it with a runner that merely recorded
            // would report success and take the journal with it — leaving a gapped
            // topology nothing could put back. A "dry" that writes is not dry.
            if dry {
                // One invocation, not one per argument: `restore` hands the whole list
                // to a single `kscreen-doctor`, and nothing at all when it is empty.
                let moves = gaps.would_restore(before);
                return match moves.is_empty() {
                    true => Vec::new(),
                    false => vec![format!("kscreen-doctor {}", moves.join(" "))],
                };
            }
            let mut said = Vec::new();
            gaps.restore(before, |args| {
                said.push(format!("kscreen-doctor {}", args.join(" ")));
                run_kscreen_doctor(args)
            });
            said
        })
        .map(|applied| applied.commands)
    }

    fn topology_requires_stopped_engine(&self) -> bool {
        #[cfg(windows)]
        {
            matches!(self.discovery, Discovery::Windows)
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    #[cfg(windows)]
    fn apply_topology(
        &mut self,
        layout_id: &str,
        document: &LayoutDocument,
        adjust_scale: bool,
        how: lbm_display::linux::topology::How,
    ) -> Result<Vec<String>, String> {
        if !matches!(self.discovery, Discovery::Windows) {
            return Err("This discovery backend cannot change Windows display positions.".into());
        }
        let (mut edited, wanted) = crate::windows_topology::prepare(
            self.editable_by(layout_id, document)?,
            document,
            adjust_scale,
        )?;
        let dry = how == lbm_display::linux::topology::How::DryRun;
        let report = lbm_display::windows::topology::placement::apply(&wanted, dry, || {
            self.persistence
                .save(&mut edited)
                .map_err(|e| e.to_string())?;
            // The subsequent display notification reloads this saved physical layout.
            self.layout = Some(edited.clone());
            Ok(())
        })?;
        Ok(report)
    }

    fn set_preview(&mut self, layout_id: &str, document: &LayoutDocument) -> Result<(), String> {
        let mut preview = self.editable_by(layout_id, document)?.clone();
        document.apply(&mut preview);
        self.preview = Some(preview);
        Ok(())
    }

    fn save_options(
        &mut self,
        options: Option<&GlobalOptionsDto>,
        excluded: Option<&[String]>,
        load_at_startup: Option<bool>,
    ) -> Result<(), String> {
        let apply = |o: &mut LayoutOptions| {
            apply_global_options(o, options);
            if let Some(excluded) = excluded {
                o.excluded_list = excluded.to_vec();
            }
        };
        // The hook runs the preview while there is one: it must see the options too.
        if let Some(preview) = self.preview.as_mut() {
            preview.edit_options(apply);
        }
        let layout = self.layout.as_mut().ok_or("no layout yet")?;
        layout.edit_options(apply);
        // C# `UpdateSchedule`, after the options: StartElevated may come in the same
        // request. The option ends up as what the session will do; the rest is stored
        // whatever became of the autostart.
        let autostart = load_at_startup
            .map(|requested| self.persistence.set_load_at_startup(layout, requested));
        self.persistence
            .save_live(&layout.options)
            .map_err(|e| e.to_string())?;
        autostart.unwrap_or(Ok(()))
    }

    fn app_options(&self) -> Option<(bool, bool)> {
        self.layout
            .as_ref()
            .map(|l| (l.options.load_at_startup, l.options.hide_tray_icon))
    }

    fn rescue_shortcut(&self) -> Option<String> {
        self.layout
            .as_ref()
            .map(|l| l.options.rescue_shortcut.clone())
    }

    fn bound_to_agent(&self) -> Option<bool> {
        self.layout.as_ref().map(|l| l.options.bound_to_agent)
    }

    fn excluded(&self) -> Option<Vec<String>> {
        self.layout
            .as_ref()
            .map(|l| l.options.excluded_list.clone())
    }

    fn prepare_for_engine(&mut self) -> bool {
        match &self.gaps {
            Some(gaps) => gaps.apply(&self.outputs(), run_kscreen_doctor),
            None => false,
        }
    }

    fn restore_after_engine(&mut self) -> bool {
        match &self.gaps {
            Some(gaps) => gaps.restore(&self.outputs(), run_kscreen_doctor),
            None => false,
        }
    }

    fn recover_stale(&mut self) -> bool {
        match &self.gaps {
            Some(gaps) => gaps.recover_stale(&self.outputs(), run_kscreen_doctor),
            None => false,
        }
    }
}

/// Merges the remotes' screens into a layout holding this machine's, and gives it the
/// id of the whole set. Nothing at all without remotes: the id is then the one
/// `populate` computed, and a machine alone builds exactly what it always has.
fn merge_remotes(layout: &mut Layout, remotes: &[RemoteScreens]) {
    if remotes.is_empty() {
        return;
    }
    for remote in remotes {
        lbm_display::peer::merge_peer(layout, &remote.machine_id, &remote.name, &remote.displays);
    }
    layout.id = layout.compute_id();
}

/// The monitor of `layout` that this machine reports to a master as `display`
/// (`peers::local`): its id on Linux, where the report uses the layout's ids, and its
/// device id on Windows, where it uses the device tree's. Only a monitor with a source
/// of this machine's is one.
fn display_monitor(layout: &Layout, display: &str) -> Option<String> {
    let local = |m: &&lbm_layout::model::Monitor| {
        m.sources
            .iter()
            .any(|s| layout.source(s).is_some_and(|s| s.source.is_local()))
    };
    let monitors = layout.monitors();
    monitors
        .iter()
        .filter(local)
        .find(|m| m.id == display)
        .or_else(|| {
            monitors
                .iter()
                .filter(local)
                .find(|m| m.device_id.as_deref() == Some(display))
        })
        .map(|m| m.id.clone())
}

/// This machine's desktop, in its pixels: what the hook's absolute pointing device
/// spans, so the remote screens never take part.
///
/// A shared panel showing another machine is still an output of this desktop (the
/// compositor does not know the panel looks elsewhere), so its local source counts
/// although it is not the one shown. Without remotes, exactly
/// [`Layout::desktop_pixel_bounds`].
fn local_desktop(layout: &Layout) -> Option<lbm_layout::geo::Rect> {
    if layout.hosts().is_empty() {
        return layout.desktop_pixel_bounds();
    }
    layout
        .sources()
        .filter(|s| s.source.is_local() && s.source.attached_to_desktop)
        .filter(|s| {
            layout.monitor(&s.monitor).is_some_and(|m| {
                m.active_source.as_deref() == Some(s.source.id.as_str())
                    || !layout.shows_local_source(m)
            })
        })
        .map(|s| s.source.in_pixel.bounds())
        .reduce(|a, b| {
            // Min/max, not `Rect::union`: see `desktop_pixel_bounds`.
            let left = a.left().min(b.left());
            let top = a.top().min(b.top());
            lbm_layout::geo::Rect::new(
                left,
                top,
                a.right().max(b.right()) - left,
                a.bottom().max(b.bottom()) - top,
            )
        })
}

/// Starting with the session: the XDG autostart entry (Linux), the scheduled task
/// (Windows).
pub trait SessionStart: Send + Sync + std::fmt::Debug {
    /// Whether the session starts the agent (C#: `IsAutostartScheduled`). On Windows, a
    /// task whose program is gone is pointed back at this agent on the way, or answered
    /// `false` when it cannot be ([`crate::schtask`]).
    fn scheduled(&self) -> bool;
    /// Aligns it on the options (C#: `SetAutostart`).
    fn schedule(&self, enabled: bool, elevated: bool) -> io::Result<()>;
}

impl SessionStart for XdgAutostart {
    fn scheduled(&self) -> bool {
        self.is_scheduled()
    }

    /// Elevation is Windows' business.
    fn schedule(&self, enabled: bool, _elevated: bool) -> io::Result<()> {
        self.set(enabled)
    }
}

#[cfg(windows)]
impl SessionStart for crate::schtask::ScheduledTask {
    fn scheduled(&self) -> bool {
        self.is_scheduled()
    }

    fn schedule(&self, enabled: bool, elevated: bool) -> io::Result<()> {
        self.set(enabled, elevated)
    }
}

/// The persistence platform hooks on this machine: elevation (root / administrator), and
/// starting with the session ([`SessionStart`]); none without one.
#[derive(Debug, Default)]
pub struct Platform {
    autostart: Option<Box<dyn SessionStart>>,
}

impl Platform {
    /// Starting with the session through `autostart`.
    pub fn with_autostart(autostart: impl SessionStart + 'static) -> Self {
        Platform {
            autostart: Some(Box::new(autostart)),
        }
    }
}

impl PersistencePlatform for Platform {
    #[cfg(unix)]
    fn is_elevated(&self) -> bool {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(windows)]
    fn is_elevated(&self) -> bool {
        crate::winpipe::is_elevated()
    }

    fn is_autostart_scheduled(&self, _layout: &Layout) -> bool {
        self.autostart.as_ref().is_some_and(|a| a.scheduled())
    }

    /// Every save aligns it (C#: `SetAutostart`). Without a way to start with the
    /// session, asking for one is the error.
    fn set_autostart(&self, _layout: &Layout, enabled: bool, elevated: bool) -> io::Result<()> {
        match &self.autostart {
            Some(autostart) => autostart.schedule(enabled, elevated),
            None if enabled => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this agent has no session autostart",
            )),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use lbm_layout::linux::{add_monitor, LinuxMonitor};
    use lbm_layout::model::{Layout, LayoutOptions};
    use lbm_layout::remote::{merge_remote, RemoteMonitor};

    use super::display_monitor;

    #[test]
    fn a_reported_display_is_found_by_its_id_or_its_device_id_and_only_among_this_machines() {
        let mut layout = Layout::new(LayoutOptions::default());
        add_monitor(&mut layout, &LinuxMonitor::fallback());
        // Windows reports a monitor by its device id, which the layout keeps apart from
        // its own id.
        let id = layout.monitors()[0].id.clone();
        let mut monitor = layout.monitors()[0].clone();
        monitor.device_id = Some(r"MONITOR\DEL4065\{4d36e96e}\0003".into());
        layout.add_or_update_monitor(monitor);
        assert_eq!(display_monitor(&layout, &id), Some(id.clone()));
        assert_eq!(
            display_monitor(&layout, r"MONITOR\DEL4065\{4d36e96e}\0003"),
            Some(id)
        );
        // A monitor of another machine is never one of this machine's displays.
        merge_remote(
            &mut layout,
            "a1b2c3d4e5f60718293a4b5c6d7e8f90",
            "laptop",
            &[RemoteMonitor {
                id: "HDMI-1".into(),
                connector: "HDMI-1".into(),
                width: 1920,
                height: 1080,
                scale: 1.0,
                attached: true,
                ..RemoteMonitor::default()
            }],
        );
        assert_eq!(display_monitor(&layout, "HDMI-1@a1b2c3d4"), None);
        assert_eq!(display_monitor(&layout, "HDMI-1"), None);
    }
}
