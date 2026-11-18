//! Monitor geometry stays proportional to the physical layout. Names fill their
//! top bezel using measured text bounds; cropped manufacturer logos fill the bottom
//! bezel at their own aspect ratio. Neither decoration changes the screen geometry.

use lbm_layout::geo::Rect;

/// Millimetres to points, per axis — the two differ when the map is stretched to fit a
/// window that is not the desktop's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ratio {
    pub x: f64,
    pub y: f64,
}

/// What a monitor looks like on the map, in points, relative to the map's own corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    /// The screen with its bezel: what the user sees as the object.
    pub outside: egui::Rect,
    /// The lit part, inside the bezel: what the layout is actually about.
    pub content: egui::Rect,
    /// Where the name goes: the top bezel, across the lit width — the frame's cell
    /// (0, 1). It is printed *on the plastic*, where a real monitor prints it, not over
    /// the picture.
    pub name_band: egui::Rect,
    /// Target ink height for the name, or `None` when there is no top bezel.
    pub name_height: Option<f32>,
    /// Where the manufacturer logo goes: the bottom bezel, across the lit width — the
    /// frame's cell (2, 1), the name's opposite number
    /// (`MonitorFrameView.axaml:196-206`: `Grid.Row="2" Grid.Column="1"`,
    /// `Height="{Binding Unrotated.BottomBorder}"`, `Width="{Binding Unrotated.Width}"`).
    pub logo_band: egui::Rect,
}

impl Drawn {
    /// The same monitor, moved. Every rectangle is listed rather than spread from
    /// `self`, so that a rectangle added to `Drawn` later cannot be quietly left behind
    /// — the compiler asks about it here, next to the field it belongs to.
    pub fn translate(self, by: egui::Vec2) -> Drawn {
        Drawn {
            outside: self.outside.translate(by),
            content: self.content.translate(by),
            name_band: self.name_band.translate(by),
            name_height: self.name_height,
            logo_band: self.logo_band.translate(by),
        }
    }
}

/// Use nearly the full bezel height, leaving a small margin around the ink.
const NAME_SHARE_OF_TOP_BEZEL: f64 = 0.9;

/// Where a monitor lands on the map, and how big its parts are.
///
/// `mm_outside` is the screen with its bezel and `mm_content` the lit part, both in the
/// layout's millimetre space; `origin` is the map's own corner in that space, so that
/// what comes out is relative to the map.
pub fn draw(mm_outside: Rect, mm_content: Rect, origin: (f64, f64), ratio: Ratio) -> Drawn {
    let place = |r: Rect| {
        egui::Rect::from_min_size(
            egui::pos2(
                ((r.left() - origin.0) * ratio.x) as f32,
                ((r.top() - origin.1) * ratio.y) as f32,
            ),
            egui::vec2((r.width() * ratio.x) as f32, (r.height() * ratio.y) as f32),
        )
    };
    let outside = place(mm_outside);
    let content = place(mm_content);
    // The top bezel, across the lit width: the frame grid's cell (0, 1), whose row is
    // the border's height and whose column is the panel's width.
    let name_band = egui::Rect::from_min_max(
        egui::pos2(content.left(), outside.top()),
        egui::pos2(content.right(), content.top()),
    );
    let name_height = (name_band.height() as f64 * NAME_SHARE_OF_TOP_BEZEL) as f32;
    // The bottom bezel, same width: the name's opposite number.
    let logo_band = egui::Rect::from_min_max(
        egui::pos2(content.left(), content.bottom()),
        egui::pos2(content.right(), outside.bottom()),
    );
    Drawn {
        outside,
        content,
        name_band,
        // No visibility cutoff: small monitors keep their names.
        // `None` only when there is no band to print on, which is not a judgement about
        // readability but about whether there is a size at all.
        name_height: (name_height > 0.0).then_some(name_height),
        logo_band,
    }
}

/// The colour a logo is painted in.
///
/// The Avalonia frame says `Foreground="LightGray"` outright, and can: its bezel is a
/// fixed pair of gradient brushes, so light grey always lands on the same dark plastic.
/// This frame paints the bezel from the theme, so a fixed light grey would be light on
/// light in a light theme. Following the theme is what keeps the *intent* of that
/// LightGray while retaining enough contrast on a small reflective bezel.
pub fn logo_colour(visuals: &egui::Visuals) -> egui::Color32 {
    visuals.text_color()
}

