use lbm_agent::{
    desktop::ScreenWallpaper,
    wallpaper_job::Job,
    windows_wallpaper::{self, Output, Shell},
};
use lbm_store::wallpaper_settings::{
    self, LayoutWallpaperSettings, ScreenWallpaperKind, ScreenWallpaperSettings, WallpaperStyle,
};
use std::{
    cell::{Cell, RefCell},
    sync::{atomic::AtomicU64, Arc},
};
struct Fake {
    outputs: RefCell<Vec<Output>>,
    position: Cell<i32>,
    writes: RefCell<Vec<(String, String)>>,
    fail: Cell<bool>,
    reread_changed: bool,
    reads: Cell<u32>,
}
impl Fake {
    fn new(position: i32) -> Self {
        Self {
            position: Cell::new(position),
            outputs: RefCell::new(vec![
                Output {
                    id: "L".into(),
                    x: -4,
                    y: 0,
                    width: 4,
                    height: 6,
                    wallpaper: "old-left".into(),
                },
                Output {
                    id: "R".into(),
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 4,
                    wallpaper: "old-right".into(),
                },
            ]),
            writes: Default::default(),
            fail: Cell::new(false),
            reread_changed: false,
            reads: Cell::new(0),
        }
    }
}
impl Shell for Fake {
    fn outputs(&self) -> Result<Vec<Output>, String> {
        let mut out = self.outputs.borrow().clone();
        self.reads.set(self.reads.get() + 1);
        if self.reread_changed && self.reads.get() > 1 {
            out[0].width += 1;
        }
        Ok(out)
    }
    fn position(&self) -> Result<i32, String> {
        Ok(self.position.get())
    }
    fn set_position(&self, p: i32) -> Result<(), String> {
        self.position.set(p);
        Ok(())
    }
    fn set_wallpaper(&self, id: &str, path: &str) -> Result<(), String> {
        self.writes.borrow_mut().push((id.into(), path.into()));
        if id == "R" && self.fail.replace(false) {
            return Err("injected failure".into());
        }
        self.outputs
            .borrow_mut()
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .wallpaper = path.into();
        Ok(())
    }
}
fn screens() -> Vec<ScreenWallpaper> {
    vec![
        ScreenWallpaper {
            x: -4.,
            y: 0.,
            expected_size: Some((4, 6)),
            image: None,
            style: WallpaperStyle::Fill,
            color: "#123456".into(),
        },
        ScreenWallpaper {
            x: 0.,
            y: 0.,
            expected_size: Some((8, 4)),
            image: None,
            style: WallpaperStyle::Fit,
            color: "#abcdef".into(),
        },
    ]
}
#[test]
fn partial_updates_preserve_other_screens_and_the_global_style() {
    let dir = tempfile::tempdir().unwrap();
    let shell = Fake::new(4);
    windows_wallpaper::apply_with(&shell, &screens()[..1], dir.path()).unwrap();
    assert_eq!(shell.position.get(), 4);
    assert_eq!(shell.writes.borrow().len(), 1);
    // L is enumerated first and R is the primary: the shell paints R's slot on L.
    let output = shell.outputs.borrow();
    assert_eq!(output[0].wallpaper, "old-left");
    let image = image::open(&output[1].wallpaper).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (4, 6));
    assert_eq!(image.get_pixel(0, 0).0, [0x12, 0x34, 0x56, 255]);
}
/// `outputs` holds what each slot's own path files; which monitor shows it is the paint
/// order: the primary first, then the others in filing order (mgth/LittleBigMouse#789).
fn shown_on(shell: &Fake, id: &str) -> image::RgbaImage {
    let outputs = shell.outputs.borrow();
    let primary = outputs.iter().position(|o| o.x == 0 && o.y == 0);
    let painted = lbm_wallpaper::slots::paint_order(primary, outputs.len());
    let shown = outputs.iter().position(|o| o.id == id).unwrap();
    let slot = painted.iter().position(|&j| j == shown).unwrap();
    image::open(&outputs[slot].wallpaper).unwrap().to_rgba8()
}
#[test]
fn every_screen_shows_its_own_image_when_the_primary_is_not_first() {
    let dir = tempfile::tempdir().unwrap();
    let shell = Fake::new(4);
    windows_wallpaper::apply_with(&shell, &screens(), dir.path()).unwrap();
    assert_eq!(
        shell
            .writes
            .borrow()
            .iter()
            .map(|w| w.0.as_str())
            .collect::<Vec<_>>(),
        ["R", "L"]
    );
    assert_eq!(shown_on(&shell, "L").dimensions(), (4, 6));
    assert_eq!(
        shown_on(&shell, "L").get_pixel(0, 0).0,
        [0x12, 0x34, 0x56, 255]
    );
    assert_eq!(shown_on(&shell, "R").dimensions(), (8, 4));
    assert_eq!(
        shown_on(&shell, "R").get_pixel(0, 0).0,
        [0xab, 0xcd, 0xef, 255]
    );
}
#[test]
fn a_primary_enumerated_first_is_filed_under_its_own_path() {
    let dir = tempfile::tempdir().unwrap();
    let shell = Fake::new(4);
    shell.outputs.borrow_mut().reverse();
    windows_wallpaper::apply_with(&shell, &screens(), dir.path()).unwrap();
    let outputs = shell.outputs.borrow();
    assert_eq!(outputs[0].id, "R");
    assert_eq!(
        image::open(&outputs[0].wallpaper)
            .unwrap()
            .to_rgba8()
            .dimensions(),
        (8, 4)
    );
    assert_eq!(
        image::open(&outputs[1].wallpaper)
            .unwrap()
            .to_rgba8()
            .dimensions(),
        (4, 6)
    );
}
#[test]
fn spanning_requires_every_screen_and_failures_restore_previous_state() {
    let dir = tempfile::tempdir().unwrap();
    for position in [1, 5] {
        let shell = Fake::new(position);
        assert!(windows_wallpaper::apply_with(&shell, &screens()[..1], dir.path()).is_err());
        assert!(shell.writes.borrow().is_empty());
        assert_eq!(shell.position.get(), position);
        shell.fail.set(true);
        let before = shell.outputs.borrow().clone();
        let error = windows_wallpaper::apply_with(&shell, &screens(), dir.path()).unwrap_err();
        assert!(error.contains("restored"));
        assert_eq!(*shell.outputs.borrow(), before);
        assert_eq!(shell.position.get(), position);
        windows_wallpaper::apply_with(&shell, &screens(), dir.path()).unwrap();
        assert_eq!(shell.position.get(), 2);
    }
}
#[test]
fn geometry_changes_and_invalid_later_images_are_refused_before_any_mutation() {
    let dir = tempfile::tempdir().unwrap();
    for cause in 0..4 {
        let mut shell = Fake::new(4);
        let mut s = screens();
        match cause {
            0 => s[1].expected_size = Some((9, 9)),
            1 => s[1].image = Some(dir.path().join("missing.png")),
            2 => shell.reread_changed = true,
            _ => s[1].x = -4.,
        }
        assert!(windows_wallpaper::apply_with(&shell, &s, dir.path()).is_err());
        assert!(shell.writes.borrow().is_empty());
    }
}
fn job(dir: &std::path::Path) -> Job {
    use lbm_layout::{
        linux::{add_monitor, LinuxMonitor},
        model::{Layout, LayoutOptions},
    };
    let mut layout = Layout::new(LayoutOptions::default());
    let mut m = LinuxMonitor::fallback();
    m.logical_width = 8.;
    m.logical_height = 4.;
    m.pixel_width = 8;
    m.pixel_height = 4;
    add_monitor(&mut layout, &m);
    layout.id = "test".into();
    let mut settings = LayoutWallpaperSettings::default();
    settings.per_screen.insert(
        layout.monitors()[0].id.clone(),
        ScreenWallpaperSettings {
            kind: ScreenWallpaperKind::Color,
            ..Default::default()
        },
    );
    Job {
        layout,
        settings,
        store: dir.join("wallpaper.json"),
        images: dir.join("images"),
    }
}
#[test]
fn jobs_validate_before_persisting_and_never_replace_corrupt_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wallpaper.json");
    assert!(job(dir.path())
        .prepare(Arc::new(AtomicU64::new(2)), 1)
        .is_err());
    assert!(!path.exists());
    std::fs::write(&path, "bad json").unwrap();
    assert!(job(dir.path())
        .prepare(Arc::new(AtomicU64::new(1)), 1)
        .is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "bad json");
    std::fs::remove_file(&path).unwrap();
    wallpaper_settings::update(&path, "other", Default::default()).unwrap();
    let prepared = job(dir.path())
        .prepare(Arc::new(AtomicU64::new(1)), 1)
        .unwrap();
    assert_eq!(prepared.len(), 1);
    assert!(prepared[0].image.as_ref().unwrap().is_file());
    let all = wallpaper_settings::load_checked(&path).unwrap();
    assert!(all.contains_key("other"));
    assert!(all.contains_key("test"));
}

