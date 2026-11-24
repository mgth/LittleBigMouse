//! Small line icons drawn at the display scale, without a font or image loader.
use egui::{vec2, Color32, Rect, Stroke};

#[derive(Clone, Copy)]
pub enum Icon {
    Info,
    Move,
    Border,
    Size,
    Sun,
    Picture,
    Mouse,
    Save,
    Undo,
    Play,
    Stop,
    Lock,
    Unlock,
    Collapse,
    // Property panel pictograms.
    EdgeLeft,
    EdgeTop,
    EdgeRight,
    EdgeBottom,
    Width,
    Height,
    OuterWidth,
    OuterHeight,
    Diagonal,
    PositionX,
    PositionY,
    ScaleX,
    ScaleY,
    Plus,
    Trash,
    Refresh,
    Expand,
    Mirror,
    Crossing,
    Eye,
    Import,
    Export,
    Palette,
    Rgb,
    Contrast,
    Speaker,
    Power,
    Chip,
    Layout,
    Display,
    Attach,
    Detach,
    Star,
    Pointer,
    Drag,
}

pub fn button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled(enabled, |ui: &mut egui::Ui| {
        // `add_enabled` hands over the row's own `Ui`: what `steady_at_rest` changes in
        // its style is put back before returning, or it would reach the next button.
        let style = ui.style().clone();
        let id = ui.make_persistent_id(("tool-icon", label));
        let mut button = egui::Button::new((egui::Atom::custom(id, vec2(16.0, 16.0)), label))
            .wrap_mode(egui::TextWrapMode::Extend);
        if let Some(on) = selected {
            button = button.selected(on);
            if on {
                button = button.frame_when_inactive(true);
            } else {
                button = super::theme::steady_at_rest(ui, button);
            }
        } else if matches!(icon, Icon::Save) && ui.is_enabled() {
            button = button
                .fill(super::theme::SELECTED)
                .frame_when_inactive(true);
        } else {
            button = super::theme::steady_at_rest(ui, button);
        }
        let response = button.atom_ui(ui);
        if let Some(rect) = response.rect(id) {
            let color = if !ui.is_enabled() {
                super::theme::MUTED.gamma_multiply(0.4)
            } else if selected == Some(true) {
                super::theme::ACCENT
            } else {
                super::theme::MUTED
            };
            paint(ui.painter(), rect, icon, color);
        }
        ui.set_style(style);
        response.response
    })
}

fn tone(ui: &egui::Ui, on: bool) -> Color32 {
    if !ui.is_enabled() {
        super::theme::MUTED.gamma_multiply(0.4)
    } else if on {
        super::theme::ACCENT
    } else {
        super::theme::MUTED
    }
}

/// A pictogram sized to sit in a line of panel text.
pub fn glyph(ui: &mut egui::Ui, icon: Icon) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), rect, icon, tone(ui, false));
    }
    response
}

/// A label led by its pictogram; returns the text so a field can be labelled by it.
pub fn label(ui: &mut egui::Ui, icon: Icon, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        glyph(ui, icon);
        ui.label(text)
    })
    .inner
}

/// A strong subsection title led by its pictogram.
pub fn title(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    label(ui, icon, egui::RichText::new(text).strong())
}

/// [`title`] for a text that may outgrow its column — a Windows output's interface path —
/// wrapped onto further lines instead of running past the panel. Kept apart from
/// [`label`], whose rows sit in grids sized by their content, where wrapping would
/// break every label to its narrowest.
pub fn wrapped_title(ui: &mut egui::Ui, icon: Icon, text: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        glyph(ui, icon);
        ui.add(egui::Label::new(egui::RichText::new(text).strong()).wrap())
    })
    .inner
}

/// A framed command led by its pictogram, the text staying its accessible name.
pub fn action(ui: &mut egui::Ui, icon: Icon, text: &str, enabled: bool) -> egui::Response {
    pictogram_button(ui, icon, text, None, enabled, true)
}

/// A selectable choice led by its pictogram, lit in the accent colour when chosen.
pub fn choice(ui: &mut egui::Ui, icon: Icon, text: &str, selected: bool) -> egui::Response {
    pictogram_button(ui, icon, text, Some(selected), true, false)
}

/// A switch-like command (on/off) led by its pictogram.
pub fn toggle(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    on: bool,
    enabled: bool,
) -> egui::Response {
    pictogram_button(ui, icon, text, Some(on), enabled, true)
}