/// Strong enough to survive downscaling on the reflective bezel.
pub const LOGO_OPACITY: f32 = 0.95;

/// The largest rectangle of `of`'s shape that fits centred inside `into` — Avalonia's
/// `Stretch="Uniform"`, which is what an `Image` inside an `IconView` does by default.
///
/// Empty when there is no room or nothing to place, so a caller can skip the drawing
/// without a second condition.
pub fn fit_uniform(into: egui::Rect, of: egui::Vec2) -> egui::Rect {
    if of.x <= 0.0 || of.y <= 0.0 || into.width() <= 0.0 || into.height() <= 0.0 {
        return egui::Rect::from_min_size(into.center(), egui::Vec2::ZERO);
    }
    let scale = (into.width() / of.x).min(into.height() / of.y);
    egui::Rect::from_center_size(into.center(), of * scale)
}

/// The bezel's colour, which is how a selected screen is told apart.
///
/// A function rather than two lines inside the painting, so that "selected looks
/// different" is something a test can ask without a pixel: the drawing itself can only
/// be checked by eye, but the choice it makes can be checked here.
pub fn bezel_fill(visuals: &egui::Visuals, selected: bool) -> egui::Color32 {
    if selected {
        visuals.selection.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    }
}

/// What is written on a frame, as opposed to where the frame is.
///
/// A struct rather than more arguments, because the frame is going to gain the wallpaper
/// thumbnail and the prober's strips, and a call of six positional flags says nothing at
/// the call site.
#[derive(Clone, Copy, Default)]
pub struct Look<'a> {
    /// `PhysicalMonitorModel.PnpDeviceName` — "DELL P2419H", not the layout's id.
    pub name: &'a str,
    pub selected: bool,
    /// The manufacturer logo, already resolved through `lbm_icons`, recoloured to
    /// [`logo_colour`] and uploaded. `None` when the monitor has no logo, which is a
    /// real case: the Linux factory leaves `Logo` empty for a monitor that reports no
    /// EDID manufacturer code, where the Windows one falls back to the LittleBigMouse
    /// mark.
    pub logo: Option<&'a egui::TextureHandle>,
    /// The desktop picture as it will look on this screen, already cut to shape by
    /// [`crate::wallpaper`] and uploaded. Drawn stretched over the lit part, which is
    /// what `Stretch="Fill"` on the frame's image means — the thumbnail is already the
    /// screen's own shape, so filling it distorts nothing.
    pub wallpaper: Option<&'a egui::TextureHandle>,
    /// What the chosen view mode writes inside the lit part, as label/value rows —
    /// empty for [`crate::mode::Mode::Default`], which writes nothing there.
    ///
    /// The frame does not know which mode this is, and does not want to: in C# the
    /// content is a whole view resolved from the mode, and what every ported one of them
    /// comes down to is a name/value grid. Whoever holds the layout fills the rows; the
    /// frame only has to fit them.
    ///
    /// **Not scaled with the frame**, unlike the name. In C# the content is an ordinary
    /// control in the frame's middle cell, laid out at the app's font size; a screen
    /// drawn small gets fewer rows, not smaller ones. Text that shrinks with the map
    /// would be unreadable on exactly the desktops that have most to say.
    pub details: &'a [(&'a str, String)],
    pub probe: Option<crate::probe::Overlay<'a>>,
}

/// The view mode's rows, inside the lit part.
///
/// **Clipped, and that is the point.** A screen drawn small cannot hold five rows of
/// text, and there is no honest way to make it: shrinking the font gives ink that looks
/// like information and cannot be read, and letting it spill writes one screen's numbers
/// across its neighbour. So the rows are laid out at the app's own size and the frame cuts
/// off what does not fit — the same thing the C#'s grid does inside its cell.
fn write_details(ui: &mut egui::Ui, content: egui::Rect, rows: &[(&str, String)]) {
    // Off the edges, so the text does not touch the bezel.
    let inside = content.shrink(10.0);
    if inside.width() < 24.0 || inside.height() < 12.0 {
        return;
    }
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inside)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    ui.set_clip_rect(inside);
    ui.spacing_mut().item_spacing.y = 5.0;
    for (label, value) in rows {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(*label)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.label(egui::RichText::new(value).small().monospace());
        });
    }
}

