use super::{
    geometry::{self, Grab, Span},
    *,
};

fn thickness(content: egui::Rect) -> f32 {
    12.0_f32
        .min(content.width() / 3.0)
        .min(content.height() / 3.0)
        .max(0.0)
}
/// Bands lie inside the visible panel, matching the coordinates used by the engine.
/// Each one spans its whole edge, so neighbouring bands cross in the corners; who
/// answers there is [`owner`]'s call, not the drawing order.
pub fn band(content: egui::Rect, side: Side) -> egui::Rect {
    let thickness = thickness(content);
    match side {
        Side::Left => egui::Rect::from_min_max(
            content.min,
            egui::pos2(content.left() + thickness, content.bottom()),
        ),
        Side::Right => egui::Rect::from_min_max(
            egui::pos2(content.right() - thickness, content.top()),
            content.max,
        ),
        Side::Top => egui::Rect::from_min_max(
            content.min,
            egui::pos2(content.right(), content.top() + thickness),
        ),
        Side::Bottom => egui::Rect::from_min_max(
            egui::pos2(content.left(), content.bottom() - thickness),
            content.max,
        ),
    }
}
/// The squares where a vertical and a horizontal band cross.
const CORNERS: [(Side, Side); 4] = [
    (Side::Left, Side::Top),
    (Side::Right, Side::Top),
    (Side::Left, Side::Bottom),
    (Side::Right, Side::Bottom),
];
fn corner(content: egui::Rect, vertical: Side, horizontal: Side) -> egui::Rect {
    let t = thickness(content);
    let x = if vertical == Side::Left {
        content.left()
    } else {
        content.right() - t
    };
    let y = if horizontal == Side::Top {
        content.top()
    } else {
        content.bottom() - t
    };
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(t, t))
}
/// A band without its two corners: the part no other band covers.
fn middle(content: egui::Rect, side: Side) -> egui::Rect {
    let t = thickness(content);
    band(content, side).shrink2(if side.vertical() {
        egui::vec2(0.0, t)
    } else {
        egui::vec2(t, 0.0)
    })
}
/// Distance from the panel's outer edge on that side.
fn inset(content: egui::Rect, side: Side, at: egui::Pos2) -> f32 {
    match side {
        Side::Left => at.x - content.left(),
        Side::Right => content.right() - at.x,
        Side::Top => at.y - content.top(),
        Side::Bottom => content.bottom() - at.y,
    }
}
/// The band, its edge length in mm and its pixels per mm, when it can be drawn.
fn measure(content: egui::Rect, monitor: &Monitor, side: Side) -> Option<(egui::Rect, f64, f64)> {
    let length = side.length(monitor.bounds);
    let rect = band(content, side);
    let pixels = if side.vertical() {
        rect.height()
    } else {
        rect.width()
    };
    (length.is_finite() && length > 0.0 && pixels > 0.0)
        .then(|| (rect, length, f64::from(pixels) / length))
}
/// The edge that answers a pointer at `at`. Where two bands cross, the one with a
/// section (or a section's handle) there wins, so a short section at the end of an
/// edge stays reachable; otherwise, or when both have one, the corner is cut along
/// its diagonal and the nearer edge wins.
pub fn owner(content: egui::Rect, monitor: &Monitor, at: egui::Pos2) -> Option<Side> {
    let candidates: Vec<_> = Side::ALL
        .into_iter()
        .filter(|side| {
            measure(content, monitor, *side).is_some() && band(content, *side).contains(at)
        })
        .collect();
    let holds = |side: Side| {
        measure(content, monitor, side).is_some_and(|(rect, length, scale)| {
            geometry::grab(
                side.sections(&monitor.resistance),
                mm(at, rect, side, length),
                scale,
                length,
                f64::from(thickness(content)),
            )
            .index()
            .is_some()
        })
    };
    match candidates[..] {
        [] => None,
        [one] => Some(one),
        [a, b] => Some(match (holds(a), holds(b)) {
            (true, false) => a,
            (false, true) => b,
            _ => nearer(content, a, b, at),
        }),
        _ => None,
    }
}
fn nearer(content: egui::Rect, a: Side, b: Side, at: egui::Pos2) -> Side {
    let (ia, ib) = (inset(content, a, at), inset(content, b, at));
    if ia < ib || (ia == ib && a.vertical()) {
        a
    } else {
        b
    }
}
pub fn segment(band: egui::Rect, side: Side, span: Span, length: f64) -> egui::Rect {
    let a = (span.from / length).clamp(0.0, 1.0) as f32;
    let b = (span.to / length).clamp(0.0, 1.0) as f32;
    if side.vertical() {
        egui::Rect::from_min_max(
            egui::pos2(band.left(), band.top() + a * band.height()),
            egui::pos2(band.right(), band.top() + b * band.height()),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(band.left() + a * band.width(), band.top()),
            egui::pos2(band.left() + b * band.width(), band.bottom()),
        )
    }
}
fn mm(at: egui::Pos2, band: egui::Rect, side: Side, length: f64) -> f64 {
    if side.vertical() {
        f64::from(at.y - band.top()) / f64::from(band.height()) * length
    } else {
        f64::from(at.x - band.left()) / f64::from(band.width()) * length
    }
}
#[derive(Clone)]
pub(super) struct Gesture {
    monitor: String,
    side: Side,
    expected: Vec<BorderSection>,
    grab: Grab,
    press: f64,
    anchor: f64,
    original: BorderSection,
    preview: Option<BorderSection>,
    /// Resizing the window invalidates the pointer's millimetre conversion.
    band: egui::Rect,
    bounds: lbm_layout::geo::Rect,
    guide: Option<geometry::Target>,
}
impl Gesture {
    fn update(
        &mut self,
        raw: f64,
        length: f64,
        scale: f64,
        references: &[geometry::Target],
        snap: bool,
    ) {
        self.guide = None;
        let values: Vec<_> = references.iter().map(|t| t.mm).collect();
        let targets = values.as_slice();
        let index = self.grab.index();
        let reference = if index.is_some() {
            (self.original.from() + self.original.to()) / 2.0
        } else {
            self.anchor
        };
        let free = geometry::gap(&self.expected, index, reference, length);
        let boundary = |v| geometry::snap(v, length, 8.0 / scale, targets, snap);
        let wanted = match self.grab {
            Grab::Draw => geometry::clamp(Span::new(self.anchor, boundary(raw)), free),
            Grab::From(_) => geometry::clamp(
                Span::new(
                    boundary(self.original.from() + raw - self.press),
                    self.original.to(),
                ),
                free,
            ),
            Grab::To(_) => geometry::clamp(
                Span::new(
                    self.original.from(),
                    boundary(self.original.to() + raw - self.press),
                ),
                free,
            ),
            Grab::Move(_) => {
                let from = self.original.from() + raw - self.press;
                let from = if snap {
                    geometry::snap_move(
                        from,
                        self.original.to() - self.original.from(),
                        8.0 / scale,
                        targets,
                    )
                } else {
                    from
                };
                geometry::shifted(Span::from(&self.original), from, free)
            }
        };
        // A sliver is not a new section, and an invalid resize retains the last valid span.
        if wanted.is_some() || self.grab == Grab::Draw {
            self.preview = wanted.map(|s| s.section(self.original));
        }
        // Only an actually applied boundary can explain the snap. Do not mark the fixed
        // end of a resize/draw, or the retained old preview after an invalid resize.
        if snap {
            if let Some(span) = wanted {
                let candidates = match self.grab {
                    Grab::Draw => [
                        if raw < self.anchor {
                            span.from
                        } else {
                            span.to
                        },
                        f64::NAN,
                    ],
                    Grab::From(_) => [
                        if self.original.from() + raw - self.press <= self.original.to() {
                            span.from
                        } else {
                            span.to
                        },
                        f64::NAN,
                    ],
                    Grab::To(_) => [
                        if self.original.to() + raw - self.press >= self.original.from() {
                            span.to
                        } else {
                            span.from
                        },
                        f64::NAN,
                    ],
                    Grab::Move(_) => [span.from, span.to],
                };
                self.guide = candidates.iter().find_map(|mm| {
                    references
                        .iter()
                        .find(|t| (t.mm - mm).abs() < 0.001)
                        .copied()
                });
            }
        }
    }
    fn action(&self) -> Option<Action> {
        let section = self.preview?;
        let edit = if let Some(index) = self.grab.index() {
            if section == self.original {
                return None;
            }
            Edit::Replace { index, section }
        } else {
            if self.expected.len() >= geometry::MAX_SECTIONS {
                return None;
            }
            Edit::Insert(section)
        };
        Some(Action {
            monitor: self.monitor.clone(),
            side: self.side,
            expected: self.expected.clone(),
            edit,
        })
    }
}
#[derive(Default)]
pub struct Interaction {
    pub monitor: Option<String>,
    pub action: Option<Action>,
}
impl Editor {
    /// Corners are sensed apart, then handed to the edge that owns the pointer there,
    /// so no band hides another one's end. A gesture keeps its edge until release.
    fn band_responses(
        &self,
        ui: &egui::Ui,
        id: &str,
        monitor: &Monitor,
        content: egui::Rect,
    ) -> Vec<(Side, egui::Response)> {
        let mut responses: Vec<(Side, egui::Response)> = Side::ALL
            .into_iter()
            .filter(|side| measure(content, monitor, *side).is_some())
            .map(|side| {
                let response = ui.interact(
                    middle(content, side),
                    ui.id().with(("resistance-band", id, side)),
                    egui::Sense::click_and_drag(),
                );
                (side, response)
            })
            .collect();
        for (vertical, horizontal) in CORNERS {
            let response = ui.interact(
                corner(content, vertical, horizontal),
                ui.id()
                    .with(("resistance-corner", id, vertical, horizontal)),
                egui::Sense::click_and_drag(),
            );
            let held = self
                .gesture
                .as_ref()
                .filter(|g| {
                    g.monitor == id
                        && (g.side == vertical || g.side == horizontal)
                        && (response.dragged() || response.drag_stopped())
                })
                .map(|g| g.side);
            let at = if response.drag_started() {
                ui.input(|i| i.pointer.press_origin())
            } else {
                response
                    .interact_pointer_pos()
                    .or_else(|| response.hover_pos())
            };
            let Some(side) = held.or_else(|| at.and_then(|at| owner(content, monitor, at))) else {
                continue;
            };
            if let Some((_, owned)) = responses.iter_mut().find(|(s, _)| *s == side) {
                *owned = owned.union(response);
            }
        }
        responses
    }
    /// The uncommitted gesture, for drawing on a copy of the layout on real screens.
    pub fn preview_action(&self) -> Option<Action> {
        self.gesture.as_ref().and_then(Gesture::action)
    }
    pub fn guide(&self) -> Option<Guide> {
        let g = self.gesture.as_ref()?;
        let target = g.guide?;
        Some(Guide {
            monitor: g.monitor.clone(),
            side: g.side,
            mm: target.mm,
            layout_mm: g.side.origin(g.bounds) + target.mm,
            kind: target.kind,
        })
    }
    /// Preview stays local until release. Escape, focus loss, a changed layout or a
    /// changed projection discard it, so cancelled gestures never reach Save/Live.
    pub fn bands(
        &mut self,
        ui: &mut egui::Ui,
        monitors: &[Monitor],
        placements: &[(String, egui::Rect)],
    ) -> Interaction {
        let mut result = Interaction::default();
        if !ui.is_enabled() || ui.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused) {
            self.cancel();
        }
        if let Some(g) = &self.gesture {
            let valid = monitors
                .iter()
                .find(|m| m.id == g.monitor)
                .is_some_and(|m| {
                    g.side.sections(&m.resistance) == g.expected && m.bounds == g.bounds
                })
                && placements
                    .iter()
                    .find(|(id, _)| *id == g.monitor)
                    .is_some_and(|(_, r)| band(*r, g.side) == g.band);
            if !valid {
                self.cancel();
            }
        }
        for (id, content) in placements {
            let Some(monitor) = monitors.iter().find(|m| m.id == *id) else {
                continue;
            };
            let mut drawn = Vec::new();
            for (side, response) in self.band_responses(ui, id, monitor, *content) {
                let Some((rect, length, scale)) = measure(*content, monitor, side) else {
                    continue;
                };
                let thickness = f64::from(thickness(*content));
                let sections = side.sections(&monitor.resistance);
                let snap = !ui.input(|i| i.modifiers.ctrl);
                if response.clicked() || response.double_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let at = mm(pos, rect, side, length);
                        let grab = geometry::grab(sections, at, scale, length, thickness);
                        self.selected = grab.index().map(|index| Selection {
                            monitor: id.clone(),
                            side,
                            index,
                        });
                        self.side = side;
                        result.monitor = Some(id.clone());
                        if response.double_clicked() {
                            let span = geometry::gap(sections, grab.index(), at, length);
                            if span.length() >= geometry::MIN_LENGTH {
                                let edit = if let Some(index) = grab.index() {
                                    Edit::Replace {
                                        index,
                                        section: span.section(sections[index]),
                                    }
                                } else {
                                    Edit::Insert(span.section(super::new_section()))
                                };
                                result.action = Some(Action::new(monitor, side, edit));
                            }
                        }
                    }
                }
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                        let raw = mm(press, rect, side, length);
                        let grab = geometry::grab(sections, raw, scale, length, thickness);
                        let targets = geometry::targets(monitors, id, side, grab.index());
                        let original = grab
                            .index()
                            .map(|i| sections[i])
                            .unwrap_or_else(super::new_section);
                        self.selected = grab.index().map(|index| Selection {
                            monitor: id.clone(),
                            side,
                            index,
                        });
                        self.side = side;
                        result.monitor = Some(id.clone());
                        self.gesture = Some(Gesture {
                            monitor: id.clone(),
                            side,
                            expected: sections.to_vec(),
                            grab,
                            press: raw,
                            anchor: geometry::snap(raw, length, 8.0 / scale, &targets, snap),
                            original,
                            preview: grab.index().map(|_| original),
                            band: rect,
                            bounds: monitor.bounds,
                            guide: None,
                        });
                    }
                }
                let mut preview = None;
                let mut replacing = None;
                if let Some(g) = self
                    .gesture
                    .as_mut()
                    .filter(|g| g.monitor == *id && g.side == side)
                {
                    let targets = geometry::references(monitors, id, side, g.grab.index());
                    g.guide = None;
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        g.update(mm(pos, rect, side, length), length, scale, &targets, snap);
                    }
                    preview = g.preview;
                    replacing = g.grab.index();
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        result.action = g.action();
                    }
                }
                let mut shown: Vec<_> = sections
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| replacing != Some(*index))
                    .map(|(index, s)| {
                        let selected = self.selected.as_ref().is_some_and(|k| {
                            k.monitor == *id && k.side == side && k.index == index
                        });
                        (s.resolved(self.defaults), selected)
                    })
                    .collect();
                shown.extend(preview.map(|s| (s.resolved(self.defaults), true)));
                drawn.push(Drawn {
                    side,
                    rect,
                    length,
                    scale,
                    shown,
                });
                if let Some(at) = response.hover_pos() {
                    let value = mm(at, rect, side, length);
                    let tip = geometry::grab(sections, value, scale, length, thickness)
                        .index()
                        .map(|i| {
                            format!(
                                "{} · {:.1}–{:.1} mm\n{}",
                                side.label(),
                                sections[i].from(),
                                sections[i].to(),
                                description(&sections[i].resolved(self.defaults))
                            )
                        })
                        .unwrap_or_else(|| format!("{} · drag to draw a section", side.label()));
                    response.on_hover_text(tip);
                }
            }
            paint(ui, &drawn);
        }
        if ui.input(|i| !i.pointer.primary_down()) {
            self.cancel();
        }
        if let Some(guide) = self.guide() {
            draw_guide(ui, &guide, monitors, placements);
        }
        if ui.is_enabled()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
        {
            if let Some(s) = &self.selected {
                if let Some(m) = monitors.iter().find(|m| m.id == s.monitor) {
                    result.action = Some(Action::new(m, s.side, Edit::Delete(s.index)));
                }
            }
        }
        if result.action.is_none() && !self.active() {
            result.action = self.applet(ui, monitors, placements);
        }
        result
    }
}
fn draw_guide(
    ui: &egui::Ui,
    guide: &Guide,
    monitors: &[Monitor],
    placements: &[(String, egui::Rect)],
) {
    for (id, rect) in placements {
        let Some(m) = monitors.iter().find(|m| m.id == *id) else {
            continue;
        };
        let length = guide.side.length(m.bounds);
        let local = guide.layout_mm - guide.side.origin(m.bounds);
        if !length.is_finite() || length <= 0. || !(0.0..=length).contains(&local) {
            continue;
        }
        let area = if guide.kind == SnapKind::EdgeEnd {
            if *id != guide.monitor {
                continue;
            }
            band(*rect, guide.side)
        } else {
            *rect
        };
        let position = (local / length) as f32;
        let line = if guide.side.vertical() {
            let y = rect.top() + position * rect.height();
            [egui::pos2(area.left(), y), egui::pos2(area.right(), y)]
        } else {
            let x = rect.left() + position * rect.width();
            [egui::pos2(x, area.top()), egui::pos2(x, area.bottom())]
        };
        ui.painter()
            .with_clip_rect(ui.clip_rect().intersect(*rect))
            .extend(egui::Shape::dashed_line(
                &line,
                egui::Stroke::new(1.5, guide.kind.color()),
                4.,
                4.,
            ));
    }
}
struct Drawn {
    side: Side,
    rect: egui::Rect,
    length: f64,
    scale: f64,
    /// The sections as shown: a gesture's preview replaces the section it edits.
    shown: Vec<(BorderSection, bool)>,
}
impl Drawn {
    fn segment(&self, s: &BorderSection) -> egui::Rect {
        segment(self.rect, self.side, Span::from(s), self.length)
    }
}
/// Which ends of `s` meet a section of the crossing edge. Only those are cut along the
/// diagonal: alone in a corner, a section is what [`owner`] answers for in all of it,
/// and a short one would otherwise shrink to a sliver of its own triangle.
fn mitres(d: &Drawn, s: &BorderSection, drawn: &[Drawn]) -> (bool, bool) {
    let [start, end] = if d.side.vertical() {
        [Side::Top, Side::Bottom]
    } else {
        [Side::Left, Side::Right]
    };
    let rect = d.segment(s);
    let meets = |crossing: Side| {
        drawn.iter().filter(|o| o.side == crossing).any(|o| {
            o.shown
                .iter()
                .any(|(other, _)| rect.intersect(o.segment(other)).is_positive())
        })
    };
    (meets(start), meets(end))
}
/// Backgrounds first, then every edge's sections, so no band covers another's.
fn paint(ui: &egui::Ui, drawn: &[Drawn]) {
    for d in drawn {
        ui.painter().add(egui::Shape::convex_polygon(
            super::shape::outline(d.rect, d.side, true, true).to_vec(),
            egui::Color32::from_black_alpha(24),
            egui::Stroke::NONE,
        ));
    }
    for d in drawn {
        for (s, selected) in &d.shown {
            let (start, end) = mitres(d, s, drawn);
            let outline = super::shape::outline(d.rect, d.side, start, end);
            paint_section(ui, d, s, *selected, &outline);
        }
    }
}
fn paint_section(
    ui: &egui::Ui,
    d: &Drawn,
    s: &BorderSection,
    selected: bool,
    outline: &[egui::Pos2; 4],
) {
    super::shape::rounded(
        ui.painter(),
        d.segment(s),
        outline,
        color(s).gamma_multiply(0.75),
        if selected {
            egui::Stroke::new(2.0, egui::Color32::WHITE)
        } else {
            egui::Stroke::NONE
        },
    );
    let grip = |boundary| {
        geometry::grip(
            boundary,
            s.to() - s.from(),
            d.scale,
            d.length,
            f64::from(d.rect.width().min(d.rect.height())),
        )
    };
    for span in [
        Span {
            from: s.from(),
            to: s.from() + grip(s.from()),
        },
        Span {
            from: s.to() - grip(s.to()),
            to: s.to(),
        },
    ] {
        super::shape::rounded(
            ui.painter(),
            segment(d.rect, d.side, span, d.length),
            outline,
            egui::Color32::from_white_alpha(208),
            egui::Stroke::NONE,
        );
    }
}
