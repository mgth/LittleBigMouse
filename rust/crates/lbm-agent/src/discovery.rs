//! Where the displays come from, as C#'s layout factories found them: `lbm-display`'s
//! Linux backends (KScreen, xrandr, sysfs — `LinuxLayoutFactory`), or the Win32 device
//! tree (`WindowsLayoutBuilder`).

use std::io;

use lbm_display::linux::{display_signature, Backend};
use lbm_layout::linux::LinuxMonitor;
use lbm_layout::model::Layout;

/// How the agent finds the displays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discovery {
    /// Nothing answers: one fallback output (C#'s factory does the same). What the tests
    /// use, on every platform.
    Fallback,
    /// The Linux backend that answered at start.
    Linux(Backend),
    /// The Win32 device tree (the process is per-monitor DPI aware: see `main`).
    #[cfg(windows)]
    Windows,
}

impl Discovery {
    /// This machine's: the Win32 tree on Windows, the first Linux backend that answers
    /// elsewhere.
    pub fn detect() -> Self {
        #[cfg(windows)]
        return Discovery::Windows;
        #[cfg(not(windows))]
        Backend::detect().map_or(Discovery::Fallback, Discovery::Linux)
    }

    /// A cheap fingerprint of the display configuration (C# `DisplaySignature`).
    pub fn signature(&self) -> String {
        match self {
            #[cfg(windows)]
            Discovery::Windows => lbm_display::windows::current_display_signature(),
            _ => display_signature(&self.outputs()),
        }
    }

    /// The layout of the displays now, its profile loaded by `load` (C#: the factory's
    /// `Create`: mapped, id computed, loaded, placed, anchored).
    pub fn populate(
        &self,
        layout: &mut Layout,
        load: impl FnOnce(&mut Layout) -> io::Result<()>,
    ) -> io::Result<()> {
        match self {
            #[cfg(windows)]
            Discovery::Windows => {
                let tree = lbm_display::windows::discover().map_err(io::Error::other)?;
                lbm_layout::windows::populate(
                    layout,
                    lbm_display::windows::thread_dpi_awareness(),
                    &tree.layout_input(),
                    load,
                )
            }
            _ => lbm_layout::linux::populate(layout, &self.outputs(), load),
        }
    }

    /// The Linux outputs (the KWin gaps move them); none elsewhere.
    pub fn outputs(&self) -> Vec<LinuxMonitor> {
        match self {
            Discovery::Linux(backend) => backend.query().unwrap_or_else(|error| {
                eprintln!("[lbm-agent] display discovery failed: {error}");
                Vec::new()
            }),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use lbm_layout::model::LayoutOptions;

    use super::*;

    #[test]
    fn with_nothing_to_ask_the_layout_has_one_fallback_output() {
        let mut layout = Layout::new(LayoutOptions::default());
        let mut loaded = None;
        Discovery::Fallback
            .populate(&mut layout, |l| {
                loaded = Some(l.id.clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(layout.monitors().len(), 1);
        assert_eq!(
            loaded.as_deref(),
            Some(layout.id.as_str()),
            "loaded by its id"
        );
        assert_eq!(
            Discovery::Fallback.signature(),
            Discovery::Fallback.signature()
        );
        assert!(Discovery::Fallback.outputs().is_empty());
    }
}