/// A fine inset shared by names and logos: at least one physical pixel at normal
/// sizes, capped on tiny bezels so the margin cannot consume the whole band.
fn bezel_inner(band: egui::Rect, pixels_per_point: f32) -> egui::Rect {
    let margin = (band.height() * 0.06)
        .max(1.0 / pixels_per_point)
        .min(band.height().max(0.0) * 0.2)
        .min(band.width().max(0.0) * 0.2);
    band.shrink(margin)
}

/// Fit the ink rather than the font's line box: ascender/descender padding otherwise
/// wastes most of a thin bezel. Long names shrink as one line instead of wrapping.
fn write_name(ui: &mut egui::Ui, band: egui::Rect, height: f32, name: &str) {
    if name.is_empty() || !band.is_positive() {
        return;
    }
    let band = bezel_inner(band, ui.ctx().pixels_per_point());
    let target = egui::Rect::from_center_size(
        band.center(),
        egui::vec2(band.width(), height.min(band.height())),
    );
    let color = ui.visuals().text_color();
    let layout = |size| {
        ui.painter()
            .layout_no_wrap(name.to_owned(), egui::FontId::proportional(size), color)
    };
    let mut size = height * 1.4;
    let mut galley = layout(size);
    for _ in 0..3 {
        let ink = galley.mesh_bounds.size();
        if ink.x <= 0.0 || ink.y <= 0.0 {
            break;
        }
        let scale = (target.width() / ink.x).min(target.height() / ink.y);
        if (scale - 1.0).abs() < 0.02 {
            break;
        }
        size *= scale;
        galley = layout(size);
    }
    // At subpixel scales the rasterizer can produce no ink. Keep the accessible
    // name and its real band rather than silently removing the monitor's label.
    let ink = if galley.mesh_bounds.is_positive() {
        galley.mesh_bounds
    } else {
        egui::Rect::from_min_size(egui::Pos2::ZERO, target.size())
    };
    let origin = egui::pos2(
        target.left() - ink.left(),
        target.center().y - ink.center().y,
    );
    let bounds = ink.translate(origin.to_vec2()).intersect(band);
    ui.scope_builder(egui::UiBuilder::new().max_rect(band), |ui| {
        let response = ui.allocate_rect(bounds, egui::Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), name)
        });
        ui.painter()
            .with_clip_rect(band.intersect(ui.clip_rect()))
            .galley(origin, galley, color);
    });
}

