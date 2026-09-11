//! Display changes the hook does not report — the poll of C#'s `LinuxLayoutFactory`.
//!
//! Every two seconds: the sysfs plug signature (a plug, an unplug, a monitor swapped),
//! and the modification time of the files the desktops rewrite the moment their
//! output layout changes — KWin's `kwinoutputconfig.json` (Plasma 6), mutter's
//! `monitors.xml` — for what the plug signature cannot see (a move, a scale, an
//! output switched off). A spurious rewrite settles to the same display signature and
//! is absorbed by the reconciler's idempotence guard.
//!
//! (inotify and DRM uevents are the planned upgrade; the poll stays as the fallback.)

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use lbm_display::linux::drm;
use tokio::sync::mpsc::UnboundedSender;

use crate::reconcile::Input;

/// C#: `LinuxLayoutFactory.PollInterval`.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// C#: `OutputConfigPaths`, in the XDG configuration directory.
fn output_config_paths() -> Vec<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")));
    config
        .map(|dir| vec![dir.join("kwinoutputconfig.json"), dir.join("monitors.xml")])
        .unwrap_or_default()
}

/// The latest modification of the output configuration files; `None` when none exists.
fn output_config_stamp(paths: &[PathBuf]) -> Option<SystemTime> {
    paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
}

/// What one tick found.
#[derive(Debug, Default)]
pub struct Poll {
    plug: String,
    stamp: Option<SystemTime>,
    started: bool,
}

impl Poll {
    /// One tick: whether the displays changed since the last. The first tick only
    /// records (C#: the stamp counts once it is known).
    pub fn tick(&mut self, plug: String, stamp: Option<SystemTime>) -> bool {
        let changed = self.started && (plug != self.plug || stamp != self.stamp);
        self.plug = plug;
        self.stamp = stamp;
        self.started = true;
        changed
    }
}

/// Polls until `inputs` is closed, sending [`Input::DisplayChanged`] on a change.
pub async fn poll_displays(inputs: UnboundedSender<Input>) {
    let paths = output_config_paths();
    let mut poll = Poll::default();
    let mut ticks = tokio::time::interval(POLL_INTERVAL);
    loop {
        ticks.tick().await;
        let plug = tokio::task::spawn_blocking(drm::plug_signature)
            .await
            .unwrap_or_default();
        if poll.tick(plug, output_config_stamp(&paths))
            && inputs.send(Input::DisplayChanged).is_err()
        {
            return;
        }
        if inputs.is_closed() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_a_different_plug_or_a_newer_output_file() {
        let t0 = SystemTime::UNIX_EPOCH;
        let t1 = t0 + Duration::from_secs(1);
        let mut poll = Poll::default();
        assert!(!poll.tick("a".into(), Some(t0)), "the first tick records");
        assert!(!poll.tick("a".into(), Some(t0)));
        assert!(poll.tick("b".into(), Some(t0)), "plugged");
        assert!(
            poll.tick("b".into(), Some(t1)),
            "the compositor rewrote its outputs"
        );
        assert!(!poll.tick("b".into(), Some(t1)));
    }
}
