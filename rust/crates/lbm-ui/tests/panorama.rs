use egui::{pos2, vec2};
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_layout::geo::{Rect, Size};
use lbm_store::wallpaper_settings::SpanPlacement;
use lbm_ui::{map::MapMonitor, panorama::Editor};
struct State {
    editor: Editor,
    texture: Option<egui::TextureHandle>,
    placement: Option<SpanPlacement>,
    enabled: bool,
}
fn harness(width: f32) -> Harness<'static, State> {
    Harness::builder()
        .with_size(vec2(width, 560.))
        .build_ui_state(
            |ui, state| {
                lbm_ui::theme::install(ui.ctx());
                ui.painter()
                    .rect_filled(ui.max_rect(), 0., lbm_ui::theme::BACKGROUND);
                let picture = state.texture.get_or_insert_with(|| {
                    let mut image = egui::ColorImage::filled([600, 300], egui::Color32::BLACK);
                    for y in 0..300 {
                        for x in 0..600 {
                            image[(x, y)] = egui::Color32::from_rgb(
                                (x * 255 / 600) as u8,
                                (y * 255 / 300) as u8,
                                130,
                            );
                        }
                    }
                    ui.ctx()
                        .load_texture("panorama-test", image, egui::TextureOptions::LINEAR)
                });
                if !state.enabled {
                    ui.disable();
                }
                let monitors = [
                    MapMonitor {
                        id: "left",
                        name: "Landscape",
                        mm_outside: Rect::new(-400., 0., 400., 240.),
                        mm_content: Rect::new(-390., 8., 380., 222.),
                        logo: None,
                        wallpaper: None,
                        details: &[],
                        probe: None,
                    },
                    MapMonitor {
                        id: "right",
                        name: "Portrait",
                        mm_outside: Rect::new(20., -80., 200., 320.),
                        mm_content: Rect::new(28., -72., 184., 302.),
                        logo: None,
                        wallpaper: None,
                        details: &[],
                        probe: None,
                    },
                ];
                state.editor.show(
                    ui,
                    &monitors,
                    picture,
                    Size::new(600., 300.),
                    &mut state.placement,
                    Some("left"),
                );
            },
            State {
                editor: Editor::default(),
                texture: None,
                placement: None,
                enabled: true,
            },
        )
}
fn handle(h: &Harness<State>, label: &str) -> egui::Pos2 {
    let r = h
        .get_by_label(label)
        .accesskit_node()
        .bounding_box()
        .unwrap();
    pos2((r.x0 + r.x1) as f32 / 2., (r.y0 + r.y1) as f32 / 2.)
}
#[test]
fn resize_has_stable_opposite_anchor_and_escape_restores_the_original_draft() {
    let mut h = harness(900.);
    h.run();
    let top_left = handle(&h, "Resize panorama top left");
    let bottom_right = handle(&h, "Resize panorama bottom right");
    h.drag_at(bottom_right);
    h.step();
    let mut previous = 1.;
    for d in [12., 24., 36.] {
        h.hover_at(bottom_right + vec2(d, d / 2.));
        h.step();
        let p = h.state().placement.unwrap();
        assert!(p.scale > previous);
        previous = p.scale;
        assert!((handle(&h, "Resize panorama top left") - top_left).length() < 0.01);
    }
    h.key_press(egui::Key::Escape);
    h.step();
    assert_eq!(h.state().placement, None);
    h.drop_at(bottom_right);
    h.run();
    assert_eq!(h.state().placement, None);
}
#[test]
fn image_moves_over_the_monitors_without_resizing_and_disabled_canvas_cannot_edit() {
    let mut h = harness(900.);
    h.run();
    let a = handle(&h, "Resize panorama top left");
    let b = handle(&h, "Resize panorama bottom right");
    let center = a + (b - a) * 0.5;
    h.drag_at(center);
    h.step();
    h.hover_at(center + vec2(25., -10.));
    h.step();
    let p = h.state().placement.unwrap();
    assert_eq!(p.scale, 1.);
    assert!(p.offset_x_mm > 0. && p.offset_y_mm < 0.);
    h.drop_at(center + vec2(25., -10.));
    h.run();
    h.state_mut().enabled = false;
    h.run();
    let before = h.state().placement;
    h.drag_at(center);
    h.step();
    h.drop_at(center + vec2(60., 0.));
    h.run();
    assert_eq!(h.state().placement, before);
}
#[test]
fn panorama_canvas_wide() {
    let mut h = harness(900.);
    h.run();
    #[cfg(not(windows))]
    h.snapshot("panorama-framing-wide");
}
#[test]
fn panorama_canvas_narrow() {
    let mut h = harness(400.);
    h.run();
    #[cfg(not(windows))]
    h.snapshot("panorama-framing-narrow");
}
