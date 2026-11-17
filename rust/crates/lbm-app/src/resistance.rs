//! The UI sends an edit against an exact side snapshot; only this adapter writes the layout.
/// Own edge ends already have a band boundary and need no line across the other panels.
pub fn overlay_reference(
    guide: lbm_ui::resistance::Guide,
) -> Option<lbm_overlay::scene::Reference> {
    use lbm_overlay::scene::{GuideKind, Reference};
    use lbm_ui::resistance::SnapKind;
    let kind = match guide.kind {
        SnapKind::EdgeEnd => return None,
        SnapKind::Middle => GuideKind::Middle,
        SnapKind::ScreenEdge => GuideKind::ScreenEdge,
        SnapKind::Section => GuideKind::Section,
    };
    Some(Reference {
        horizontal: guide.side.vertical(),
        layout_mm: guide.layout_mm,
        kind,
    })
}
use lbm_layout::model::{BorderSection, Layout};
use lbm_ui::resistance::{geometry, Action, Edit, Monitor, Selection};

pub fn monitors(layout: &Layout) -> Vec<Monitor> {
    layout
        .monitors()
        .iter()
        .filter_map(|m| {
            Some(Monitor {
                id: m.id.clone(),
                bounds: layout.depth_projection(m)?.bounds(),
                excluded: m
                    .active_source
                    .as_deref()
                    .and_then(|id| layout.source(id))
                    .is_none_or(|s| {
                        !s.source.attached_to_desktop
                            || (m.excluded_from_layout && !s.source.primary)
                    }),
                resistance: m.border_resistance.clone(),
            })
        })
        .collect()
}

pub fn apply(layout: &mut Layout, action: Action) -> Result<Option<Selection>, String> {
    if layout.is_virtual() {
        return Err("No physical display layout is available.".into());
    }
    let monitors = monitors(layout);
    let monitor = monitors
        .iter()
        .find(|m| m.id == action.monitor)
        .ok_or("Monitor no longer present.")?;
    let sections = action.side.sections(&monitor.resistance);
    if sections != action.expected {
        return Err("Sections changed; select the section again.".into());
    }
    let mut edited = sections.to_vec();
    let selection = |index| {
        Some(Selection {
            monitor: action.monitor.clone(),
            side: action.side,
            index,
        })
    };
    let selected = match action.edit {
        Edit::Insert(section) => {
            if sections.len() >= geometry::MAX_SECTIONS {
                return Err("An edge can have at most 16 sections.".into());
            }
            validate(section, sections, None, action.side.length(monitor.bounds))?;
            edited.push(section);
            selection(edited.len() - 1)
        }
        Edit::Replace { index, section } => {
            if index >= sections.len() {
                return Err("Section no longer present.".into());
            }
            validate(
                section,
                sections,
                Some(index),
                action.side.length(monitor.bounds),
            )?;
            edited[index] = section;
            selection(index)
        }
        Edit::Delete(index) => {
            if index >= sections.len() {
                return Err("Section no longer present.".into());
            }
            edited.remove(index);
            None
        }
        Edit::Mirror(index) => {
            let original = selection(index).unwrap();
            let (target, section) =
                geometry::mirror(&monitors, &original, layout.options.max_travel_distance)
                    .ok_or("No free interval on a reachable facing edge.")?;
            let monitor = monitors.iter().find(|m| m.id == target.monitor).unwrap();
            validate(
                section,
                target.side.sections(&monitor.resistance),
                None,
                target.side.length(monitor.bounds),
            )?;
            layout.edit_border_resistance(&target.monitor, |b| {
                target.side.sections_mut(b).push(section)
            });
            return Ok(Some(original));
        }
    };
    layout.edit_border_resistance(&action.monitor, |b| *action.side.sections_mut(b) = edited);
    Ok(selected)
}
fn validate(
    s: BorderSection,
    others: &[BorderSection],
    excluding: Option<usize>,
    length: f64,
) -> Result<(), String> {
    if !length.is_finite()
        || [s.from(), s.to(), s.move_resistance(), s.drag()]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        || s.to() > length
        || s.to() - s.from() < geometry::MIN_LENGTH
    {
        return Err(
            "Use finite non-negative values and a section of at least 2 mm within the edge.".into(),
        );
    }
    if others
        .iter()
        .enumerate()
        .any(|(i, o)| Some(i) != excluding && s.from() < o.to() && s.to() > o.from())
    {
        return Err("Sections must not overlap.".into());
    }
    Ok(())
}
