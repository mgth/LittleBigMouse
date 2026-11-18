use lbm_layout::{
    geo::{Point, Rect},
    linux::{add_monitor, LinuxMonitor},
    model::{BorderSection, Layout, LayoutOptions, LayoutSource},
};
use lbm_overlay::{
    raster,
    scene::{self, Band, Content, Pixels, Side},
    Backend, Manager, Mode,
};
use std::{cell::RefCell, rc::Rc};

fn layout() -> Layout {
    let mut layout = Layout::new(LayoutOptions::default());
    for (id, x, y, w, h, mmw, mmh, scale, primary) in [
        ("A", 0., 0., 1280., 720., 320., 180., 1., true),
        ("B", -800., -300., 800., 1500., 200., 375., 2., false),
    ] {
        add_monitor(
            &mut layout,
            &LinuxMonitor {
                connector_name: id.into(),
                logical_x: x,
                logical_y: y,
                logical_width: w,
                logical_height: h,
                pixel_width: w as i32,
                pixel_height: h as i32,
                width_mm: mmw,
                height_mm: mmh,
                scale,
                primary,
                ..LinuxMonitor::fallback()
            },
        );
    }
    layout.set_location("A", Point::new(0., 0.));
    layout.set_location("B", Point::new(-200., -75.));
    layout
}
fn rulers() -> Vec<Band> {
    scene::from_layout(&layout(), Mode::Rulers, Some("A")).unwrap()
}
fn references(horizontal: bool, layout_mm: f64, kind: scene::GuideKind) -> Vec<Band> {
    scene::with_reference(
        &layout(),
        Mode::Resistance,
        None,
        Some(scene::Reference {
            horizontal,
            layout_mm,
            kind,
        }),
    )
    .unwrap()
}
#[test]
fn references_cross_only_the_physical_panels_they_intersect_with_per_monitor_dpi() {
    let bands = references(true, 90., scene::GuideKind::Section);
    let guides: Vec<_> = bands
        .iter()
        .filter(|b| matches!(b.content, Content::Guide { .. }))
        .collect();
    assert_eq!(guides.len(), 2);
    assert_eq!(
        guides[0].rect,
        Pixels {
            x: 0,
            y: 359,
            width: 1280,
            height: 2
        }
    );
    assert_eq!(
        guides[1].rect,
        Pixels {
            x: -800,
            y: 358,
            width: 800,
            height: 4
        }
    );
    assert_eq!(guides[1].screen.height, 1500); // negative-origin portrait at 200%
    assert_eq!(
        references(true, -50., scene::GuideKind::ScreenEdge).len(),
        9
    );
    assert_eq!(
        references(true, 900., scene::GuideKind::ScreenEdge).len(),
        8
    );
    let vertical = references(false, 100., scene::GuideKind::Middle);
    assert_eq!(vertical.len(), 9);
    assert_eq!(
        vertical.last().unwrap().rect,
        Pixels {
            x: 399,
            y: 0,
            width: 2,
            height: 720
        }
    );
    let decoded: Vec<Band> = serde_json::from_slice(&serde_json::to_vec(&bands).unwrap()).unwrap();
    assert_eq!(decoded, bands);
}
#[test]
fn a_guide_leaving_a_screen_never_hands_its_surface_to_a_screen_of_another_dpi() {
    // A (100 %) spans y = 0..180 mm and B (200 %) y = -75..300 mm. Going straight from a
    // reference across both to one across B alone must not give A's guide surface to B.
    let both = references(true, 90., scene::GuideKind::Section);
    let only_b = references(true, 250., scene::GuideKind::Section);
    let guide = |bands: &[Band]| -> Vec<String> {
        bands
            .iter()
            .filter(|b| matches!(b.content, Content::Guide { .. }))
            .map(|b| b.connector.clone())
            .collect()
    };
    assert_eq!(guide(&both), ["A", "B"]);
    assert_eq!(guide(&only_b), ["B"]);
    let previous: Vec<_> = both.iter().map(Band::binding).collect();
    let plan = scene::reuse(&previous, &only_b);
    for (band, index) in only_b.iter().zip(&plan) {
        let index = index.expect("every band keeps a surface of its own screen and role");
        assert_eq!(both[index].binding(), band.binding());
        assert_eq!(both[index].connector, band.connector);
        assert_eq!(both[index].scale, band.scale);
    }
    let guide_index = plan.last().unwrap().unwrap();
    assert_eq!(both[guide_index].connector, "B");
    // A's guide surface is the only one left over, so it is released.
    let mut used: Vec<_> = plan.iter().flatten().copied().collect();
    used.sort();
    let released: Vec<_> = (0..both.len()).filter(|i| !used.contains(i)).collect();
    assert_eq!(released.len(), 1);
    assert_eq!(both[released[0]].connector, "A");
    assert!(matches!(both[released[0]].content, Content::Guide { .. }));

    // A guide never takes the band of its own side, and the reverse transition creates
    // A's guide surface instead of borrowing B's.
    let back = scene::reuse(&only_b.iter().map(Band::binding).collect::<Vec<_>>(), &both);
    assert_eq!(back.iter().filter(|i| i.is_none()).count(), 1);
    let created = back.iter().position(Option::is_none).unwrap();
    assert_eq!(both[created].connector, "A");
    assert!(matches!(both[created].content, Content::Guide { .. }));
}
#[test]
fn guide_endpoints_remain_inside_each_output_and_opt_in_controls_their_lifetime() {
    for (horizontal, mm) in [(false, 0.), (true, 180.), (true, -75.), (true, 300.)] {
        for b in references(horizontal, mm, scene::GuideKind::ScreenEdge) {
            assert!(b.rect.x >= b.screen.x && b.rect.y >= b.screen.y);
            assert!(b.rect.x + b.rect.width <= b.screen.x + b.screen.width);
            assert!(b.rect.y + b.rect.height <= b.screen.y + b.screen.height);
        }
    }
    let r = scene::Reference {
        horizontal: true,
        layout_mm: 90.,
        kind: scene::GuideKind::Middle,
    };
    assert!(scene::with_reference(&layout(), Mode::Off, None, Some(r))
        .unwrap()
        .is_empty());
    assert_eq!(
        scene::with_reference(&layout(), Mode::Rulers, Some("A"), Some(r)).unwrap(),
        rulers()
    );
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut manager = Manager::new(Fake(trace.clone()));
    let base = scene::with_reference(&layout(), Mode::Resistance, None, None).unwrap();
    let guides = scene::with_reference(&layout(), Mode::Resistance, None, Some(r)).unwrap();
    for bands in [&base, &guides, &guides, &base] {
        manager.sync(bands).unwrap();
    }
    assert_eq!(trace.borrow().calls, ["present", "present", "present"]);
    assert!(scene::with_reference(
        &layout(),
        Mode::Resistance,
        None,
        Some(scene::Reference {
            layout_mm: f64::NAN,
            ..r
        })
    )
    .is_err());
}
#[test]
fn guide_dashes_have_transparent_gaps_and_the_reference_palette_without_corner_miters() {
    for horizontal in [false, true] {
        for kind in [
            scene::GuideKind::Middle,
            scene::GuideKind::ScreenEdge,
            scene::GuideKind::Section,
        ] {
            let bands = references(horizontal, 90., kind);
            let guide = bands
                .iter()
                .find(|b| matches!(b.content, Content::Guide { .. }))
                .unwrap();
            let image = raster::render(guide).unwrap();
            let [r, g, b, a] = kind.rgba();
            let premultiply = |v: u8| (u16::from(v) * u16::from(a) / 255) as u8;
            let pixel = |axis: usize| {
                let offset = if horizontal {
                    axis * 4
                } else {
                    axis * image.width * 4
                };
                &image.bgra[offset..offset + 4]
            };
            assert_eq!(
                pixel(0),
                [premultiply(b), premultiply(g), premultiply(r), a]
            );
            assert_eq!(pixel(4), [0, 0, 0, 0]);
            assert_eq!(pixel(8), pixel(0));
        }
    }
}
#[cfg(target_os = "linux")]
#[test]
fn wayland_guides_keep_their_interior_position_and_fractional_buffer_scale() {
    for horizontal in [true, false] {
        let mm = if horizontal { 90. } else { -100. };
        let mut bands = references(horizontal, mm, scene::GuideKind::Section);
        let b = bands
            .iter_mut()
            .find(|b| b.monitor == "B" && matches!(b.content, Content::Guide { .. }))
            .unwrap();
        for scale in [1., 1.25, 1.5, 2.] {
            b.scale = scale;
            let (logical, buffer) =
                lbm_overlay::linux::wayland_geometry(b, (scale * 120.) as u32).unwrap();
            assert_eq!(logical.thickness(), 2);
            assert_eq!(buffer.thickness(), (2. * scale).round() as i32);
            if horizontal {
                assert_eq!(logical.rect.y, 359);
            } else {
                assert_eq!(logical.rect.x, -401);
            }
            assert_eq!(buffer.content, b.content);
        }
        b.content = Content::Guide {
            at: f64::NAN,
            kind: scene::GuideKind::Middle,
        };
        assert!(lbm_overlay::linux::validate(std::slice::from_ref(b)).is_err());
    }
}

