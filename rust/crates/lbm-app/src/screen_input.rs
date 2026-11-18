//! Real-screen gestures edit the same local document as the map. A drag commits on release.
use lbm_layout::{geo::Vector, model::Layout};
use lbm_overlay::{
    input::{Input, Kind},
    scene::Content,
};
use lbm_ui::resistance::{Editor, ExternalPhase, ExternalPointer, Side};

struct RulerDrag {
    press: Input,
    original: Layout,
    preview: Layout,
}
#[derive(Default)]
pub struct Controller {
    ruler: Option<RulerDrag>,
    press: Option<Input>,
    click: Option<Input>,
}
#[derive(Default)]
pub struct Outcome {
    pub changed: bool,
    pub selected: Option<String>,
    pub hide: bool,
}
impl Controller {
    pub fn active(&self) -> bool {
        self.ruler.is_some()
    }
    pub fn cancel(&mut self, editor: &mut Editor) {
        self.ruler = None;
        self.press = None;
        self.click = None;
        if editor.external_active() {
            editor.cancel();
        }
    }
    pub fn preview(&self) -> Option<&Layout> {
        self.ruler.as_ref().map(|g| &g.preview)
    }
    pub fn handle(
        &mut self,
        layout: &mut Layout,
        editor: &mut Editor,
        input: Input,
    ) -> Result<Outcome, String> {
        let mut out = Outcome::default();
        if !input.valid() || layout.is_virtual() {
            return Ok(out);
        }
        if matches!(input.kind, Kind::Cancel | Kind::Escape) {
            out.hide = input.kind == Kind::Escape && !self.active() && !editor.external_active();
            self.cancel(editor);
            return Ok(out);
        }
        if self.ruler.as_ref().is_some_and(|g| {
            lbm_store::LayoutDocument::of(&g.original) != lbm_store::LayoutDocument::of(layout)
        }) {
            self.cancel(editor);
            return Ok(out);
        }
        if matches!(input.band.content, Content::Ruler { .. }) {
            match input.kind {
                Kind::Press => {
                    self.cancel(editor);
                    self.ruler = Some(RulerDrag {
                        press: input,
                        original: layout.clone(),
                        preview: layout.clone(),
                    });
                }
                Kind::Move | Kind::Release => {
                    if let Some(g) = &mut self.ruler {
                        if g.press.band.monitor != input.band.monitor
                            || g.press.band.side != input.band.side
                        {
                            self.cancel(editor);
                            return Ok(out);
                        }
                        g.preview = g.original.clone();
                        let delta = -(input.at - g.press.at) * g.press.band.axis_mm
                            / (g.press.length - 16.5).max(1.);
                        let by = if input.band.side.vertical() {
                            Vector::new(0., delta)
                        } else {
                            Vector::new(delta, 0.)
                        };
                        g.preview.offset_location(&input.band.monitor, by);
                        if input.kind == Kind::Release {
                            *layout = g.preview.clone();
                            self.ruler = None;
                            out.changed = true;
                        }
                    }
                }
                Kind::Wheel(delta) => {
                    self.cancel(editor);
                    let monitor = layout
                        .monitor(&input.band.monitor)
                        .ok_or("Monitor disappeared")?;
                    let mut ratio = monitor.depth_ratio;
                    let bounds = layout
                        .depth_projection(monitor)
                        .ok_or("No monitor dimensions")?
                        .bounds();
                    let step = delta.signum()
                        * 0.01
                        * if input.ctrl { 0.1 } else { 1. }
                        * if input.shift { 10. } else { 1. };
                    if input.band.side.vertical() {
                        ratio.y = (ratio.y + step).clamp(0.05, 20.);
                    } else {
                        ratio.x = (ratio.x + step).clamp(0.05, 20.);
                    }
                    layout.set_depth_ratio(&input.band.monitor, ratio);
                    let resized = layout
                        .depth_projection(layout.monitor(&input.band.monitor).unwrap())
                        .unwrap()
                        .bounds();
                    let fraction = input.at / (input.length - 16.5).max(1.);
                    let by = if input.band.side.vertical() {
                        Vector::new(0., (bounds.height() - resized.height()) * fraction)
                    } else {
                        Vector::new((bounds.width() - resized.width()) * fraction, 0.)
                    };
                    layout.offset_location(&input.band.monitor, by);
                    out.changed = true;
                }
                _ => {}
            }
            return Ok(out);
        }
        let side = side(input.band.side);
        let mut phase = match input.kind {
            Kind::Press => ExternalPhase::Press,
            Kind::Move => ExternalPhase::Move,
            Kind::Release => ExternalPhase::Release,
            Kind::Delete => ExternalPhase::Delete,
            _ => return Ok(out),
        };
        if input.kind == Kind::Press {
            self.press = Some(input.clone());
            out.selected = Some(input.band.monitor.clone());
        }
        if input.kind == Kind::Release {
            let click = self.press.take().is_some_and(|p| {
                (input.at - p.at).abs() < 4. && input.time.wrapping_sub(p.time) < 500
            });
            if click {
                if self.click.as_ref().is_some_and(|p| {
                    p.band.monitor == input.band.monitor
                        && p.band.side == input.band.side
                        && input.time.wrapping_sub(p.time) < 400
                        && (input.at - p.at).abs() < 4.
                }) {
                    phase = ExternalPhase::Expand;
                    self.click = None;
                } else {
                    self.click = Some(input.clone());
                }
            } else {
                self.click = None;
            }
        }
        let monitors = crate::resistance::monitors(layout);
        if let Some(action) = editor.external(
            &monitors,
            ExternalPointer {
                monitor: &input.band.monitor,
                side,
                at: input.at,
                length: input.length,
                thickness: input.thickness,
                snap: !input.ctrl,
            },
            phase,
        ) {
            editor.selected = crate::resistance::apply(layout, action)?;
            out.changed = true;
        }
        Ok(out)
    }
}

pub fn side(side: lbm_overlay::scene::Side) -> Side {
    match side {
        lbm_overlay::scene::Side::Top => Side::Top,
        lbm_overlay::scene::Side::Bottom => Side::Bottom,
        lbm_overlay::scene::Side::Left => Side::Left,
        lbm_overlay::scene::Side::Right => Side::Right,
    }
}
