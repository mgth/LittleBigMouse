//! Monitor actions addressed by CCD target identity, never by a recycled DISPLAYn.
//! The policy and transaction are platform neutral; `native` only translates Win32 data.
#[cfg(windows)]
mod native;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id {
    pub adapter: u64,
    pub id: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathId {
    pub source: Id,
    pub target: Id,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub id: PathId,
    pub connector: String,
    pub name: String,
    pub available: bool,
    pub active: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: Id,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Configuration {
    /// In CCD path priority order. Multiple inactive paths can name the same target.
    pub paths: Vec<Path>,
    pub sources: Vec<Source>,
    /// Rotation, refresh rates and native mode details used to reject stale inventories.
    pub details: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub connector: String,
    pub name: String,
    pub enabled: bool,
    pub primary: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Primary,
    Attach,
    Detach,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Exactly the paths that must remain active, in priority order.
    pub paths: Vec<PathId>,
    /// An attached path gets invalid mode indices: Windows chooses its supported mode.
    pub activate: Option<PathId>,
    /// Translated active source modes; all other mode data stays unchanged.
    pub positions: Vec<(Id, i32, i32)>,
    pub allow_changes: bool,
}
impl Configuration {
    pub fn signature(&self) -> String {
        format!("windows-ccd:{self:?}")
    }

    pub fn outputs(&self) -> Result<Vec<Output>, String> {
        let mut targets = Vec::new();
        let mut outputs = Vec::new();
        for p in &self.paths {
            if !p.available {
                continue;
            }
            if targets.contains(&p.id.target) {
                continue;
            }
            if p.connector.is_empty() {
                return Err("Windows did not identify a connected monitor.".into());
            }
            let active: Vec<_> = self
                .paths
                .iter()
                .filter(|a| a.id.target == p.id.target && a.active)
                .collect();
            if active.len() > 1 {
                return Err("Windows reported ambiguous active paths for a monitor.".into());
            }
            let primary = if let Some(active) = active.first() {
                let source = self.source(active.id.source)?;
                source.x == 0 && source.y == 0
            } else {
                false
            };
            if outputs
                .iter()
                .any(|o: &Output| o.connector.eq_ignore_ascii_case(&p.connector))
            {
                return Err(
                    "Windows reported the same monitor identity for different targets.".into(),
                );
            }
            targets.push(p.id.target);
            outputs.push(Output {
                connector: p.connector.clone(),
                name: p.name.clone(),
                enabled: !active.is_empty(),
                primary,
            });
        }
        Ok(outputs)
    }
    fn source(&self, id: Id) -> Result<&Source, String> {
        let sources: Vec<_> = self.sources.iter().filter(|s| s.id == id).collect();
        if sources.len() != 1 || sources[0].width == 0 || sources[0].height == 0 {
            return Err("Windows did not report a usable active source mode.".into());
        }
        Ok(sources[0])
    }
    pub fn plan(&self, connector: &str, change: Change) -> Result<Option<Plan>, String> {
        let outputs = self.outputs()?;
        let output = outputs
            .iter()
            .find(|o| o.connector.eq_ignore_ascii_case(connector))
            .ok_or("The selected monitor is no longer connected; refresh the output list.")?;
        if change == Change::Primary && !output.enabled {
            return Err("Attach this monitor before making it primary.".into());
        }
        if achieved(output, change) {
            return Ok(None);
        }
        if change == Change::Detach && outputs.iter().filter(|o| o.enabled).count() <= 1 {
            return Err("The last enabled screen cannot be detached.".into());
        }
        // Missing modes or disconnected active targets mean a racing/incomplete query.
        for p in self.paths.iter().filter(|p| p.active) {
            if !p.available {
                return Err("An active monitor disappeared; refresh the output list.".into());
            }
            self.source(p.id.source)?;
        }
        let candidates: Vec<_> = self
            .paths
            .iter()
            .filter(|p| p.available && p.connector.eq_ignore_ascii_case(connector))
            .collect();
        let chosen = if change == Change::Attach {
            candidates.iter().find(|p|!self.paths.iter().any(|other|other.active && other.id.source==p.id.source))
                .ok_or("No unused Windows source can attach this monitor without cloning or replacing another display.")?
        } else {
            candidates
                .iter()
                .find(|p| p.active)
                .ok_or("No active path for this monitor.")?
        };
        let mut plan = Plan {
            paths: self
                .paths
                .iter()
                .filter(|p| p.active)
                .map(|p| p.id)
                .collect(),
            activate: None,
            positions: vec![],
            allow_changes: change != Change::Primary,
        };
        match change {
            Change::Attach => {
                plan.paths.push(chosen.id);
                plan.activate = Some(chosen.id);
            }
            Change::Detach => {
                plan.paths.retain(|p| p.target != chosen.id.target);
            }
            Change::Primary => {
                // Put the selected path first as well as moving its source to the origin.
                plan.paths.retain(|p| *p != chosen.id);
                plan.paths.insert(0, chosen.id);
            }
        }
        let origin = if change == Change::Primary {
            Some(chosen.id.source)
        } else if change == Change::Detach && output.primary {
            plan.paths.first().map(|p| p.source)
        } else {
            None
        };
        if let Some(origin) = origin {
            let origin = self.source(origin)?;
            for source in &self.sources {
                if !plan.paths.iter().any(|p| p.source == source.id) {
                    continue;
                }
                let x = source
                    .x
                    .checked_sub(origin.x)
                    .ok_or("Desktop coordinates overflow after changing primary.")?;
                let y = source
                    .y
                    .checked_sub(origin.y)
                    .ok_or("Desktop coordinates overflow after changing primary.")?;
                plan.positions.push((source.id, x, y));
            }
        }
        Ok(Some(plan))
    }
}
fn achieved(output: &Output, change: Change) -> bool {
    match change {
        Change::Primary => output.enabled && output.primary,
        Change::Attach => output.enabled,
        Change::Detach => !output.enabled,
    }
}

/// `validate` writes nothing. `apply` submits one CCD configuration, without pending
/// registry writes. `query` refreshes the native data used to materialize the plan.
pub trait Backend {
    fn query(&mut self) -> Result<Configuration, String>;
    fn validate(&mut self, plan: &Plan) -> Result<(), String>;
    fn apply(&mut self, plan: &Plan) -> Result<(), String>;
}
pub fn change_checked_with(
    backend: &mut impl Backend,
    change: Change,
    connector: &str,
    signature: &str,
) -> Result<Configuration, String> {
    let before = backend.query()?;
    if before.signature() != signature {
        return Err("The displays changed; refresh the output list.".into());
    }
    let Some(plan) = before.plan(connector, change)? else {
        return Ok(before);
    };
    backend.validate(&plan)?;
    // Validation can run while the user docks or changes Windows display settings.
    if backend.query()?.signature() != signature {
        return Err("The displays changed during validation; refresh the output list.".into());
    }
    let applied = backend.apply(&plan);
    let reread = backend.query(); // Also refresh after a driver error: a partial change is possible.
    applied?;
    let after = reread?;
    let outputs = after.outputs()?;
    if !outputs
        .iter()
        .any(|o| o.connector.eq_ignore_ascii_case(connector) && achieved(o, change))
    {
        return Err(
            "Windows did not confirm the requested monitor state; refresh the output list.".into(),
        );
    }
    // Attaching a target must never silently replace another one; detaching it must
    // leave every other monitor in its previous enabled state.
    for old in before
        .outputs()?
        .iter()
        .filter(|o| !o.connector.eq_ignore_ascii_case(connector))
    {
        if !outputs
            .iter()
            .any(|o| o.enabled == old.enabled && o.connector.eq_ignore_ascii_case(&old.connector))
        {
            return Err("Another monitor changed unexpectedly; refresh the output list.".into());
        }
    }
    if change == Change::Primary {
        for (id, x, y) in plan.positions {
            let old = before.source(id)?;
            if !after.sources.iter().any(|s| {
                s.id == id
                    && s.x == x
                    && s.y == y
                    && s.width == old.width
                    && s.height == old.height
                    && s.pixel_format == old.pixel_format
            }) {
                return Err("Windows did not preserve the monitor modes and relative positions; refresh the output list.".into());
            }
        }
    }
    Ok(after)
}
#[cfg(windows)]
pub fn connected_outputs() -> Result<Configuration, String> {
    native::Native::default().query()
}
#[cfg(windows)]
pub fn change_checked(
    change: Change,
    connector: &str,
    signature: &str,
) -> Result<Configuration, String> {
    change_checked_with(&mut native::Native::default(), change, connector, signature)
}
