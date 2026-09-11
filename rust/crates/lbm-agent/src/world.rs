//! What the agent acts on: the displays, the stored profiles and the current layout.
//!
//! [`AgentWorld`] is the [`World`] the reconciler reads, plus what the runtime needs to
//! carry out its effects: the zones to hand the hook, and the two kinds of save.
//! [`SystemWorld`] is the real one — the displays found by `lbm-display`, the layout
//! built by `lbm_layout::linux::populate` with the profile the persistence engine
//! loads (C#: `LinuxLayoutFactory.Create`).

use std::io;

use lbm_display::linux::{display_signature, Backend};
use lbm_layout::linux::populate;
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_layout::zoning::compute_zones;
use lbm_store::layout_dto_mapper::apply_global_options;
use lbm_store::{
    GlobalOptionsDto, LayoutDocument, LayoutPersistence, LayoutStore, PersistencePlatform,
};

use crate::autostart::XdgAutostart;
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

    /// Records the app-level options and the excluded list (C#: `SaveLive`).
    fn save_options(
        &mut self,
        _options: Option<&GlobalOptionsDto>,
        _excluded: Option<&[String]>,
    ) -> Result<(), String> {
        Err("this agent keeps no options".to_owned())
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

/// The real world on Linux: discovery, profiles, the layout.
pub struct SystemWorld<S, P> {
    backend: Option<Backend>,
    persistence: LayoutPersistence<S, P>,
    layout: Option<Layout>,
    /// A frontend's edit of `layout`, being previewed.
    preview: Option<Layout>,
    gaps: Option<GapGuard>,
}

impl<S: LayoutStore, P: PersistencePlatform> SystemWorld<S, P> {
    /// Discovery through `backend` (none: a single fallback output), profiles through
    /// `persistence`.
    pub fn new(backend: Option<Backend>, persistence: LayoutPersistence<S, P>) -> Self {
        SystemWorld {
            backend,
            persistence,
            layout: None,
            preview: None,
            gaps: None,
        }
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
        match self.backend.map(Backend::query) {
            Some(Ok(monitors)) => monitors,
            Some(Err(error)) => {
                eprintln!("[lbm-agent] display discovery failed: {error}");
                Vec::new()
            }
            None => Vec::new(),
        }
    }
}

impl<S: LayoutStore, P: PersistencePlatform> World for SystemWorld<S, P> {
    fn display_signature(&mut self) -> String {
        display_signature(&self.outputs())
    }

    fn rebuild_layout(&mut self) {
        self.preview = None;
        let outputs = self.outputs();
        let mut layout = Layout::new(LayoutOptions::default());
        match populate(&mut layout, &outputs, |l| self.persistence.load(l)) {
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
}

impl<S: LayoutStore, P: PersistencePlatform> AgentWorld for SystemWorld<S, P> {
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
    ) -> Result<(), String> {
        let layout = self.layout.as_mut().ok_or("no layout yet")?;
        layout.edit_options(|o| {
            apply_global_options(o, options);
            if let Some(excluded) = excluded {
                o.excluded_list = excluded.to_vec();
            }
        });
        self.persistence
            .save_live(&layout.options)
            .map_err(|e| e.to_string())
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

/// The persistence platform hooks on this machine: elevation (root / administrator), and
/// starting with the session — the XDG autostart entry under Linux; none without one
/// (Windows' scheduled task comes with the Windows agent).
#[derive(Debug, Default)]
pub struct Platform {
    autostart: Option<XdgAutostart>,
}

impl Platform {
    /// Starting with the session through `autostart`.
    pub fn with_autostart(autostart: XdgAutostart) -> Self {
        Platform {
            autostart: Some(autostart),
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
        // The Windows platform (elevation, autostart task) comes with the Windows agent.
        false
    }

    fn is_autostart_scheduled(&self, _layout: &Layout) -> bool {
        self.autostart
            .as_ref()
            .is_some_and(XdgAutostart::is_scheduled)
    }

    /// Every save aligns it (C#: `SetAutostart`); elevation is Windows' business.
    fn set_autostart(&self, _layout: &Layout, enabled: bool, _elevated: bool) {
        if let Some(autostart) = &self.autostart {
            if let Err(error) = autostart.set(enabled) {
                eprintln!("[lbm-agent] cannot set the session autostart: {error}");
            }
        }
    }
}