/// The C# casing's diagonal reflection: a bright ramp ending sharply halfway
/// across the surface, then a darker ramp. Both sides share the same geometry,
/// including rounded corners, so the highlight also works on portrait monitors.
fn reflection(painter: &egui::Painter, rect: egui::Rect, radius: f32, strength: u8) {
    if !rect.is_positive() {
        return;
    }
    // MonitorFrameView.axaml: StartPoint="0%,40%", EndPoint="100%,60%".
    let start = rect.min + egui::vec2(0.0, rect.height() * 0.4);
    let direction = egui::vec2(rect.width(), rect.height() * 0.2);
    let position = |p: egui::Pos2| (p - start).dot(direction) / direction.length_sq();
    let radius = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    let mut outline = Vec::with_capacity(36);
    for (center, quarter) in [
        (rect.right_top() + egui::vec2(-radius, radius), -1.0),
        (rect.right_bottom() + egui::vec2(-radius, -radius), 0.0),
        (rect.left_bottom() + egui::vec2(radius, -radius), 1.0),
        (rect.left_top() + egui::vec2(radius, radius), 2.0),
    ] {
        for step in 0..=8 {
            let angle = (quarter + step as f32 / 8.0) * std::f32::consts::FRAC_PI_2;
            outline.push(center + egui::vec2(angle.cos(), angle.sin()) * radius);
        }
    }
    let mut mesh = egui::Mesh::default();
    for bright in [true, false] {
        let mut polygon = Vec::with_capacity(outline.len() + 2);
        // Clip the rounded outline at the gradient's middle stop. Duplicating the
        // shared edge lets the light jump there, just like the two 0.5 stops in C#.
        for i in 0..outline.len() {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            let ta = position(a);
            let tb = position(b);
            let inside = |t: f32| if bright { t <= 0.5 } else { t >= 0.5 };
            if inside(ta) {
                polygon.push(a);
            }
            if inside(ta) != inside(tb) {
                polygon.push(a + (b - a) * ((0.5 - ta) / (tb - ta)));
            }
        }
        let first = mesh.vertices.len() as u32;
        for &p in &polygon {
            let t = position(p).clamp(0.0, 1.0);
            let alpha = if bright { 2.0 * t } else { 0.16 * (1.0 - t) };
            mesh.colored_vertex(
                p,
                egui::Color32::from_white_alpha((alpha * strength as f32) as u8),
            );
        }
        for i in 1..polygon.len().saturating_sub(1) {
            mesh.add_triangle(first, first + i as u32, first + i as u32 + 1);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Draws one monitor. Returns the rectangle it took, so a caller can lay several out.
pub fn monitor(ui: &mut egui::Ui, drawn: &Drawn, look: &Look) -> egui::Rect {
    monitor_with_panorama(ui, drawn, look, None)
}

/// A shared image behind every panel, clipped before the glass reflection is drawn.
pub fn monitor_with_panorama(
    ui: &mut egui::Ui,
    drawn: &Drawn,
    look: &Look,
    panorama: Option<(&egui::TextureHandle, egui::Rect)>,
) -> egui::Rect {
    let Look {
        name,
        selected,
        logo,
        wallpaper,
        details,
        probe,
    } = *look;
    let fill = bezel_fill(ui.visuals(), selected);
    let painter = ui.painter();
    // The bezel is what is between the two rectangles; drawing the outside first and
    // the lit part over it is the whole of it.
    let radius = (drawn.outside.height() * 0.025).clamp(2.0, 9.0);
    // Rounded casing, a soft lower shadow and a lit upper edge, like the C# frame.
    for spread in (1..=4).rev() {
        painter.rect_filled(
            drawn
                .outside
                .translate(egui::vec2(0.0, 3.0))
                .expand(spread as f32),
            radius + spread as f32,
            egui::Color32::from_black_alpha(10),
        );
    }
    painter.rect_filled(drawn.outside, radius, fill);
    reflection(painter, drawn.outside, radius, 72);
    painter.rect_stroke(
        drawn.outside,
        radius,
        egui::Stroke::new(
            1.0,
            if selected {
                ui.visuals().selection.stroke.color
            } else {
                crate::theme::BORDER
            },
        ),
        egui::StrokeKind::Inside,
    );
    crate::theme::gradient(
        painter,
        drawn.content,
        egui::Color32::from_rgb(27, 38, 46),
        ui.visuals().extreme_bg_color,
    );
    if let Some((picture, image_rect)) = panorama {
        painter.rect_filled(drawn.content, 0.0, egui::Color32::BLACK);
        painter
            .with_clip_rect(drawn.content.intersect(painter.clip_rect()))
            .image(
                picture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
    } else if let Some(picture) = wallpaper {
        // Over the background colour and under everything else, as the frame's third
        // grid is: what is on the screen, not what is written about it.
        painter.image(
            picture.id(),
            drawn.content,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // A quieter sheen over the glass (and wallpaper), aligned with the casing.
    // Information and crossing overlays are drawn afterwards to stay crisp.
    reflection(
        &painter.with_clip_rect(drawn.content.intersect(painter.clip_rect())),
        drawn.outside,
        radius,
        26,
    );

    painter.rect_stroke(
        drawn.content,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(150)),
        egui::StrokeKind::Inside,
    );
    // The original's small power indicator on the lower bezel.
    if drawn.logo_band.height() >= 4.0 {
        let center = egui::pos2(drawn.content.right() - 5.0, drawn.logo_band.center().y);
        painter.circle_filled(
            center,
            (drawn.logo_band.height() * 0.12).clamp(1.0, 2.0),
            if selected {
                crate::theme::ACCENT
            } else {
                crate::theme::MUTED
            },
        );
    }

    if let Some(height) = drawn.name_height {
        write_name(ui, drawn.name_band, height, name);
    }

    if !details.is_empty() {
        write_details(ui, drawn.content, details);
    }
    if let Some(probe) = probe {
        probe.draw(ui, drawn.content);
    }

    if let Some(logo) = logo {
        // Uniform and centred in the bottom bezel, as the `IconView`'s `Image` is. The
        // colour is already in the texture — `lbm_icons::load` paints the black paths on
        // the way in — so the tint carries only the opacity.
        let where_ = fit_uniform(
            bezel_inner(drawn.logo_band, ui.ctx().pixels_per_point()),
            logo.size_vec2(),
        );
        if where_.width() >= 1.0 && where_.height() >= 1.0 {
            ui.painter().image(
                logo.id(),
                where_,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::from_white_alpha((LOGO_OPACITY * 255.0) as u8),
            );
        }
    }
    drawn.outside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> (Rect, Rect) {
        // A 600x340 mm screen with a 10 mm bezel all round.
        let outside = Rect::new(0.0, 0.0, 620.0, 360.0);
        let content = Rect::new(10.0, 10.0, 600.0, 340.0);
        (outside, content)
    }

    #[test]
    fn everything_scales_with_the_ratio() {
        let (outside, content) = screen();
        // Both bands and their target ink heights scale with the physical bezel.
        let one = draw(outside, content, (0.0, 0.0), Ratio { x: 2.0, y: 2.0 });
        let two = draw(outside, content, (0.0, 0.0), Ratio { x: 4.0, y: 4.0 });

        assert_eq!(two.outside.width(), one.outside.width() * 2.0);
        assert_eq!(two.content.height(), one.content.height() * 2.0);
        assert_eq!(two.name_height.unwrap(), one.name_height.unwrap() * 2.0);
    }

    #[test]
    fn the_lit_part_sits_inside_the_bezel_by_the_bezel() {
        let (outside, content) = screen();
        let ratio = Ratio { x: 0.5, y: 0.5 };

        let drawn = draw(outside, content, (0.0, 0.0), ratio);

        // 10 mm of bezel at half a point per millimetre is 5 points, on every side.
        assert_eq!(drawn.content.left() - drawn.outside.left(), 5.0);
        assert_eq!(drawn.outside.right() - drawn.content.right(), 5.0);
        assert_eq!(drawn.content.top() - drawn.outside.top(), 5.0);
        assert_eq!(drawn.outside.bottom() - drawn.content.bottom(), 5.0);
    }

    #[test]
    fn the_map_corner_is_where_the_drawing_starts() {
        let (outside, content) = screen();
        // A second screen 700 mm to the right, on a map whose corner is that screen's.
        let right = Rect::new(700.0, 0.0, 620.0, 360.0);
        let right_content = Rect::new(710.0, 10.0, 600.0, 340.0);

        let drawn = draw(right, right_content, (700.0, 0.0), Ratio { x: 1.0, y: 1.0 });

        assert_eq!(
            drawn.outside.left(),
            0.0,
            "the map starts at its own corner"
        );
        assert_eq!(
            draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 })
                .outside
                .left(),
            0.0
        );
    }

    /// The two bezels carry one thing each, at opposite ends, and neither of them
    /// reaches over the picture. A logo drawn on the lit part would cover the screen it
    /// is labelling.
    #[test]
    fn the_name_is_on_the_top_bezel_and_the_logo_on_the_bottom() {
        let (outside, content) = screen();
        let drawn = draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 });

        assert_eq!(drawn.name_band.bottom(), drawn.content.top());
        assert_eq!(drawn.logo_band.top(), drawn.content.bottom());
        assert_eq!(drawn.logo_band.bottom(), drawn.outside.bottom());
        // Both across the lit width, which is the grid's middle column.
        for band in [drawn.name_band, drawn.logo_band] {
            assert_eq!(band.left(), drawn.content.left());
            assert_eq!(band.right(), drawn.content.right());
        }
        assert!(!drawn.name_band.intersects(drawn.logo_band));
    }

    /// Uniform: the logo keeps its shape and is centred, however wrong the band's shape
    /// is for it. A bezel is a long thin strip, so this is the normal case and not an
    /// edge one.
    #[test]
    fn a_logo_keeps_its_shape_in_a_band_of_the_wrong_one() {
        // A square logo in a strip ten times as wide as it is tall.
        let band = egui::Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(500.0, 50.0));
        let placed = fit_uniform(band, egui::vec2(64.0, 64.0));

        assert_eq!(placed.width(), placed.height(), "it was stretched");
        assert_eq!(
            placed.height(),
            50.0,
            "the tight axis is the one that binds"
        );
        assert_eq!(placed.center(), band.center(), "and it is centred");
        assert!(band.contains_rect(placed));

        // A wide logo in the same strip binds on the other axis.
        let wide = fit_uniform(band, egui::vec2(1000.0, 20.0));
        assert_eq!(wide.width(), 500.0);
        assert!((wide.height() - 10.0).abs() < 1e-4);
    }

    /// Nothing to place, or nowhere to place it, comes back empty rather than infinite
    /// or negative — the caller skips on the size, so the size has to be honest.
    #[test]
    fn a_logo_with_no_room_or_no_size_is_empty() {
        let band = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(500.0, 50.0));
        for of in [egui::vec2(0.0, 64.0), egui::vec2(64.0, 0.0)] {
            assert_eq!(fit_uniform(band, of).area(), 0.0);
        }
        let none = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::Vec2::ZERO);
        assert_eq!(fit_uniform(none, egui::vec2(64.0, 64.0)).area(), 0.0);
    }

    /// A logo the colour of the plastic it sits on is not a logo. The Avalonia frame can
    /// hard-code LightGray because its bezel is a fixed brush; this one cannot.
    #[test]
    fn a_logo_is_not_the_colour_of_the_bezel_under_it() {
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            for selected in [false, true] {
                assert_ne!(
                    logo_colour(&visuals),
                    bezel_fill(&visuals, selected),
                    "the logo vanishes into the bezel"
                );
            }
        }
    }

    /// Whatever the theme, the selected screen is not painted like the others — the
    /// map is read at a glance, and a selection you have to infer is not one.
    #[test]
    fn a_selected_screen_is_not_painted_like_the_rest() {
        for visuals in [egui::Visuals::light(), egui::Visuals::dark()] {
            assert_ne!(
                bezel_fill(&visuals, true),
                bezel_fill(&visuals, false),
                "selection is invisible in this theme"
            );
        }
    }

    /// **However small.** The floor this port used to carry is gone: a name of a tenth
    /// of a point is still a name, and dropping it is what made a map at an ordinary
    /// window size lose every one of them.
    #[test]
    fn a_tiny_name_is_still_drawn() {
        let (outside, content) = screen();
        let tiny = draw(outside, content, (0.0, 0.0), Ratio { x: 0.01, y: 0.01 });
        // Small names stay present rather than being hidden by a size threshold.
        assert!((tiny.name_height.unwrap() - 0.09).abs() < 1e-6);
    }

    /// The one case left, and it is arithmetic rather than taste: a screen whose panel
    /// reaches the top of its own outline has no plastic to print on. A font size of
    /// zero is not a small name.
    #[test]
    fn a_screen_with_no_top_bezel_has_nowhere_to_print_its_name() {
        // 600x340 with bezels everywhere but the top.
        let outside = Rect::new(0.0, 0.0, 620.0, 350.0);
        let content = Rect::new(10.0, 0.0, 600.0, 340.0);
        let drawn = draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 });
        assert_eq!(drawn.name_band.height(), 0.0);
        assert_eq!(drawn.name_height, None);
    }

    /// The name is printed on the plastic, where a monitor prints it — not over the
    /// picture. Getting this wrong is not a matter of taste: a name laid over the lit
    /// part covers the thing the map is about, and on a small frame it covers all of it.
    #[test]
    fn the_name_is_printed_on_the_bezel_and_not_on_the_screen() {
        let (outside, content) = screen();
        let drawn = draw(outside, content, (0.0, 0.0), Ratio { x: 1.0, y: 1.0 });

        assert!(
            drawn.name_band.bottom() <= drawn.content.top(),
            "the name band reaches over the lit part: {:?} against {:?}",
            drawn.name_band,
            drawn.content
        );
        assert_eq!(drawn.name_band.top(), drawn.outside.top());
        // Across the lit width, which is the frame grid's middle column.
        assert_eq!(drawn.name_band.left(), drawn.content.left());
        assert_eq!(drawn.name_band.right(), drawn.content.right());
    }

    /// The target ink height uses the bezel, not the lit panel.
    #[test]
    fn the_name_is_measured_against_the_bezel_not_the_panel() {
        let ratio = Ratio { x: 20.0, y: 20.0 };
        // Same 600x340 panel, two different bezels.
        let thin = draw(
            Rect::new(0.0, 0.0, 604.0, 344.0),
            Rect::new(2.0, 2.0, 600.0, 340.0),
            (0.0, 0.0),
            ratio,
        );
        let thick = draw(
            Rect::new(0.0, 0.0, 640.0, 380.0),
            Rect::new(20.0, 20.0, 600.0, 340.0),
            (0.0, 0.0),
            ratio,
        );

        assert_eq!(thin.name_height, Some(36.0), "90% of a 2 mm bezel, at 20:1");
        assert_eq!(thick.name_height, Some(360.0), "90% of a 20 mm bezel");
        assert_eq!(
            thin.content.height(),
            thick.content.height(),
            "the panels are the same, so a panel-sized rule would give the same name"
        );
    }
}
