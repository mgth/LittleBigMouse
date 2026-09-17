//! Apply the physical-layout solver's positions without changing modes or scaling.
use lbm_layout::geo::Rect;

use super::{Backend, Configuration, Plan};

#[derive(Clone, Debug)]
pub struct Wanted {
    /// Stable monitor interface path, never a DISPLAYn name.
    pub connector: String,
    /// Bounds from the layout's original detection, used to reject stale edits.
    pub previous: Rect,
    pub x: f64,
    pub y: f64,
}

fn coordinate(value: f64) -> Result<i32, String> {
    let rounded = value.round();
    if !rounded.is_finite() || rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
        return Err("A requested Windows position is not a finite 32-bit coordinate.".into());
    }
    Ok(rounded as i32)
}

impl Configuration {
    pub fn placement_plan(&self, wanted: &[Wanted]) -> Result<Plan, String> {
        self.outputs()?; // Reject ambiguous target identities and missing source modes.
        let active: Vec<_> = self.paths.iter().filter(|p| p.active).collect();
        if active.is_empty() || wanted.len() != active.len() {
            return Err("The layout must place every active Windows monitor; reload it.".into());
        }
        let mut plan = Plan {
            paths: active.iter().map(|p| p.id).collect(),
            activate: None,
            positions: Vec::new(),
            allow_changes: false,
        };
        let mut targets = Vec::new();
        let mut has_primary = false;
        for w in wanted {
            let p = active
                .iter()
                .find(|p| p.available && p.connector.eq_ignore_ascii_case(&w.connector))
                .ok_or("A monitor in the layout is no longer active; reload the layout.")?;
            if targets.contains(&p.id.target) {
                return Err("The layout places the same Windows monitor more than once.".into());
            }
            targets.push(p.id.target);
            let old = self.source(p.id.source)?;
            if w.previous
                != Rect::new(
                    old.x as f64,
                    old.y as f64,
                    old.width as f64,
                    old.height as f64,
                )
            {
                return Err(
                    "The Windows display positions or modes changed; reload the layout.".into(),
                );
            }
            let (x, y) = (coordinate(w.x)?, coordinate(w.y)?);
            if old.x == 0 && old.y == 0 {
                has_primary = true;
                if (x, y) != (0, 0) {
                    return Err("The current primary monitor must stay at the origin.".into());
                }
            }
            if let Some(&(_, px, py)) = plan.positions.iter().find(|(id, _, _)| *id == old.id) {
                if (x, y) != (px, py) {
                    return Err("Cloned monitors cannot be placed at different positions.".into());
                }
            } else {
                plan.positions.push((old.id, x, y));
            }
        }
        if !has_primary {
            return Err("Windows did not report a primary monitor at the origin.".into());
        }
        Ok(plan)
    }
}

/// Validate before saving, save before moving, and read back even after a driver error.
/// A dry run calls neither the profile writer nor SetDisplayConfig(APPLY).
pub fn apply_with(
    backend: &mut impl Backend,
    wanted: &[Wanted],
    dry_run: bool,
    save_profile: impl FnOnce() -> Result<(), String>,
) -> Result<Vec<String>, String> {
    let before = backend.query()?;
    let plan = before.placement_plan(wanted)?;
    let signature = before.signature();
    let mut report = vec!["Save the physical layout profile.".to_owned()];
    for &(id, x, y) in &plan.positions {
        let old = before.source(id)?;
        if (old.x, old.y) != (x, y) {
            for path in before
                .paths
                .iter()
                .filter(|p| p.active && p.id.source == id)
            {
                report.push(format!(
                    "{} [{}]: ({}, {}) -> ({x}, {y}) px; keep {} x {} px.",
                    path.name, path.connector, old.x, old.y, old.width, old.height
                ));
            }
        }
    }
    let moved = report.len() > 1;
    if moved {
        backend.validate(&plan)?;
        report.push(
            "Apply and persist one Windows display configuration; verify positions and modes."
                .into(),
        );
    } else {
        report.push("Keep current Windows display positions.".into());
    }
    if backend.query()?.signature() != signature {
        return Err("The displays changed during validation; reload the layout.".into());
    }
    if dry_run {
        return Ok(report);
    }
    save_profile()?;
    // Profile I/O can take time too. Never submit a plan materialized from newer modes.
    if backend.query()?.signature() != signature {
        return Err("The displays changed while saving; reload the layout before applying.".into());
    }
    if !moved {
        return Ok(report);
    }
    let applied = backend.apply(&plan);
    let reread = backend.query();
    applied?;
    let after = reread?;
    let active: Vec<_> = after
        .paths
        .iter()
        .filter(|p| p.active)
        .map(|p| p.id)
        .collect();
    if active != plan.paths || after.outputs()? != before.outputs()? {
        return Err(
            "Windows changed the active monitors or primary unexpectedly; reload the layout."
                .into(),
        );
    }
    for (id, x, y) in plan.positions {
        let mut expected = before.source(id)?.clone();
        expected.x = x;
        expected.y = y;
        if after.source(id)? != &expected {
            return Err("Windows did not confirm the requested positions and unchanged modes; reload the layout.".into());
        }
    }
    Ok(report)
}

#[cfg(windows)]
pub fn apply(
    wanted: &[Wanted],
    dry_run: bool,
    save_profile: impl FnOnce() -> Result<(), String>,
) -> Result<Vec<String>, String> {
    apply_with(
        &mut super::native::Native::default(),
        wanted,
        dry_run,
        save_profile,
    )
}