#[test]
fn scene_transport_preserves_connectors_physical_units_and_section_flags() {
    let mut bands = rulers();
    bands.extend(scene::from_layout(&layout(), Mode::Resistance, None).unwrap());
    bands.last_mut().unwrap().content =
        Content::Resistance(vec![BorderSection::new(12.5, 32., 2.5, true, 7., false)]);
    let json = serde_json::to_vec(&bands).unwrap();
    let decoded: Vec<Band> = serde_json::from_slice(&json).unwrap();
    assert_eq!(decoded, bands);
    assert_eq!(decoded[0].connector, "A");
    assert_eq!(decoded[4].connector, "B");
}

#[cfg(target_os = "linux")]
mod linux_geometry {
    use super::*;
    use lbm_overlay::linux::{server, validate, wayland_geometry, Server};
    #[test]
    fn wayland_session_never_falls_back_to_xwayland() {
        assert_eq!(server(Some("wayland"), false), Server::Wayland);
        assert_eq!(server(None, true), Server::Wayland);
        assert_eq!(server(Some("x11"), true), Server::X11);
        assert_eq!(server(None, false), Server::X11);
    }
    #[test]
    fn fractional_buffers_preserve_millimetres_and_logical_edge_placement() {
        for scale in [1., 1.25, 1.5, 1.75, 2.] {
            for mut b in rulers().into_iter().skip(4) {
                b.scale = scale;
                let (logical, buffer) = wayland_geometry(&b, (scale * 120.) as u32).unwrap();
                assert_eq!(logical.thickness(), 100);
                assert_eq!(buffer.thickness(), (100. * scale) as i32);
                assert_eq!(
                    buffer.axis_pixels(),
                    (logical.axis_pixels() as f64 * scale) as i32
                );
                assert!((buffer.pixel_at(10.) - b.pixel_at(10.) * scale).abs() < 1e-9);
                assert_eq!(buffer.cross_pixels_per_mm, b.cross_pixels_per_mm * scale);
                assert_eq!(buffer.content, b.content);
                match b.side {
                    Side::Top => assert_eq!(logical.rect.y, b.screen.y),
                    Side::Bottom => assert_eq!(
                        logical.rect.y + logical.rect.height,
                        b.screen.y + b.screen.height
                    ),
                    Side::Left => assert_eq!(logical.rect.x, b.screen.x),
                    Side::Right => assert_eq!(
                        logical.rect.x + logical.rect.width,
                        b.screen.x + b.screen.width
                    ),
                }
                assert_eq!(
                    raster::render(&buffer).unwrap().width,
                    buffer.rect.width as usize
                );
            }
        }
    }
    #[test]
    fn resistance_uses_logical_thickness_and_refuses_stale_scale() {
        let bands = scene::from_layout(&layout(), Mode::Resistance, None).unwrap();
        for band in &bands[4..] {
            let (logical, buffer) = wayland_geometry(band, 240).unwrap();
            assert_eq!(logical.thickness(), 40);
            assert_eq!(buffer.thickness(), 80);
            assert!(wayland_geometry(band, 180)
                .unwrap_err()
                .contains("scale changed"));
            assert!(wayland_geometry(band, 0).is_err());
        }
    }
    #[test]
    fn scene_validation_rejects_unbounded_or_inconsistent_native_requests() {
        let bands = rulers();
        validate(&bands).unwrap();
        assert!(validate(&[]).is_err());
        assert!(validate(&vec![bands[0].clone(); 81]).is_err());
        for cause in 0..8 {
            let mut b = bands[0].clone();
            match cause {
                0 => b.connector.clear(),
                1 => b.rect.x -= 1,
                2 => b.screen.x = i32::MAX,
                3 => b.scale = f64::NAN,
                4 => b.axis_mm = 0.,
                5 => b.cross_pixels_per_mm = f64::INFINITY,
                6 => {
                    b.content = Content::Resistance(vec![BorderSection::new(
                        20., 10., 0., false, 0., false,
                    )])
                }
                _ => {
                    b.content = Content::Ruler {
                        zero_mm: f64::NAN,
                        measured_mm: 200.,
                    }
                }
            }
            assert!(validate(&[b]).is_err(), "cause {cause}");
        }
    }
}