fn pictogram_button(
    ui: &mut egui::Ui,
    icon: Icon,
    text: &str,
    selected: Option<bool>,
    enabled: bool,
    framed: bool,
) -> egui::Response {
    ui.add_enabled(enabled, |ui: &mut egui::Ui| {
        // Put back what `steady_at_rest` changes in the row's style (see `button`).
        let style = ui.style().clone();
        let id = ui.next_auto_id().with("pictogram");
        let mut button = egui::Button::new((egui::Atom::custom(id, vec2(14.0, 14.0)), text));
        if let Some(on) = selected {
            button = button.selected(on);
            if framed || on {
                button = button.frame_when_inactive(true);
            } else {
                button = super::theme::steady_at_rest(ui, button);
            }
        }
        let response = button.atom_ui(ui);
        if let Some(rect) = response.rect(id) {
            paint(ui.painter(), rect, icon, tone(ui, selected == Some(true)));
        }
        ui.set_style(style);
        response.response
    })
}

/// The side of a rectangle a pictogram singles out.
pub fn edge(side: crate::resistance::Side) -> Icon {
    use crate::resistance::Side;
    match side {
        Side::Left => Icon::EdgeLeft,
        Side::Top => Icon::EdgeTop,
        Side::Right => Icon::EdgeRight,
        Side::Bottom => Icon::EdgeBottom,
    }
}

