//! Shared diagonal clipping for a band, its sections and their resize caps.
use super::Side;
use egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke};

/// A band's shape. `start` and `end` (in millimetre order: top or left first) say
/// which of its ends are cut along the corner's diagonal.
pub(super) fn outline(r: Rect, side: Side, start: bool, end: bool) -> [Pos2; 4] {
    let t = r.width().min(r.height());
    let (s, e) = (if start { t } else { 0.0 }, if end { t } else { 0.0 });
    match side {
        Side::Left => [
            r.left_top(),
            pos2(r.right(), r.top() + s),
            pos2(r.right(), r.bottom() - e),
            r.left_bottom(),
        ],
        Side::Right => [
            r.right_top(),
            r.right_bottom(),
            pos2(r.left(), r.bottom() - e),
            pos2(r.left(), r.top() + s),
        ],
        Side::Top => [
            r.left_top(),
            r.right_top(),
            pos2(r.right() - e, r.bottom()),
            pos2(r.left() + s, r.bottom()),
        ],
        Side::Bottom => [
            r.left_bottom(),
            pos2(r.left() + s, r.top()),
            pos2(r.right() - e, r.top()),
            r.right_bottom(),
        ],
    }
}

fn clip(mut points: Vec<Pos2>, band: &[Pos2; 4]) -> Vec<Pos2> {
    for (a, b) in band.iter().zip(band.iter().cycle().skip(1)) {
        let edge = *b - *a;
        let distance = |p: Pos2| {
            let v = p - *a;
            edge.x * v.y - edge.y * v.x
        };
        let mut clipped = Vec::new();
        for (&p, &q) in points.iter().zip(points.iter().cycle().skip(1)) {
            let dp = distance(p);
            let dq = distance(q);
            if dp >= 0.0 {
                clipped.push(p);
            }
            if (dp >= 0.0) != (dq >= 0.0) {
                clipped.push(p + (q - p) * (dp / (dp - dq)));
            }
        }
        // A vertex exactly on the diagonal can also be emitted as an
        // intersection. Duplicate vertices break epaint's antialias normals.
        clipped.dedup_by(|a, b| a.distance_sq(*b) < 1e-8);
        if clipped.len() > 1 && clipped[0].distance_sq(*clipped.last().unwrap()) < 1e-8 {
            clipped.pop();
        }
        points = clipped;
    }
    points
}

fn fill(painter: &Painter, points: Vec<Pos2>, band: &[Pos2; 4], color: Color32) {
    let points = clip(points, band);
    if points.len() >= 3 {
        painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
    }
}

pub(super) fn rounded(
    painter: &Painter,
    rect: Rect,
    band: &[Pos2; 4],
    color: Color32,
    stroke: Stroke,
) {
    if !rect.is_positive() {
        return;
    }
    let mut points = Vec::new();
    egui::epaint::tessellator::path::rounded_rectangle(
        &mut points,
        rect,
        egui::epaint::CornerRadiusF32::same(2.0),
    );
    fill(painter, points.clone(), band, color);
    // Clip the original outline too: the mitre must not acquire an artificial
    // selection stroke where the neighbouring band's triangle was cut away.
    if stroke.width > 0.0 {
        for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
            let delta = b - a;
            if delta.length_sq() == 0.0 {
                continue;
            }
            let normal = egui::vec2(-delta.y, delta.x).normalized() * stroke.width * 0.5;
            fill(
                painter,
                vec![a - normal, b - normal, b + normal, a + normal],
                band,
                stroke.color,
            );
        }
    }
}