#[test]
fn asynchronous_backend_error_is_reported_even_when_the_scene_is_unchanged() {
    struct AsyncFake {
        trace: Rc<RefCell<Vec<&'static str>>>,
        fail: Rc<RefCell<bool>>,
    }
    impl Backend for AsyncFake {
        fn present(&mut self, _: &[Band]) -> Result<(), String> {
            self.trace.borrow_mut().push("present");
            Ok(())
        }
        fn clear(&mut self) {
            self.trace.borrow_mut().push("clear");
        }
        fn poll(&mut self) -> Result<(), String> {
            if *self.fail.borrow() {
                Err("layer-shell is unavailable".into())
            } else {
                Ok(())
            }
        }
    }
    let trace = Rc::new(RefCell::new(Vec::new()));
    let fail = Rc::new(RefCell::new(false));
    let mut manager = Manager::new(AsyncFake {
        trace: trace.clone(),
        fail: fail.clone(),
    });
    manager.sync(&rulers()).unwrap();
    *fail.borrow_mut() = true;
    assert_eq!(
        manager.sync(&rulers()).unwrap_err(),
        "layer-shell is unavailable"
    );
    assert_eq!(*trace.borrow(), ["present", "clear"]);
}
#[test]
fn four_edge_windows_are_anchored_inside_each_screen_including_negative_portrait() {
    let bands = rulers();
    assert_eq!(bands.len(), 8);
    for b in &bands {
        assert!(b.rect.x >= b.screen.x && b.rect.y >= b.screen.y);
        assert!(b.rect.x + b.rect.width <= b.screen.x + b.screen.width);
        assert!(b.rect.y + b.rect.height <= b.screen.y + b.screen.height);
        match b.side {
            Side::Top => assert_eq!(b.rect.y, b.screen.y),
            Side::Bottom => assert_eq!(b.rect.y + b.rect.height, b.screen.y + b.screen.height),
            Side::Left => assert_eq!(b.rect.x, b.screen.x),
            Side::Right => assert_eq!(b.rect.x + b.rect.width, b.screen.x + b.screen.width),
        }
    }
    assert_eq!(
        bands[4].screen,
        Pixels {
            x: -800,
            y: -300,
            width: 800,
            height: 1500
        }
    );
    assert_eq!(bands[0].thickness(), 100);
    assert_eq!(bands[4].thickness(), 200);
}
#[test]
fn dpi_changes_thickness_but_not_physical_graduation_spacing() {
    let bands = rulers();
    assert_eq!(bands[0].pixel_at(10.), 40.);
    assert_eq!(bands[4].pixel_at(10.), 40.);
    let a = scene::ticks(&bands[0]);
    assert_eq!(
        a.iter().find(|t| t.mm == 100).unwrap().label.as_deref(),
        Some("1")
    );
    assert_eq!(a.iter().find(|t| t.mm == 50).unwrap().length_mm, 15.);
    assert_eq!(
        a.iter().find(|t| t.mm == 10).unwrap().label.as_deref(),
        Some("1")
    );
    assert!(a.iter().find(|t| t.mm == 5).unwrap().label.is_none());
}
#[test]
fn rulers_project_selected_origins_and_dim_the_overhang_on_other_panels() {
    let bands = rulers();
    assert_eq!(
        bands[6].content,
        Content::Ruler {
            zero_mm: 75.,
            measured_mm: 180.
        }
    );
    let ticks = scene::ticks(&bands[6]);
    let zero = ticks.iter().find(|t| t.mm == 0).unwrap();
    assert_eq!(zero.pixel, 300.);
    assert!(zero.inside);
    assert!(!ticks.iter().find(|t| t.mm == -50).unwrap().inside);
    assert_eq!(
        ticks.iter().find(|t| t.mm == -50).unwrap().label.as_deref(),
        Some("-5")
    );
    assert!(!ticks.iter().find(|t| t.mm == 181).unwrap().inside);
    assert_eq!(
        scene::ticks(&bands[7]),
        ticks,
        "right does not reverse the millimetre axis"
    );
}
#[test]
fn resistance_intervals_clip_to_pixels_and_keep_distinct_block_colors() {
    let mut bands = scene::from_layout(&layout(), Mode::Resistance, None).unwrap();
    assert_eq!(bands[0].thickness(), 40);
    assert_eq!(bands[4].thickness(), 80);
    bands[0].content = Content::Resistance(vec![
        BorderSection::new(0., 40., 0., true, 0., true),
        BorderSection::new(40., 80., 0., true, 0., false),
        BorderSection::new(80., 120., 0., false, 0., true),
        BorderSection::new(120., 9999., 2., false, 0., false),
    ]);
    let image = raster::render(&bands[0]).unwrap();
    let pixel = |x: usize| &image.bgra[(20 * image.width + x) * 4..(20 * image.width + x) * 4 + 4];
    assert_ne!(pixel(80), pixel(240));
    assert_ne!(pixel(240), pixel(400));
    assert_ne!(pixel(400), pixel(640));
    assert_eq!(pixel(700), pixel(1200));
    for p in image.bgra.as_chunks::<4>().0.iter() {
        assert!(p[..3].iter().all(|v| *v <= p[3]));
    }
}
#[test]
fn absent_foreign_cloned_and_invalid_screens_never_produce_partial_scenes() {
    let mut l = layout();
    assert!(scene::from_layout(&l, Mode::Rulers, None).is_err());
    assert!(scene::from_layout(&l, Mode::Rulers, Some("gone")).is_err());
    l.source_kind = LayoutSource::VirtualImport;
    assert!(scene::from_layout(&l, Mode::Resistance, None).is_err());
    assert!(scene::from_layout(&l, Mode::Off, None).unwrap().is_empty());
    assert!(scene::from_layout(
        &Layout::new(LayoutOptions::default()),
        Mode::Resistance,
        None
    )
    .is_err());
    for r in [
        Rect::new(0., 0., 0., 100.),
        Rect::new(0., 0., f64::NAN, 100.),
        Rect::new(i32::MAX as f64, 0., 1280., 720.),
        Rect::new(0.5, 0., 1280., 720.),
    ] {
        assert!(Pixels::checked(r).is_err());
    }
    let mut l = Layout::new(LayoutOptions::default());
    for name in ["clone1", "clone2"] {
        add_monitor(
            &mut l,
            &LinuxMonitor {
                connector_name: name.into(),
                ..LinuxMonitor::fallback()
            },
        );
    }
    assert!(scene::from_layout(&l, Mode::Resistance, None)
        .unwrap_err()
        .contains("cloned"));
}
#[test]
fn raster_bounds_and_premultiplication_hold_for_all_four_orientations() {
    for band in rulers() {
        let image = raster::render(&band).unwrap();
        assert_eq!(image.bgra.len(), image.width * image.height * 4);
        assert!(image
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[..3].iter().all(|v| *v <= p[3])));
        assert!(image.bgra.as_chunks::<4>().0.iter().any(|p| p[3] < 255));
    }
    let mut b = rulers().remove(0);
    b.rect.width = i32::MAX;
    assert!(raster::render(&b).is_err());
}

