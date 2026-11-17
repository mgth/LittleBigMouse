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
    let b = band(content, side);
    if side.vertical() {
        egui::Rect::from_min_max(
            egui::pos2(b.left(), b.top() + t),
            egui::pos2(b.right(), b.bottom() - t),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(b.left() + t, b.top()),
            egui::pos2(b.right() - t, b.bottom()),
        )
    }
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
    length: f64,
}
impl Gesture {
    fn update(&mut self, raw: f64, length: f64, scale: f64, targets: &[f64], snap: bool) {
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
    /// The uncommitted gesture, for drawing on a copy of the layout on real screens.
    pub fn preview_action(&self) -> Option<Action> {
        self.gesture.as_ref().and_then(Gesture::action)
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
                    g.side.sections(&m.resistance) == g.expected
                        && g.side.length(m.bounds) == g.length
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
            let content = *content;
            // Corners are sensed apart, then handed to the edge that owns the pointer
            // there, so no band hides another one's end.
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
                // A gesture keeps the edge it started on until release.
                let held = self
                    .gesture
                    .as_ref()
                    .filter(|g| {
                        g.monitor == *id
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
                let Some(side) = held.or_else(|| at.and_then(|at| owner(content, monitor, at)))
                else {
                    continue;
                };
                if let Some((_, owned)) = responses.iter_mut().find(|(s, _)| *s == side) {
                    *owned = owned.union(response);
                }
            }
            let mut drawn = Vec::new();
            let mut lines = Vec::new();
            for (side, response) in responses {
                let Some((rect, length, scale)) = measure(content, monitor, side) else {
                    continue;
                };
                let sections = side.sections(&monitor.resistance);
                let snap = !ui.input(|i| i.modifiers.ctrl);
                if response.clicked() || response.double_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let at = mm(pos, rect, side, length);
                        let grab = geometry::grab(sections, at, scale);
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
                                    Edit::Insert(span.section(BorderSection::new(
                                        0.0, 0.0, 0.0, true, 0.0, false,
                                    )))
                                };
                                result.action = Some(Action::new(monitor, side, edit));
                            }
                        }
                    }
                }
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                        let raw = mm(press, rect, side, length);
                        let grab = geometry::grab(sections, raw, scale);
                        let targets = geometry::targets(monitors, id, side, grab.index());
                        let original = grab
                            .index()
                            .map(|i| sections[i])
                            .unwrap_or_else(|| BorderSection::new(0.0, 0.0, 0.0, true, 0.0, false));
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
                            length,
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
                    let targets = geometry::targets(monitors, id, side, g.grab.index());
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        g.update(mm(pos, rect, side, length), length, scale, &targets, snap);
                    }
                    preview = g.preview;
                    replacing = g.grab.index();
                    if snap {
                        if let Some(s) = preview {
                            for boundary in [s.from(), s.to()] {
                                if targets.iter().any(|v| (*v - boundary).abs() < 0.001) {
                                    let r = segment(
                                        rect,
                                        side,
                                        Span {
                                            from: boundary,
                                            to: boundary,
                                        },
                                        length,
                                    );
                                    let area = ui.max_rect();
                                    lines.push(if side.vertical() {
                                        [
                                            egui::pos2(area.left(), r.top()),
                                            egui::pos2(area.right(), r.top()),
                                        ]
                                    } else {
                                        [
                                            egui::pos2(r.left(), area.top()),
                                            egui::pos2(r.left(), area.bottom()),
                                        ]
                                    });
                                }
                            }
                        }
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        result.action = g.action();
                    }
                }
                let mut shown: Vec<(BorderSection, bool)> = sections
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| replacing != Some(*index))
                    .map(|(index, s)| {
                        let selected = self.selected.as_ref().is_some_and(|k| {
                            k.monitor == *id && k.side == side && k.index == index
                        });
                        (*s, selected)
                    })
                    .collect();
                shown.extend(preview.map(|s| (s, true)));
                if let Some(at) = response.hover_pos() {
                    let value = mm(at, rect, side, length);
                    let tip = geometry::grab(sections, value, scale)
                        .index()
                        .map(|i| {
                            format!(
                                "{} · {:.1}–{:.1} mm\n{}",
                                side.label(),
                                sections[i].from(),
                                sections[i].to(),
                                description(&sections[i])
                            )
                        })
                        .unwrap_or_else(|| format!("{} · drag to draw a section", side.label()));
                    response.on_hover_text(tip);
                }
                drawn.push(Drawn {
                    side,
                    rect,
                    length,
                    scale,
                    shown,
                });
            }
            paint(ui, content, &drawn);
            for line in lines {
                ui.painter()
                    .line_segment(line, egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE));
            }
        }
        if ui.input(|i| !i.pointer.primary_down()) {
            self.cancel();
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
        result
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
/// Backgrounds first, then every edge's sections, so no band covers another's.
/// Where sections of two edges meet in a corner, each keeps the half [`owner`]
/// gives its edge.
fn paint(ui: &egui::Ui, content: egui::Rect, drawn: &[Drawn]) {
    let painter = ui.painter();
    for d in drawn {
        painter.rect_filled(d.rect, 0.0, egui::Color32::from_gray(50));
    }
    let drawn_rect =
        |d: &Drawn, s: &BorderSection| segment(d.rect, d.side, Span::from(s), d.length);
    for d in drawn {
        for (s, _) in &d.shown {
            painter.rect_filled(drawn_rect(d, s), 0.0, color(s));
        }
    }
    for (vertical, horizontal) in CORNERS {
        let find = |side| drawn.iter().find(|d| d.side == side);
        let (Some(v), Some(h)) = (find(vertical), find(horizontal)) else {
            continue;
        };
        for (sv, _) in &v.shown {
            for (sh, _) in &h.shown {
                let shared = drawn_rect(v, sv).intersect(drawn_rect(h, sh));
                if !shared.is_positive() {
                    continue;
                }
                for (side, other, s) in [(vertical, horizontal, sv), (horizontal, vertical, sh)] {
                    let half = clip(&shared, |p| {
                        inset(content, other, p) - inset(content, side, p)
                    });
                    if half.len() >= 3 {
                        painter.add(egui::Shape::convex_polygon(
                            half,
                            color(s),
                            egui::Stroke::NONE,
                        ));
                    }
                }
            }
        }
    }
    for d in drawn {
        for (s, selected) in &d.shown {
            let rect = drawn_rect(d, s);
            if *selected {
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
            let grip = geometry::grip(s, d.scale);
            for span in [
                Span {
                    from: s.from(),
                    to: s.from() + grip,
                },
                Span {
                    from: s.to() - grip,
                    to: s.to(),
                },
            ] {
                painter.rect_filled(
                    segment(d.rect, d.side, span, d.length).shrink(2.0),
                    0.0,
                    egui::Color32::from_white_alpha(100),
                );
            }
        }
    }
}
/// The part of `rect` where `f` is not negative (`f` linear: a half-plane).
fn clip(rect: &egui::Rect, f: impl Fn(egui::Pos2) -> f32) -> Vec<egui::Pos2> {
    let points = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let mut out = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        let (fp, fq) = (f(*p), f(q));
        if fp >= 0.0 {
            out.push(*p);
        }
        if (fp >= 0.0) != (fq >= 0.0) {
            out.push(*p + (q - *p) * (fp / (fp - fq)));
        }
    }
    out
}
