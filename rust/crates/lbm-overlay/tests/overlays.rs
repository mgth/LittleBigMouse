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
