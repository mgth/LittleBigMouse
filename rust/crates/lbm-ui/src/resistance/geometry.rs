//! Interval rules shared by the band gestures, numeric editor and layout adapter.
use super::{Monitor, Selection, Side};
use lbm_layout::model::BorderSection;

pub const MIN_LENGTH: f64 = 2.0;
pub const MAX_SECTIONS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub from: f64,
    pub to: f64,
}
impl Span {
    pub fn new(a: f64, b: f64) -> Self {
        Self {
            from: a.min(b),
            to: a.max(b),
        }
    }
    pub fn length(self) -> f64 {
        self.to - self.from
    }
    pub fn section(self, mut section: BorderSection) -> BorderSection {
        section.set_from(self.from);
        section.set_to(self.to);
        section
    }
}
impl From<&BorderSection> for Span {
    fn from(s: &BorderSection) -> Self {
        Self {
            from: s.from(),
            to: s.to(),
        }
    }
}

pub fn gap(sections: &[BorderSection], excluding: Option<usize>, at: f64, length: f64) -> Span {
    let mut free = Span {
        from: 0.0,
        to: length,
    };
    for (i, s) in sections.iter().enumerate() {
        if Some(i) == excluding {
            continue;
        }
        if s.from() < at && s.to() > at {
            return Span { from: at, to: at };
        }
        if s.to() <= at {
            free.from = free.from.max(s.to());
        }
        if s.from() >= at {
            free.to = free.to.min(s.from());
        }
    }
    free
}

pub fn clamp(wanted: Span, free: Span) -> Option<Span> {
    let result = Span {
        from: wanted.from.max(free.from),
        to: wanted.to.min(free.to),
    };
    (result.length() >= MIN_LENGTH).then_some(result)
}

pub fn largest(sections: &[BorderSection], wanted: Span) -> Option<Span> {
    let mut sorted: Vec<_> = sections.iter().collect();
    sorted.sort_by(|a, b| a.from().total_cmp(&b.from()));
    let mut cursor = wanted.from;
    let mut best = Span {
        from: cursor,
        to: cursor,
    };
    for s in sorted {
        if s.to() <= wanted.from || s.from() >= wanted.to {
            continue;
        }
        if s.from() - cursor > best.length() {
            best = Span {
                from: cursor,
                to: s.from().min(wanted.to),
            };
        }
        cursor = cursor.max(s.to());
    }
    if wanted.to - cursor > best.length() {
        best = Span {
            from: cursor,
            to: wanted.to,
        };
    }
    (best.length() >= MIN_LENGTH).then_some(best)
}

pub fn shifted(current: Span, wanted: f64, free: Span) -> Option<Span> {
    if free.length() < current.length() {
        return None;
    }
    let from = wanted.clamp(free.from, free.to - current.length());
    Some(Span {
        from,
        to: from + current.length(),
    })
}

