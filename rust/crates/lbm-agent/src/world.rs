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
use crate::reconcile::{LayoutState, World};

/// The world the runtime drives.
pub trait AgentWorld: World {
    /// The zones to hand the hook — the previewed layout's while there is one, else the
    /// current layout's — serialized, and whether the layout is foreign (simulated:
    /// loaded, never run). `None` before the first layout.
    fn zones(&self) -> Option<(String, bool)>;

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

    /// The layout to preview: the current one with the edit applied, the current one
    /// left as it is. Refused as [`edit`](Self::edit) is.
    fn set_preview(&mut self, _layout_id: &str, _document: &LayoutDocument) -> Result<(), String> {
        Err("this agent takes no previews".to_owned())
    }

    /// What the desktop should show for the layout now held: one target per screen,
    /// with any span slices cut (v6: the agent applies the wallpaper, so that a display
    /// change puts it back whether or not a window is open).
    fn wallpaper(&self) -> Vec<crate::desktop::ScreenWallpaper> {
        Vec::new()
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
        }
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

    fn outputs(&self) -> Vec<lbm_layout::linux::LinuxMonitor> {
        self.discovery.outputs()
    }
}

impl<S: LayoutStore, P: PersistencePlatform> World for SystemWorld<S, P> {
    fn display_signature(&mut self) -> String {
        self.discovery.signature()
    }

    fn rebuild_layout(&mut self) {
        self.preview = None;
        let mut layout = Layout::new(LayoutOptions::default());
        let first_load = self.first_load.take();
        let persistence = &mut self.persistence;
        let loaded = self.discovery.populate(&mut layout, |l| {
            if let Some(before) = first_load {
                before(&l.id, persistence.store());
            }
            persistence.load(l)
        });
        match loaded {
            Ok(()) => self.layout = Some(layout),
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
        crate::wallpaper::screens(layout, settings, &paths.slices)
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
        let mut all = wallpaper_settings::load(&paths.settings);
        all.insert(layout_id.to_owned(), settings);
        wallpaper_settings::save(&paths.settings, &all)
            .map_err(|error| format!("the wallpaper settings could not be written: {error}"))?;
        Ok(self.wallpaper())
    }

    fn zones(&self) -> Option<(String, bool)> {
        self.preview
            .as_ref()
            .or(self.layout.as_ref())
            .map(|l| (compute_zones(l).serialize(), l.is_virtual()))
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
        document.apply(self.editable(layout_id)?);
        Ok(())
    }

    fn set_preview(&mut self, layout_id: &str, document: &LayoutDocument) -> Result<(), String> {
        let mut preview = self.editable(layout_id)?.clone();
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
        let layout = self.layout.as_mut().ok_or("no layout yet")?;
        layout.edit_options(|o| {
            apply_global_options(o, options);
            if let Some(excluded) = excluded {
                o.excluded_list = excluded.to_vec();
            }
            if let Some(load_at_startup) = load_at_startup {
                o.load_at_startup = load_at_startup;
            }
        });
        // C# `UpdateSchedule`: the session autostart follows the option, and the store
        // keeps the rest.
        if load_at_startup.is_some() {
            self.persistence.platform().set_autostart(
                layout,
                layout.options.load_at_startup,
                layout.options.start_elevated,
            );
        }
        self.persistence
            .save_live(&layout.options)
            .map_err(|e| e.to_string())
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

/// Starting with the session: the XDG autostart entry (Linux), the scheduled task
/// (Windows).
pub trait SessionStart: Send + Sync + std::fmt::Debug {
    /// Whether the session starts the agent (C#: `IsAutostartScheduled`).
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

    /// Every save aligns it (C#: `SetAutostart`).
    fn set_autostart(&self, _layout: &Layout, enabled: bool, elevated: bool) {
        if let Some(autostart) = &self.autostart {
            if let Err(error) = autostart.schedule(enabled, elevated) {
                eprintln!("[lbm-agent] cannot set the session autostart: {error}");
            }
        }
    }
}