#[test]
fn corner_miters_neither_overlap_nor_leave_holes_between_neighbouring_bands() {
    let bands = rulers();
    let top = raster::render(&bands[0]).unwrap();
    let left = raster::render(&bands[2]).unwrap();
    for y in 0..top.height {
        for x in 0..left.width {
            let a = top.bgra[(y * top.width + x) * 4 + 3] > 0;
            let b = left.bgra[(y * left.width + x) * 4 + 3] > 0;
            assert_ne!(a, b, "corner pixel {x},{y} must belong to exactly one edge");
        }
    }
}
#[derive(Default)]
struct Trace {
    calls: Vec<&'static str>,
    fail: bool,
    invalid: bool,
}
struct Fake(Rc<RefCell<Trace>>);
impl Backend for Fake {
    fn present(&mut self, _: &[Band]) -> Result<(), String> {
        let mut t = self.0.borrow_mut();
        t.calls.push("present");
        if t.fail {
            Err("partial failure".into())
        } else {
            Ok(())
        }
    }
    fn clear(&mut self) {
        self.0.borrow_mut().calls.push("clear");
    }
    fn invalidated(&mut self) -> bool {
        std::mem::take(&mut self.0.borrow_mut().invalid)
    }
}
#[test]
fn unchanged_frames_do_not_redraw_and_hiding_or_dropping_releases_every_surface() {
    let trace = Rc::new(RefCell::new(Trace::default()));
    {
        let mut m = Manager::new(Fake(trace.clone()));
        m.sync(&rulers()).unwrap();
        m.sync(&rulers()).unwrap();
        assert_eq!(trace.borrow().calls, ["present"]);
        m.sync(&[]).unwrap();
        assert_eq!(trace.borrow().calls, ["present", "clear"]);
    }
    assert_eq!(trace.borrow().calls, ["present", "clear", "clear"]);
}
#[test]
fn partial_native_failure_and_display_invalidation_clear_all_surfaces() {
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut m = Manager::new(Fake(trace.clone()));
    trace.borrow_mut().fail = true;
    assert_eq!(m.sync(&rulers()).unwrap_err(), "partial failure");
    assert_eq!(trace.borrow().calls, ["present", "clear"]);
    trace.borrow_mut().fail = false;
    m.sync(&rulers()).unwrap();
    trace.borrow_mut().invalid = true;
    assert!(m.sync(&rulers()).unwrap_err().contains("Displays changed"));
    assert_eq!(
        trace.borrow().calls,
        ["present", "clear", "present", "clear"]
    );
}

