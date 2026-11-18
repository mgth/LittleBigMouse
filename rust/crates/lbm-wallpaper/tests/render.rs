use image::{Rgba, RgbaImage};
use lbm_layout::{
    linux::{add_monitor, LinuxMonitor},
    model::{Layout, LayoutOptions},
};
use lbm_store::wallpaper_settings::*;
use lbm_wallpaper::render::*;
fn layout() -> Layout {
    let mut l = Layout::new(LayoutOptions::default());
    for (id, x, scale, w, h) in [
        ("left", -800., 1., 800., 400.),
        ("portrait", 0., 2., 400., 800.),
    ] {
        let mut m = LinuxMonitor::fallback();
        m.connector_name = id.into();
        m.logical_x = x;
        m.logical_width = w;
        m.logical_height = h;
        m.pixel_width = (w * scale) as i32;
        m.pixel_height = (h * scale) as i32;
        m.scale = scale;
        m.width_mm = w / 2.;
        m.height_mm = h / 2.;
        add_monitor(&mut l, &m);
    }
    let ids: Vec<_> = l.monitors().iter().map(|m| m.id.clone()).collect();
    l.set_location(&ids[0], lbm_layout::geo::Point::new(-400., 0.));
    l.set_location(&ids[1], lbm_layout::geo::Point::new(0., 0.));
    l
}
#[test]
fn colors_and_previews_preserve_negative_origins_portrait_and_native_dpi() {
    let l = layout();
    let mut settings = LayoutWallpaperSettings::default();
    for m in l.monitors() {
        settings.per_screen.insert(
            m.id.clone(),
            ScreenWallpaperSettings {
                kind: ScreenWallpaperKind::Color,
                color: "#1234aB".into(),
                ..Default::default()
            },
        );
    }
    let full = paint(&l, &settings, false).unwrap();
    let preview = paint(&l, &settings, true).unwrap();
    assert_eq!(full[0].x, -800.);
    assert_eq!(full[0].pixels.dimensions(), (800, 400));
    assert_eq!(
        full[1].pixels.dimensions(),
        if cfg!(windows) {
            (400, 800)
        } else {
            (800, 1600)
        }
    );
    assert_eq!(preview[0].pixels.dimensions(), (512, 256));
    assert_eq!(preview[1].pixels.dimensions(), (256, 512));
    assert_eq!(
        preview[1].pixels.get_pixel(0, 0),
        &Rgba([0x12, 0x34, 0xab, 255])
    );
}
#[test]
fn invalid_inputs_fail_without_painting_unconfigured_screens() {
    for s in ["#ééé", "123456", "#12345g", ""] {
        assert!(color(s).is_err());
    }
    let l = layout();
    let mut settings = LayoutWallpaperSettings::default();
    assert!(paint(&l, &settings, true).is_err());
    settings.per_screen.insert(
        l.monitors()[0].id.clone(),
        ScreenWallpaperSettings {
            kind: ScreenWallpaperKind::Color,
            ..Default::default()
        },
    );
    assert_eq!(paint(&l, &settings, true).unwrap().len(), 1);
    assert!(one(
        RgbaImage::new(1, 1),
        WallpaperStyle::Fill,
        100_000,
        100_000,
        [0, 0, 0, 255]
    )
    .is_err());
    assert_eq!(
        one(
            RgbaImage::new(1, 16000),
            WallpaperStyle::Fill,
            4096,
            2160,
            [0, 0, 0, 255]
        )
        .unwrap()
        .dimensions(),
        (4096, 2160)
    );
    let d = tempfile::tempdir().unwrap();
    assert!(decode(d.path()).is_err());
    std::fs::write(d.path().join("bad.png"), "not an image").unwrap();
    assert!(decode(&d.path().join("bad.png")).is_err());
}
#[test]
fn styles_use_background_and_resolve_transparency() {
    for style in [
        WallpaperStyle::Fill,
        WallpaperStyle::Fit,
        WallpaperStyle::Stretch,
        WallpaperStyle::Tile,
        WallpaperStyle::Center,
    ] {
        let out = one(
            RgbaImage::from_pixel(2, 1, Rgba([250, 20, 30, 0])),
            style,
            8,
            6,
            [10, 30, 50, 255],
        )
        .unwrap();
        assert_eq!(out.dimensions(), (8, 6));
        assert!(out.pixels().all(|p| p.0 == [10, 30, 50, 255]));
    }
}
#[test]
fn panorama_uses_physical_screen_positions_and_supports_unicode_paths() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("été panorama.png");
    RgbaImage::from_fn(120, 80, |x, _| {
        if x < 80 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 255])
        }
    })
    .save(&path)
    .unwrap();
    let l = layout();
    let settings = LayoutWallpaperSettings {
        mode: WallpaperMode::Span,
        span_image_path: Some(path.to_string_lossy().into()),
        ..Default::default()
    };
    let p = paint(&l, &settings, true).unwrap();
    assert_eq!(p.len(), 2);
    assert_ne!(
        p[0].pixels.get_pixel(10, 10),
        p[1].pixels.get_pixel(p[1].pixels.width() - 10, 10)
    );
    RgbaImage::from_pixel(120, 80, Rgba([255, 0, 0, 0]))
        .save(&path)
        .unwrap();
    let transparent = paint(&l, &settings, true).unwrap();
    assert!(transparent
        .iter()
        .all(|p| p.pixels.pixels().all(|c| c.0 == [0, 0, 0, 255])));
}

