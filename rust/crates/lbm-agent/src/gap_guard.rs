//! The engine's topology prologue and epilogue under KWin Wayland — port of
//! `KScreenGapGuard.cs` (decision D7: it moves into the agent).
//!
//! Both KWin (xdg-desktop-portal-kde) and mutter reject an InputCapture barrier on an
//! edge shared by two contiguous outputs: the portal was designed for barriers on the
//! outside of the desktop. LittleBigMouse intercepts *interior* crossings, so while the
//! engine runs the outputs are shifted apart by one logical pixel: every shared edge
//! becomes an outer edge, the hook's barriers pass, and the hook alone routes monitor
//! crossings — the Windows semantic.
//!
//! The original positions are journaled BEFORE the compositor is touched, so a crash
//! anywhere is recovered at the next start ([`GapGuard::recover_stale`]). An output the
//! user moved while gapped is left where they put it. The journal file is the C# one
//! (`kscreen-restore.json`, same shape), so either program recovers the other's.
//!
//! Skipped where it is useless: outside a Wayland Plasma session (X11 has no barriers),
//! and when the hook will use its evdev/uinput router instead of the portal (it grabs the
//! devices and routes directly: gaps would only leave 1 px void columns).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use indexmap::IndexMap;
use lbm_layout::geo::dotnet;
use lbm_layout::linux::LinuxMonitor;
use serde::{Deserialize, Serialize};

/// One output moved by the prologue (C#: `GapEntry`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GapEntry {
    pub name: String,
    pub original_x: i32,
    pub original_y: i32,
    pub applied_x: i32,
    pub applied_y: i32,
}

/// `(int)Math.Round(v)`: half to even, saturating.
fn r(v: f64) -> i32 {
    dotnet::round(v) as i32
}

fn overlaps(start1: i32, length1: i32, start2: i32, length2: i32) -> bool {
    (start1 + length1).min(start2 + length2) - start1.max(start2) > 0
}

/// C# `ComputeShifts`: one shift per output that must move. For every vertical boundary
/// where two outputs touch (A.right == B.left with a vertical overlap), every output at
/// or beyond it moves +1 in x — cumulative, so relative alignment elsewhere is kept and
/// each shared edge opens by exactly one pixel. The same on the y axis for stacks.
pub fn compute_shifts(monitors: &[LinuxMonitor]) -> IndexMap<String, (i32, i32)> {
    let mut x_cuts = BTreeSet::new();
    let mut y_cuts = BTreeSet::new();
    for (i, a) in monitors.iter().enumerate() {
        for (j, b) in monitors.iter().enumerate() {
            if i == j {
                continue;
            }
            let (ax, ay, aw, ah) = (
                r(a.logical_x),
                r(a.logical_y),
                r(a.logical_width),
                r(a.logical_height),
            );
            let (bx, by, bw, bh) = (
                r(b.logical_x),
                r(b.logical_y),
                r(b.logical_width),
                r(b.logical_height),
            );
            if ax + aw == bx && overlaps(ay, ah, by, bh) {
                x_cuts.insert(bx);
            }
            if ay + ah == by && overlaps(ax, aw, bx, bw) {
                y_cuts.insert(by);
            }
        }
    }

    let mut shifts = IndexMap::new();
    for m in monitors {
        let dx = x_cuts.iter().filter(|c| **c <= r(m.logical_x)).count() as i32;
        let dy = y_cuts.iter().filter(|c| **c <= r(m.logical_y)).count() as i32;
        if dx != 0 || dy != 0 {
            shifts.insert(m.connector_name.clone(), (dx, dy));
        }
    }
    shifts
}

/// The prologue's plan over the enabled outputs and the journal as it stands: the
/// journal to write, and the `kscreen-doctor` arguments; `None` when nothing moves.
/// Entries already journaled keep their original positions (the engine restarted while
/// gapped, or an output was plugged mid-run: the live geometry is the gapped one).
pub fn plan_apply(
    enabled: &[LinuxMonitor],
    journal: &[GapEntry],
) -> Option<(Vec<GapEntry>, Vec<String>)> {
    let shifts = compute_shifts(enabled);
    if shifts.is_empty() {
        return None;
    }
    let mut entries: IndexMap<String, GapEntry> = journal
        .iter()
        .map(|e| (e.name.clone(), e.clone()))
        .collect();
    let mut args = Vec::new();
    for m in enabled {
        let Some(&(dx, dy)) = shifts.get(&m.connector_name) else {
            continue;
        };
        let (x, y) = (r(m.logical_x), r(m.logical_y));
        let entry = entries
            .entry(m.connector_name.clone())
            .or_insert_with(|| GapEntry {
                name: m.connector_name.clone(),
                original_x: x,
                original_y: y,
                applied_x: 0,
                applied_y: 0,
            });
        entry.applied_x = x + dx;
        entry.applied_y = y + dy;
        args.push(format!(
            "output.{}.position.{},{}",
            m.connector_name,
            x + dx,
            y + dy
        ));
    }
    Some((entries.into_values().collect(), args))
}

