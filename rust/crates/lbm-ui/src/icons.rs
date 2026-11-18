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
}

pub fn button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
    enabled: bool,
) -> egui::Response {
    ui.add_enabled(enabled, |ui: &mut egui::Ui| {
        let id = ui.make_persistent_id(("tool-icon", label));
        let mut button = egui::Button::new((egui::Atom::custom(id, vec2(16.0, 16.0)), label))
            .frame_when_inactive(false)
            .wrap_mode(egui::TextWrapMode::Extend);
        if let Some(on) = selected {
            button = button.selected(on).frame_when_inactive(on);
        } else if matches!(icon, Icon::Save) && ui.is_enabled() {
            button = button
                .fill(super::theme::SELECTED)
                .frame_when_inactive(true);
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
        response.response
    })
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
    }
}