/// Runs the production rasterizer and compares to the visually inspected reference.
#[test]
fn render_preview_sheet() {
    let mut output = image::RgbaImage::from_pixel(1300, 1000, image::Rgba([235, 238, 242, 255]));
    let mut bands = rulers();
    let mut resistance = bands[0].clone();
    resistance.rect.height = 40;
    resistance.content = Content::Resistance(vec![
        BorderSection::new(20., 90., 0., true, 0., true),
        BorderSection::new(110., 160., 5., false, 0., false),
        BorderSection::new(190., 240., 0., false, 0., true),
    ]);
    bands.push(resistance);
    for (index, x, y) in [
        (0, 10, 10),
        (1, 10, 120),
        (4, 10, 230),
        (8, 10, 950),
        (6, 850, 230),
        (7, 1080, 230),
    ] {
        let img = raster::render(&bands[index]).unwrap();
        for yy in 0..img.height {
            for xx in 0..img.width {
                if xx + x >= 1300 || yy + y >= 1000 || (bands[index].side.vertical() && yy >= 700) {
                    continue;
                }
                let p = &img.bgra[(yy * img.width + xx) * 4..][..4];
                let dst = output.get_pixel_mut((xx + x) as u32, (yy + y) as u32);
                for c in 0..3 {
                    dst.0[c] =
                        (p[2 - c] as u16 + (dst.0[c] as u16 * (255 - p[3]) as u16) / 255) as u8;
                }
            }
        }
    }
    if let Some(path) = std::env::var_os("LBM_OVERLAY_PREVIEW") {
        output.save(path).unwrap();
    }
    let reference = image::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/screen-overlays.png"
    ))
    .unwrap()
    .into_rgba8();
    assert_eq!(output.dimensions(), reference.dimensions());
    assert!(
        output.as_raw() == reference.as_raw(),
        "overlay rendering differs from the inspected reference"
    );
}