pub(crate) fn paint(p: &egui::Painter, r: Rect, icon: Icon, color: Color32) {
    let at = |x: f32, y: f32| r.min + vec2(x, y) * (r.width() / 16.0);
    let stroke = Stroke::new(1.3, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            stroke,
        ));
    };
    let outline = |a: (f32, f32), b: (f32, f32)| {
        p.rect_stroke(
            Rect::from_min_max(at(a.0, a.1), at(b.0, b.1)),
            2.0,
            stroke,
            egui::StrokeKind::Inside,
        );
    };
    let faint = Stroke::new(1.0, color.gamma_multiply(0.55));
    let dim_outline = |a: (f32, f32), b: (f32, f32)| {
        p.rect_stroke(
            Rect::from_min_max(at(a.0, a.1), at(b.0, b.1)),
            1.5,
            faint,
            egui::StrokeKind::Inside,
        );
    };
    let dim_line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], faint);
    };
    let head = |from: (f32, f32), to: (f32, f32)| {
        let d = (vec2(to.0 - from.0, to.1 - from.1)).normalized() * 2.6;
        let side = vec2(-d.y, d.x) * 0.8;
        let tip = (to.0, to.1);
        let a = (tip.0 - d.x + side.x, tip.1 - d.y + side.y);
        let b = (tip.0 - d.x - side.x, tip.1 - d.y - side.y);
        p.add(egui::Shape::line(
            vec![at(a.0, a.1), at(tip.0, tip.1), at(b.0, b.1)],
            stroke,
        ));
    };
    let arrow = |from: (f32, f32), to: (f32, f32)| {
        line(from, to);
        head(from, to);
    };
    let both = |a: (f32, f32), b: (f32, f32)| {
        line(a, b);
        head(a, b);
        head(b, a);
    };
    // Angles in degrees, clockwise from the right, as screen coordinates run.
    let arc = |c: (f32, f32), radius: f32, from: f32, to: f32| -> Vec<egui::Pos2> {
        let steps = 20;
        (0..=steps)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / steps as f32).to_radians();
                at(c.0 + radius * a.cos(), c.1 + radius * a.sin())
            })
            .collect()
    };
    let unit = r.width() / 16.0;
    match icon {
        Icon::Lock | Icon::Unlock => {
            outline((3., 7.), (13., 15.));
            path(&[(5., 7.), (5., 4.), (7., 2.), (9., 2.), (11., 4.)]);
            if matches!(icon, Icon::Lock) {
                line((11., 4.), (11., 7.));
            }
            line((8., 10.), (8., 12.));
        }
        Icon::Collapse => path(&[(4., 10.), (8., 6.), (12., 10.)]),
        Icon::Info => {
            p.circle_stroke(r.center(), 6.0, stroke);
            line((8., 7.), (8., 11.));
            p.circle_filled(at(8., 4.5), 0.8, color);
        }
        Icon::Move => {
            line((1., 8.), (15., 8.));
            line((8., 1.), (8., 15.));
            for points in [
                &[(4., 5.), (1., 8.), (4., 11.)][..],
                &[(12., 5.), (15., 8.), (12., 11.)],
                &[(5., 4.), (8., 1.), (11., 4.)],
                &[(5., 12.), (8., 15.), (11., 12.)],
            ] {
                path(points);
            }
        }
        Icon::Border => {
            outline((1., 2.), (15., 14.));
            line((8., 3.), (8., 6.));
            line((8., 10.), (8., 13.));
            path(&[(5., 8.), (11., 8.), (9., 6.)]);
        }
        Icon::Size => {
            outline((1., 4.), (15., 12.));
            for x in [4., 7., 10., 13.] {
                line((x, 4.), (x, 7.));
            }
        }
        Icon::Sun => {
            p.circle_stroke(r.center(), 3.0, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([r.center() + d * 5., r.center() + d * 7.], stroke);
            }
        }
        Icon::Picture => {
            outline((1., 2.), (15., 14.));
            path(&[(2., 12.), (6., 8.), (9., 11.), (12., 7.), (14., 9.)]);
            p.circle_filled(at(5., 5.5), 1., color);
        }
        Icon::Mouse => {
            outline((4., 1.), (12., 15.));
            line((8., 1.), (8., 6.));
            line((4., 6.), (12., 6.));
        }
        Icon::Save => {
            outline((2., 1.), (14., 15.));
            outline((5., 9.), (11., 15.));
            path(&[(5., 1.), (5., 5.), (11., 5.), (11., 1.)]);
        }
        Icon::Undo => {
            path(&[(5., 2.), (1., 6.), (5., 10.)]);
            path(&[
                (1., 6.),
                (10., 6.),
                (13., 8.),
                (13., 12.),
                (10., 14.),
                (6., 14.),
            ]);
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![at(4., 2.), at(14., 8.), at(4., 14.)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_min_max(at(3., 3.), at(13., 13.)), 2., color);
        }
        Icon::EdgeLeft | Icon::EdgeTop | Icon::EdgeRight | Icon::EdgeBottom => {
            dim_outline((2., 3.), (14., 13.));
            let (a, b) = match icon {
                Icon::EdgeLeft => ((2.8, 3.), (2.8, 13.)),
                Icon::EdgeTop => ((2., 3.8), (14., 3.8)),
                Icon::EdgeRight => ((13.2, 3.), (13.2, 13.)),
                _ => ((2., 12.2), (14., 12.2)),
            };
            p.line_segment([at(a.0, a.1), at(b.0, b.1)], Stroke::new(2.4, color));
        }
        Icon::Width => {
            dim_outline((1., 3.), (15., 13.));
            both((3.5, 8.), (12.5, 8.));
        }
        Icon::Height => {
            dim_outline((1., 3.), (15., 13.));
            both((8., 4.8), (8., 11.2));
        }
        Icon::OuterWidth => {
            dim_outline((2., 7.), (14., 15.));
            both((2.5, 3.5), (13.5, 3.5));
            line((1.5, 1.5), (1.5, 5.5));
            line((14.5, 1.5), (14.5, 5.5));
        }
        Icon::OuterHeight => {
            dim_outline((1., 2.), (9., 14.));
            both((12.5, 2.5), (12.5, 13.5));
            line((10.5, 1.5), (14.5, 1.5));
            line((10.5, 14.5), (14.5, 14.5));
        }
        Icon::Diagonal => {
            dim_outline((1., 3.), (15., 13.));
            both((3.5, 10.8), (12.5, 5.2));
        }
        Icon::PositionX => {
            dim_line((2., 2.), (2., 14.));
            arrow((2., 8.), (14., 8.));
        }
        Icon::PositionY => {
            dim_line((2., 2.), (14., 2.));
            arrow((8., 2.), (8., 14.));
        }
        Icon::ScaleX => {
            dim_outline((5.5, 4.), (10.5, 12.));
            arrow((5., 8.), (1., 8.));
            arrow((11., 8.), (15., 8.));
        }
        Icon::ScaleY => {
            dim_outline((4., 5.5), (12., 10.5));
            arrow((8., 5.), (8., 1.));
            arrow((8., 11.), (8., 15.));
        }
        Icon::Plus => {
            line((8., 2.5), (8., 13.5));
            line((2.5, 8.), (13.5, 8.));
        }
        Icon::Trash => {
            line((2., 4.), (14., 4.));
            path(&[(6., 4.), (6., 2.), (10., 2.), (10., 4.)]);
            path(&[(3.5, 4.), (4.5, 14.5), (11.5, 14.5), (12.5, 4.)]);
            line((6.5, 7.), (6.5, 12.));
            line((9.5, 7.), (9.5, 12.));
        }
        Icon::Refresh => {
            p.add(egui::Shape::line(arc((8., 8.), 5.5, -50., 250.), stroke));
            let end = (
                8. + 5.5 * 250f32.to_radians().cos(),
                8. + 5.5 * 250f32.to_radians().sin(),
            );
            let before = (
                8. + 5.5 * 225f32.to_radians().cos(),
                8. + 5.5 * 225f32.to_radians().sin(),
            );
            head(before, end);
        }
        Icon::Expand => {
            line((1.5, 4.), (1.5, 12.));
            line((14.5, 4.), (14.5, 12.));
            both((3.5, 8.), (12.5, 8.));
        }
        Icon::Mirror => {
            for y in [1.5, 5.5, 9.5, 13.5] {
                dim_line((8., y), (8., y + 2.));
            }
            path(&[(6., 4.), (6., 12.), (1.5, 12.), (6., 4.)]);
            p.add(egui::Shape::convex_polygon(
                vec![at(10., 4.), at(14.5, 12.), at(10., 12.)],
                color.gamma_multiply(0.55),
                Stroke::NONE,
            ));
        }
        Icon::Crossing => {
            dim_line((8., 1.5), (8., 14.5));
            dim_line((8.6, 1.5), (8.6, 14.5));
            arrow((1.5, 8.), (14.5, 8.));
        }
        Icon::Eye => {
            path(&[(1., 8.), (3.5, 5.), (8., 3.5), (12.5, 5.), (15., 8.)]);
            path(&[(1., 8.), (3.5, 11.), (8., 12.5), (12.5, 11.), (15., 8.)]);
            p.circle_stroke(at(8., 8.), 2.2 * unit, stroke);
        }
        Icon::Import | Icon::Export => {
            path(&[(2., 10.), (2., 14.), (14., 14.), (14., 10.)]);
            if matches!(icon, Icon::Import) {
                arrow((8., 1.5), (8., 10.5));
            } else {
                arrow((8., 10.5), (8., 1.5));
            }
        }
        Icon::Palette => {
            p.add(egui::Shape::line(arc((8., 8.), 6.5, 70., 380.), stroke));
            path(&[(10.2, 14.2), (9.5, 11.5), (12.5, 10.5)]);
            for (x, y) in [(4.8, 8.), (6., 4.8), (9.5, 4.3), (12., 6.5)] {
                p.circle_filled(at(x, y), 1.05 * unit, color);
            }
        }
        Icon::Rgb => {
            let k = f32::from(color.a()) / 255.;
            for (c, tint) in [
                ((8., 5.5), Color32::from_rgb(237, 133, 133)),
                ((5.6, 9.8), super::theme::ACCENT),
                ((10.4, 9.8), Color32::from_rgb(135, 179, 242)),
            ] {
                p.circle_stroke(
                    at(c.0, c.1),
                    3.7 * unit,
                    Stroke::new(1.3, tint.gamma_multiply(k)),
                );
            }
        }
        Icon::Contrast => {
            p.circle_stroke(r.center(), 6.0 * unit, stroke);
            let mut half = arc((8., 8.), 6., 90., 270.);
            half.push(at(8., 2.));
            p.add(egui::Shape::convex_polygon(half, color, Stroke::NONE));
        }
        Icon::Speaker => {
            path(&[
                (1.5, 6.),
                (4.5, 6.),
                (8.5, 2.5),
                (8.5, 13.5),
                (4.5, 10.),
                (1.5, 10.),
                (1.5, 6.),
            ]);
            p.add(egui::Shape::line(arc((8.5, 8.), 3., -45., 45.), stroke));
            p.add(egui::Shape::line(arc((8.5, 8.), 5.8, -50., 50.), stroke));
        }
        Icon::Power => {
            p.add(egui::Shape::line(arc((8., 9.), 5.8, -55., 235.), stroke));
            line((8., 1.5), (8., 8.));
        }
        Icon::Chip => {
            outline((4., 4.), (12., 12.));
            for i in [6., 8., 10.] {
                line((i, 1.5), (i, 4.));
                line((i, 12.), (i, 14.5));
                line((1.5, i), (4., i));
                line((12., i), (14.5, i));
            }
        }
        Icon::Layout => {
            outline((1., 5.), (8.5, 12.5));
            dim_outline((9., 2.5), (15., 9.5));
        }
        Icon::Display | Icon::Attach | Icon::Detach => {
            outline((1., 2.), (15., 11.5));
            line((8., 11.5), (8., 14.));
            line((5., 14.5), (11., 14.5));
            if !matches!(icon, Icon::Display) {
                line((5.5, 6.75), (10.5, 6.75));
            }
            if matches!(icon, Icon::Attach) {
                line((8., 4.25), (8., 9.25));
            }
        }
        Icon::Star => {
            let points: Vec<_> = (0..=10)
                .map(|i| {
                    let a = (-90. + 36. * i as f32).to_radians();
                    let radius = if i % 2 == 0 { 7. } else { 2.9 };
                    at(8. + radius * a.cos(), 8.6 + radius * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(points, stroke));
        }
        Icon::Pointer => {
            path(&[
                (4., 1.5),
                (4., 13.),
                (6.8, 10.4),
                (8.8, 14.5),
                (10.6, 13.6),
                (8.6, 9.6),
                (12.3, 9.6),
                (4., 1.5),
            ]);
        }
        Icon::Drag => {
            outline((4., 1.), (12., 15.));
            p.rect_filled(Rect::from_min_max(at(4.6, 1.6), at(8., 6.)), 1.5, color);
            line((8., 1.), (8., 6.));
            line((4., 6.), (12., 6.));
        }
    }
}