#[test]
fn custom_panorama_framing_survives_serialization_and_pads_uncovered_screens() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("panorama.png");
    RgbaImage::from_pixel(120, 80, Rgba([240, 30, 40, 255]))
        .save(&path)
        .unwrap();
    let l = layout();
    let settings = LayoutWallpaperSettings {
        mode: WallpaperMode::Span,
        span_image_path: Some(path.to_string_lossy().into()),
        span_placement: Some(SpanPlacement {
            scale: 0.5,
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview = paint(&l, &settings, true).unwrap();
    let full = paint(&l, &settings, false).unwrap();
    for pictures in [&preview, &full] {
        assert!(pictures
            .iter()
            .any(|p| p.pixels.pixels().any(|c| c.0 == [0, 0, 0, 255])));
        assert!(pictures
            .iter()
            .any(|p| p.pixels.pixels().any(|c| c.0 == [240, 30, 40, 255])));
    }
    let store = d.path().join("wallpaper.json");
    lbm_store::wallpaper_settings::update(&store, "test", settings.clone()).unwrap();
    assert_eq!(
        lbm_store::wallpaper_settings::load_checked(&store).unwrap()["test"],
        settings
    );
    let mut shifted = settings;
    shifted.span_placement.as_mut().unwrap().offset_x_mm = 2000.;
    assert!(paint(&l, &shifted, true)
        .unwrap()
        .iter()
        .all(|p| p.pixels.pixels().all(|c| c.0 == [0, 0, 0, 255])));
    shifted.span_placement.as_mut().unwrap().scale = f64::NAN;
    assert!(paint(&l, &shifted, true).is_err());
}

#[test]
fn moving_a_panel_recalculates_the_saved_panorama_slices() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gradient.png");
    RgbaImage::from_fn(600, 400, |x, y| {
        Rgba([(x * 255 / 600) as u8, (y * 255 / 400) as u8, 80, 255])
    })
    .save(&path)
    .unwrap();
    let mut l = layout();
    let original = l.clone();
    let settings = LayoutWallpaperSettings {
        mode: WallpaperMode::Span,
        span_image_path: Some(path.to_string_lossy().into()),
        span_placement: Some(SpanPlacement {
            scale: 1.3,
            offset_x_mm: 25.,
            offset_y_mm: -12.,
        }),
        ..Default::default()
    };
    let before = paint(&l, &settings, false).unwrap();
    let id = l.monitors()[1].id.clone();
    l.offset_location(&id, lbm_layout::geo::Vector::new(100., 40.));
    let after = paint(&l, &settings, false).unwrap();
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.pixels.dimensions(), b.pixels.dimensions());
        assert_ne!(a.pixels, b.pixels);
    }
    let restored = paint(&original, &settings, false).unwrap();
    assert_eq!(restored[0].pixels, before[0].pixels);
}