#[test]
fn panorama_job_persists_and_reuses_the_custom_framing() {
    use lbm_store::wallpaper_settings::{SpanPlacement, WallpaperMode};
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.png");
    image::RgbaImage::from_pixel(8, 4, image::Rgba([220, 40, 80, 255]))
        .save(&source)
        .unwrap();
    let mut task = job(dir.path());
    task.settings.mode = WallpaperMode::Span;
    task.settings.span_image_path = Some(source.to_string_lossy().into_owned());
    task.settings.span_placement = Some(SpanPlacement {
        scale: 0.5,
        ..Default::default()
    });
    let layout = task.layout.clone();
    let settings = task.settings.clone();
    let applied = task.prepare(Arc::new(AtomicU64::new(1)), 1).unwrap();
    let saved = wallpaper_settings::load_checked(&dir.path().join("wallpaper.json")).unwrap();
    assert_eq!(saved["test"], settings);
    let restored =
        lbm_agent::wallpaper::screens(&layout, &saved["test"], &dir.path().join("images"));
    assert_eq!(restored[0].image, applied[0].image);
    let pixels = image::open(applied[0].image.as_ref().unwrap())
        .unwrap()
        .into_rgba8();
    assert!(pixels.pixels().any(|p| p.0 == [0, 0, 0, 255]));
    assert!(pixels.pixels().any(|p| p.0 == [220, 40, 80, 255]));
}