/// The epilogue's plan: the `kscreen-doctor` arguments putting journaled outputs back.
/// An unplugged output has nothing to restore, one already back needs nothing, and one
/// whose live position is not the one applied was moved by the user: left alone.
pub fn plan_restore(journal: &[GapEntry], monitors: &[LinuxMonitor]) -> Vec<String> {
    journal
        .iter()
        .filter_map(|entry| {
            // C#: a dictionary by connector name, the last output of a name wins.
            let m = monitors
                .iter()
                .rev()
                .find(|m| m.connector_name == entry.name)?;
            let (x, y) = (r(m.logical_x), r(m.logical_y));
            if (x, y) == (entry.original_x, entry.original_y)
                || (x, y) != (entry.applied_x, entry.applied_y)
            {
                return None;
            }
            Some(format!(
                "output.{}.position.{},{}",
                entry.name, entry.original_x, entry.original_y
            ))
        })
        .collect()
}

/// The prologue and epilogue, over a journal file and a way to run `kscreen-doctor`.
pub struct GapGuard {
    journal: PathBuf,
    /// Gaps are needed at all: a Wayland Plasma session where the hook uses the portal.
    needed: bool,
}

impl GapGuard {
    /// The guard for this session, journaling to `journal` (C#:
    /// `<data dir>/kscreen-restore.json`).
    pub fn for_session(journal: PathBuf) -> Self {
        GapGuard {
            journal,
            needed: is_wayland_kde() && !evdev_likely_active(),
        }
    }

    /// C# `Apply`: open a 1 px logical gap at every shared edge between enabled outputs.
    /// Idempotent: a gapped topology plans no move. Returns whether the topology changed.
    pub fn apply(&self, monitors: &[LinuxMonitor], run: impl FnOnce(&[String]) -> bool) -> bool {
        if !self.needed {
            return false;
        }
        let enabled: Vec<LinuxMonitor> = monitors.iter().filter(|m| m.enabled).cloned().collect();
        let Some((journal, args)) = plan_apply(&enabled, &load(&self.journal)) else {
            return false;
        };
        // Journal first: the crash-safety net must exist before the compositor moves.
        if save(&self.journal, &journal).is_err() {
            return false;
        }
        run(&args)
    }

    /// What [`restore`](Self::restore) would run, running nothing and **leaving the
    /// journal alone**.
    ///
    /// The second half is the whole reason this exists rather than a flag on `restore`.
    /// `restore` deletes the journal when it succeeds, which is right for a real restore
    /// and quietly destructive for a preview: the journal is the only record of the gaps
    /// this agent opened, so a dry run that threw it away would leave a gapped topology
    /// with nothing left able to put it back. A "dry" that writes is not dry.
    pub fn would_restore(&self, monitors: &[LinuxMonitor]) -> Vec<String> {
        plan_restore(&load(&self.journal), monitors)
    }

    /// C# `Restore`: put the journaled outputs back. Returns whether the topology
    /// changed; the journal goes once nothing is left to restore.
    pub fn restore(&self, monitors: &[LinuxMonitor], run: impl FnOnce(&[String]) -> bool) -> bool {
        let journal = load(&self.journal);
        if journal.is_empty() {
            return false;
        }
        let moves = plan_restore(&journal, monitors);
        if moves.is_empty() {
            let _ = std::fs::remove_file(&self.journal);
            return false;
        }
        if !run(&moves) {
            // Kept: a later restore, or the next start, retries.
            return false;
        }
        let _ = std::fs::remove_file(&self.journal);
        true
    }

    /// C# `RecoverStale`, at startup: a leftover journal means a previous run died
    /// while the topology was gapped — restore it before anything is built on top.
    pub fn recover_stale(
        &self,
        monitors: &[LinuxMonitor],
        run: impl FnOnce(&[String]) -> bool,
    ) -> bool {
        self.journal.exists() && self.restore(monitors, run)
    }
}

fn load(path: &Path) -> Vec<GapEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Atomic, as C#: a torn journal would leave the topology unrecoverable. Written as
/// `System.Text.Json` indents it, so the C# reads it back.
fn save(path: &Path, entries: &[GapEntry]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(entries).map_err(std::io::Error::other)?;
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)
}

/// C# `LinuxDisplayController.Run("kscreen-doctor", ...)`: whether it exited cleanly.
pub fn run_kscreen_doctor(args: &[String]) -> bool {
    match Command::new("kscreen-doctor")
        .args(args)
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            eprintln!(
                "[lbm-agent] kscreen-doctor {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
            false
        }
        Err(error) => {
            eprintln!("[lbm-agent] kscreen-doctor {}: {error}", args.join(" "));
            false
        }
    }
}

/// C# `IsWaylandKde`: gaps only matter for the portal backend, under Wayland Plasma.
fn is_wayland_kde() -> bool {
    lbm_display::linux::desktop::DesktopEnvironment::current().is_kde()
        && (std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty()))
}

/// C# `EvdevLikelyActive`: the hook routes through evdev/uinput when it can write
/// `/dev/uinput` and read a physical input node. Opening them has no side effect: no
/// device is created without an ioctl, no node is grabbed by an open.
fn evdev_likely_active() -> bool {
    if std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .is_err()
    {
        return false;
    }
    let Ok(nodes) = std::fs::read_dir("/dev/input") else {
        return false;
    };
    nodes.filter_map(Result::ok).any(|node| {
        node.file_name().to_string_lossy().starts_with("event")
            && std::fs::File::open(node.path()).is_ok()
    })
}