/// Containment is half-open: touching sections each retain their own near handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grab {
    Draw,
    From(usize),
    To(usize),
    Move(usize),
}
impl Grab {
    pub fn index(self) -> Option<usize> {
        match self {
            Self::Draw => None,
            Self::From(i) | Self::To(i) | Self::Move(i) => Some(i),
        }
    }
}
/// The visible cap and its hit target share the same size, including the mitre.
pub fn grip(boundary: f64, section_length: f64, scale: f64, length: f64, thickness: f64) -> f64 {
    let basic = 6.0_f64.max(thickness / 2.0) / scale;
    let from_end = boundary.min(length - boundary);
    basic
        .max(thickness / scale - from_end)
        .min(section_length / 3.0)
}
pub fn grab(sections: &[BorderSection], at: f64, scale: f64, length: f64, thickness: f64) -> Grab {
    let handle =
        |s: &BorderSection, boundary| grip(boundary, s.to() - s.from(), scale, length, thickness);
    let containing = sections.iter().position(|s| at >= s.from() && at < s.to());
    let near = || {
        sections
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let boundary = if at < s.from() { s.from() } else { s.to() };
                let d = (at - boundary).abs();
                (d <= handle(s, boundary)).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let Some(i) = containing.or_else(near) else {
        return Grab::Draw;
    };
    let s = &sections[i];
    if at <= s.from() + handle(s, s.from()) {
        Grab::From(i)
    } else if at >= s.to() - handle(s, s.to()) {
        Grab::To(i)
    } else {
        Grab::Move(i)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    EdgeEnd,
    Middle,
    ScreenEdge,
    Section,
}
impl SnapKind {
    pub fn color(self) -> egui::Color32 {
        match self {
            Self::EdgeEnd | Self::Middle => egui::Color32::WHITE,
            Self::ScreenEdge => egui::Color32::from_rgb(0x33, 0xc4, 0xd6),
            Self::Section => egui::Color32::from_rgb(0xe8, 0xc3, 0x3a),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub mm: f64,
    pub kind: SnapKind,
}

/// Stable precedence for coincident references: own geometry, other screens, then sections.
pub fn references(
    monitors: &[Monitor],
    id: &str,
    side: Side,
    excluding: Option<usize>,
) -> Vec<Target> {
    let Some(me) = monitors.iter().find(|m| m.id == id) else {
        return vec![];
    };
    let length = side.length(me.bounds);
    let origin = side.origin(me.bounds);
    let mut targets = vec![
        Target {
            mm: 0.,
            kind: SnapKind::EdgeEnd,
        },
        Target {
            mm: length,
            kind: SnapKind::EdgeEnd,
        },
        Target {
            mm: length / 2.,
            kind: SnapKind::Middle,
        },
    ];
    let mut add = |mm: f64, kind| {
        if mm.is_finite()
            && mm >= 0.
            && mm <= length
            && !targets.iter().any(|t| (t.mm - mm).abs() < 0.001)
        {
            targets.push(Target { mm, kind });
        }
    };
    for m in monitors {
        if m.id != id {
            add(side.origin(m.bounds) - origin, SnapKind::ScreenEdge);
            add(
                side.origin(m.bounds) + side.length(m.bounds) - origin,
                SnapKind::ScreenEdge,
            );
        }
    }
    for m in monitors {
        for parallel in Side::ALL
            .into_iter()
            .filter(|s| s.vertical() == side.vertical())
        {
            for (i, s) in parallel.sections(&m.resistance).iter().enumerate() {
                if m.id == id && parallel == side && Some(i) == excluding {
                    continue;
                }
                let offset = parallel.origin(m.bounds) - origin;
                add(offset + s.from(), SnapKind::Section);
                add(offset + s.to(), SnapKind::Section);
            }
        }
    }
    targets
}
pub fn targets(monitors: &[Monitor], id: &str, side: Side, excluding: Option<usize>) -> Vec<f64> {
    references(monitors, id, side, excluding)
        .iter()
        .map(|t| t.mm)
        .collect()
}
pub fn snap(value: f64, length: f64, tolerance: f64, targets: &[f64], enabled: bool) -> f64 {
    let value = value.clamp(0.0, length);
    if !enabled {
        return value;
    }
    targets
        .iter()
        .copied()
        .filter(|t| (*t - value).abs() < tolerance)
        .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
        .unwrap_or(value)
}
pub fn snap_move(from: f64, length: f64, tolerance: f64, targets: &[f64]) -> f64 {
    targets
        .iter()
        .flat_map(|t| [*t, *t - length])
        .filter(|t| (*t - from).abs() <= tolerance)
        .min_by(|a, b| (a - from).abs().total_cmp(&(b - from).abs()))
        .unwrap_or(from)
}

/// Same nearest-facing-edge rule as Avalonia, including bezel gaps and travel limit.
pub fn mirror(
    monitors: &[Monitor],
    selected: &Selection,
    travel: f64,
) -> Option<(Selection, BorderSection)> {
    let source = monitors.iter().find(|m| m.id == selected.monitor)?;
    let section = selected
        .side
        .sections(&source.resistance)
        .get(selected.index)?;
    let side = selected.side;
    let target = monitors
        .iter()
        .filter(|m| m.id != source.id && !m.excluded)
        .filter_map(|m| {
            let a = source.bounds;
            let b = m.bounds;
            let distance = match side {
                Side::Left => a.left() - b.right(),
                Side::Right => b.left() - a.right(),
                Side::Top => a.top() - b.bottom(),
                Side::Bottom => b.top() - a.bottom(),
            };
            let overlap = side.origin(b) < side.origin(a) + side.length(a)
                && side.origin(b) + side.length(b) > side.origin(a);
            (distance >= 0.0 && distance <= travel && overlap).then_some((m, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))?
        .0;
    let opposite = side.opposite();
    let sections = opposite.sections(&target.resistance);
    if sections.len() >= MAX_SECTIONS {
        return None;
    }
    let offset = side.origin(source.bounds) - side.origin(target.bounds);
    let length = side.length(target.bounds);
    let wanted = Span {
        from: (offset + section.from()).clamp(0.0, length),
        to: (offset + section.to()).clamp(0.0, length),
    };
    let free = largest(sections, wanted)?;
    Some((
        Selection {
            monitor: target.id.clone(),
            side: opposite,
            index: sections.len(),
        },
        free.section(*section),
    ))
}

pub fn fully_blocked(sections: &[BorderSection], length: f64) -> bool {
    if length <= 0.0 {
        return false;
    }
    let mut sorted: Vec<_> = sections.iter().collect();
    sorted.sort_by(|a, b| a.from().total_cmp(&b.from()));
    let mut covered: f64 = 0.0;
    for s in sorted {
        if !s.move_block() || !s.drag_block() || s.from() > covered + 0.001 {
            return false;
        }
        covered = covered.max(s.to());
    }
    covered >= length - 0.001
}