#[test]
fn native_input_regions_match_visible_mitres_and_leave_reference_lines_click_through() {
    let bands = scene::from_layout(&layout(), Mode::Resistance, None).unwrap();
    for b in &bands[..4] {
        let regions = lbm_overlay::input::regions(b);
        let image = raster::render(b).unwrap();
        for y in 0..image.height {
            for x in 0..image.width {
                let hit = regions.iter().any(|r| {
                    (x as i32) >= r.x
                        && (x as i32) < r.x + r.width
                        && (y as i32) >= r.y
                        && (y as i32) < r.y + r.height
                });
                assert_eq!(
                    hit,
                    image.bgra[(y * image.width + x) * 4 + 3] > 0,
                    "{:?} {x},{y}",
                    b.side
                );
            }
        }
    }
    for b in references(true, 90., scene::GuideKind::Section) {
        if matches!(b.content, Content::Guide { .. }) {
            assert!(lbm_overlay::input::regions(&b).is_empty());
        }
    }
}
#[test]
fn physical_and_logical_native_input_agree_at_fractional_scale() {
    use lbm_overlay::input::{Input, Kind};
    let mut b = rulers()[0].clone();
    b.scale = 1.25;
    let logical = Input::new(&b, Kind::Move, (200., 20.), 1., 123, true, false);
    let mut physical = b.clone();
    physical.rect.width = (f64::from(b.rect.width) * 1.25) as i32;
    physical.rect.height = (f64::from(b.rect.height) * 1.25) as i32;
    let physical = Input::new(&physical, Kind::Move, (250., 25.), 1.25, 123, true, false);
    assert_eq!(
        (logical.at, logical.length, logical.thickness),
        (physical.at, physical.length, physical.thickness)
    );
    assert_eq!(
        serde_json::from_str::<Input>(&serde_json::to_string(&logical).unwrap()).unwrap(),
        logical
    );
}

#[test]
fn only_a_capture_lost_with_the_button_down_cancels_the_gesture() {
    use lbm_overlay::input::{Capture, Kind};
    let mut capture = Capture::default();
    // What Windows sends for a plain click: the capture ends after the button came up.
    capture.press();
    capture.release();
    assert_eq!(capture.lost(), None);
    // Taken away in the middle of a gesture, and reported once.
    capture.press();
    assert_eq!(capture.lost(), Some(Kind::Cancel));
    assert_eq!(capture.lost(), None);
}
