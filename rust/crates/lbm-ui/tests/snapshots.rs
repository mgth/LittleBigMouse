//! What the views actually look like.
//!
//! Every other test in this crate asks the **accessibility tree** — which is how `lbm-ui`
//! can be tested at all on a Wayland machine, and which is genuinely the right question
//! for "is the name there", "did the click land", "is Save disabled". It is the wrong
//! question for "does this look like anything". A view can have a perfect tree and paint
//! white on white, put a label under a panel, or lose its background: the tree says all
//! is well because, to the tree, all is.
//!
//! So these render. `egui_kittest` draws through wgpu and compares against a committed
//! PNG; a view that changes shape fails until someone looks at the diff and says yes.
//!
//! **Linux only, and not by taste.** The renderer is wgpu, and wgpu cannot enter this
//! workspace's dependency graph on Windows — see the note on the target-gated
//! dev-dependency in `Cargo.toml`. On Windows this file compiles to nothing.
//!
//! The references are generated **on CI's renderer**, not on a developer's. Software
//! rasterization (lavapipe) and a real GPU do not antialias identically, so a PNG made
//! here would fail there and vice versa; `UPDATE_SNAPSHOTS=1` regenerates, and the
//! workflow uploads what it rendered when a comparison fails.
#![cfg(not(windows))]

use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::frame::{self, Ratio};
use lbm_ui::map::{self, MapMonitor};
use lbm_ui::mode::Mode;

/// A window of a stated size, with a background — both on purpose.
///
/// The size, because the fit follows the window: a harness left at its default would make
/// every reference depend on that default.
///
/// The `CentralPanel`, because **`Harness::render` paints on nothing.** The `Ui` handed to
/// `eframe::App::ui` has no background of its own and the window wraps it in a panel
/// (`main.rs`); a snapshot taken without one renders white-on-transparent, which composites
/// into whatever the viewer happens to put behind it. The first run of these tests produced
/// exactly that — a map that looked like black rectangles on white paper, and a stray dark
/// block that was nothing but premultiplied alpha. Picture and app must be the same thing
/// or the reference is pinning a fiction.
fn harness<'a>(size: (f32, f32), mut app: impl FnMut(&mut egui::Ui) + 'a) -> Harness<'a> {
    Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .build_ui(move |ui| {
            lbm_ui::theme::install(ui.ctx());
            egui::CentralPanel::default().show(ui, &mut app);
        })
}

/// The two screens the other map tests use, with the 20 mm bezel the oracle's desktops
/// really have.
fn two_screens() -> Vec<MapMonitor<'static>> {
    vec![
        MapMonitor {
            id: "left",
            name: "Left screen",
            mm_outside: Rect::new(0.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(20.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
            details: &[],
            probe: None,
        },
        MapMonitor {
            id: "right",
            name: "Right screen",
            mm_outside: Rect::new(700.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(720.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
            details: &[],
            probe: None,
        },
    ]
}

/// The two toolbars together with their canvas, at desktop and compact widths.
/// Individual component snapshots cannot catch wrapping that consumes the map.
#[test]
fn studio_workspace() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (width, height, name) in [
        (1120.0, 720.0, "workspace-wide"),
        (560.0, 600.0, "workspace-narrow"),
    ] {
        let mut logos = None;
        let state = lbm_ui::State {
            engine: lbm_ui::Engine::Running,
            hook_connected: true,
            saved: false,
            ..Default::default()
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(width, height))
            .build_ui(|ui| {
                lbm_ui::theme::install(ui.ctx());
                let logos = logos.get_or_insert_with(|| {
                    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets");
                    let (catalogue, _) = lbm_icons::catalogue(&assets);
                    ["PHL", "SAM"].map(|code| {
                        let path = format!("icon/pnp/{code}");
                        let source =
                            std::fs::read(lbm_icons::resolve(&catalogue, &path).unwrap()).unwrap();
                        let color =
                            frame::logo_colour(&ui.ctx().style_of(egui::Theme::Dark).visuals);
                        let tree = lbm_icons::load(
                            &source,
                            lbm_icons::Rgba(color.r(), color.g(), color.b(), color.a()),
                        )
                        .unwrap();
                        let logo = lbm_icons::render_logo(&tree, 512).unwrap();
                        ui.ctx().load_texture(
                            path,
                            egui::ColorImage::from_rgba_premultiplied(logo.size, &logo.pixels),
                            egui::TextureOptions::LINEAR,
                        )
                    })
                });
                let mut screens = two_screens();
                for ((screen, logo), name) in screens
                    .iter_mut()
                    .zip(logos.iter())
                    .zip(["PHL 328P6VU", "Odyssey G80SD"])
                {
                    screen.name = name;
                    screen.logo = Some(logo);
                }
                egui::Panel::top("navigation")
                    .frame(lbm_ui::theme::toolbar())
                    .show(ui, |ui| {
                        lbm_ui::mode::toolbar(
                            ui,
                            lbm_ui::mode::Mode::Default,
                            &lbm_ui::mode::Offered::default(),
                            false,
                        );
                    });
                egui::Panel::bottom("engine")
                    .frame(lbm_ui::theme::toolbar())
                    .show(ui, |ui| {
                        lbm_ui::bottom_bar_with_agent(ui, &state, |ui| {
                            lbm_ui::theme::status(ui, "Agent connected", lbm_ui::theme::ACCENT);
                        });
                    });
                lbm_ui::properties::panel(ui, |ui| {
                    ui.heading("Screens");
                    let mut selected = Some("left".to_owned());
                    let mut single = false;
                    lbm_ui::properties::monitor_selector(
                        ui,
                        &mut selected,
                        &[("left", "PHL 328P6VU"), ("right", "Odyssey G80SD")],
                        &mut single,
                    );
                });
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(lbm_ui::theme::BACKGROUND)
                            .inner_margin(14),
                    )
                    .show(ui, |ui| {
                        let fit = map::fit(map::extent(&screens), ui.max_rect());
                        map::draw(ui, &screens, &fit, Some("left"));
                    });
            });
        harness.run();
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

#[test]
fn settings_cards() {
    use lbm_ui::options::{self, Page};
    let mut results = egui_kittest::SnapshotResults::new();
    for (page, name) in [
        (Page::General, "settings-cards"),
        // The settings only Windows acts on: elevation and the update check.
        (Page::General, "settings-windows"),
        (Page::Mouse, "settings-mouse"),
        (Page::Layout, "settings-layout"),
        (Page::Applications, "settings-applications"),
        (Page::Advanced, "settings-advanced"),
        (Page::Layout, "settings-narrow"),
    ] {
        let mut page = page;
        let mut options = lbm_layout::model::LayoutOptions::default();
        let mut recording = lbm_ui::shortcut::Recording::default();
        let mut open = true;
        let mut pattern = String::new();
        let size = if name == "settings-narrow" {
            (360.0, 640.0)
        } else {
            (720.0, 760.0)
        };
        let mut harness = harness(size, |ui| {
            let bounds = ui.max_rect();
            let screens = two_screens();
            let fit = map::fit(map::extent(&screens), bounds);
            map::draw(ui, &screens, &fit, None);
            options::drawer(ui.ctx(), bounds, &mut open, &mut page, true, |ui, page| {
                if page == Page::Applications {
                    lbm_ui::excluded::panel(
                        ui,
                        Some(&[
                            "game.exe".into(),
                            "/home/user/Games/Steam/steamapps/common/Example Game/bin/game".into(),
                        ]),
                        &mut pattern,
                        &["/usr/bin/example".into()],
                    );
                } else {
                    let windows = name == "settings-windows";
                    options::panel(ui, &mut options, windows, &mut recording, None, page);
                }
            });
        });
        harness.run();
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

/// The bar, running, with a screen selected — the state a user spends their time in.
#[test]
fn the_bottom_bar_running() {
    let state = lbm_ui::State {
        engine: lbm_ui::Engine::Running,
        hook_connected: true,
        ..Default::default()
    };
    let mut harness = harness((520.0, 60.0), |ui| {
        lbm_ui::bottom_bar(ui, &state);
    });
    harness.run();
    harness.snapshot("bottom-bar-running");
}

/// And with no agent: **every button disabled**. The one state the tree cannot vouch for
/// on its own — "disabled" in the tree is a flag, on screen it is a colour, and a theme
/// that greyed nothing would read as an app that simply does not respond.
#[test]
fn the_bottom_bar_without_an_agent() {
    let state = lbm_ui::State::default();
    let mut harness = harness((520.0, 60.0), |ui| {
        lbm_ui::bottom_bar(ui, &state);
    });
    harness.run();
    harness.snapshot("bottom-bar-no-agent");
}

/// The map: two screens, one selected. The bezels, the fit, and the names.
///
/// This reference is the one that settled the legibility floor. It used to show **no
/// names at all** — the name is half the top bezel, which at this very ordinary window
/// size lands near 4 points, and the 7 pt floor dropped it. Seeing that the common case
/// was the one the floor removed is what made the maintainer take the floor out
/// (2026-09-16). The names here are small; they are also there.
#[test]
fn the_map_with_two_screens() {
    let screens = two_screens();
    let mut harness = harness((900.0, 600.0), |ui| {
        let fit = map::fit(map::extent(&screens), ui.max_rect());
        map::draw(ui, &screens, &fit, Some("left"));
    });
    harness.run();
    harness.snapshot("map-two-screens");
}

#[test]
fn crossings_on_the_map_and_in_the_list() {
    use lbm_ui::probe::{Edge, Overlay, Run, Side};
    let edges = [
        Edge {
            side: Side::Right,
            runs: vec![
                Run {
                    from: 0,
                    to: 539,
                    target: None,
                },
                Run {
                    from: 540,
                    to: 1079,
                    target: Some(7),
                },
            ],
        },
        Edge {
            side: Side::Bottom,
            runs: vec![Run {
                from: 0,
                to: 1919,
                target: None,
            }],
        },
    ];
    let names = [(7, "Right screen".to_owned())];
    let mut screens = two_screens();
    screens[0].probe = Some(Overlay {
        pixels: Rect::new(0.0, 0.0, 1920.0, 1080.0),
        edges: &edges,
        names: &names,
    });
    let mut map = harness((900.0, 600.0), |ui| {
        let fit = map::fit(map::extent(&screens), ui.max_rect());
        map::draw(ui, &screens, &fit, Some("left"));
    });
    map.run();
    map.snapshot("map-crossings");
    let mut list = harness((900.0, 600.0), |ui| {
        lbm_ui::list::draw(ui, ui.max_rect(), &screens, Some("left"));
    });
    list.run();
    list.snapshot("list-crossings");
    let mut results = map.take_snapshot_results();
    results.extend(list.take_snapshot_results());
}

/// One frame. The name is **half the top bezel**, printed on the plastic — so its size
/// follows the plastic and not the panel, and on this 20 mm bezel at this ratio it comes
/// out around 6 points. Small, and drawn: there is no floor any more.
#[test]
fn a_monitor_frame_with_its_name() {
    let outside = Rect::new(0.0, 0.0, 640.0, 380.0);
    let content = Rect::new(20.0, 20.0, 600.0, 340.0);
    let drawn = frame::draw(outside, content, (0.0, 0.0), Ratio { x: 0.6, y: 0.6 });
    let mut harness = harness((420.0, 260.0), |ui| {
        frame::monitor(
            ui,
            &drawn,
            &frame::Look {
                name: "Odyssey G80SD",
                ..Default::default()
            },
        );
    });
    harness.run();
    harness.snapshot("monitor-frame");
}

/// The mode bar: what is offered and what is not.
///
/// The tree test asserts which buttons are disabled; only a render says whether *disabled*
/// looks disabled. Two of the seven are greyed here, which is the honest picture of the
/// port — and the reason the modes that are not ported yet are shown rather than dropped.
#[test]
fn the_mode_bar() {
    let mut harness = harness((760.0, 60.0), |ui| {
        lbm_ui::mode::bar(ui, lbm_ui::mode::Mode::Location, &Default::default());
    });
    harness.run();
    harness.snapshot("mode-bar");
}

#[test]
fn size_and_info_properties_stay_readable_beside_or_below_the_map() {
    use lbm_ui::properties::{self, Size};
    let size = Size {
        physical: lbm_layout::model::DisplaySize::from_rect(Rect::new(0.0, 0.0, 600.0, 340.0))
            .with_borders(lbm_layout::geo::Thickness {
                left: 10.0,
                top: 11.0,
                right: 12.0,
                bottom: 13.0,
            }),
        locked: true,
        per_monitor_borders: true,
        model: "SAM1234".into(),
    };
    let rows = vec![
        ("Discovery".into(), "kscreen".into()),
        ("ConnectorName".into(), "DP-1".into()),
        ("PixelWidth".into(), "3840".into()),
        ("Edid / Model".into(), "Studio display".into()),
        ("Edid / SerialNumber".into(), "Not reported".into()),
        (
            "InterfacePath".into(),
            "a-long-device-path-that-must-stay-readable-without-expanding-the-properties-panel"
                .into(),
        ),
    ];
    let screens = two_screens();
    let mut results = egui_kittest::SnapshotResults::new();
    for (width, height, info, name) in [
        (1100.0, 650.0, false, "size-properties-wide"),
        (480.0, 440.0, false, "size-properties-narrow"),
        (1100.0, 650.0, true, "info-properties"),
    ] {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(width, height))
            .build_ui(|ui| {
                lbm_ui::theme::install(ui.ctx());
                properties::panel(ui, |ui| {
                    lbm_ui::mode::heading(ui, if info { Mode::Info } else { Mode::Size });
                    ui.label("Left screen");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if info {
                            lbm_ui::icons::title(
                                ui,
                                lbm_ui::icons::Icon::Chip,
                                "Detected · last display refresh",
                            );
                            properties::rows(ui, &rows);
                        } else {
                            properties::size(ui, "left", &size);
                        }
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    let fit = map::fit(map::extent(&screens), ui.max_rect());
                    map::draw(ui, &screens, &fit, Some("left"));
                });
            });
        harness.run();
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

#[test]
fn system_displays_include_a_detached_screen_and_a_concrete_confirmation() {
    use lbm_ipc::displays::{Change, Output, Outputs};
    let outputs = Outputs {
        signature: "test".into(),
        outputs: vec![
            Output {
                connector: "DP-1".into(),
                name: "Main screen".into(),
                enabled: true,
                primary: true,
            },
            Output {
                connector: "HDMI-1".into(),
                name: "Side screen".into(),
                enabled: false,
                primary: false,
            },
        ],
    };
    let mut results = egui_kittest::SnapshotResults::new();
    for (width, height, name) in [
        (1100., 780., "system-displays"),
        (480., 640., "system-displays-narrow"),
    ] {
        let mut panel = lbm_ui::displays::Panel {
            selected: Some("HDMI-1".into()),
            confirm: Some(("HDMI-1".into(), Change::Attach)),
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(width, height))
            .build_ui(|ui| {
                lbm_ui::theme::install(ui.ctx());
                lbm_ui::properties::panel(ui, |ui| {
                    lbm_ui::mode::heading(ui, Mode::Location);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label("Side screen · HDMI-1");
                        lbm_ui::icons::title(ui, lbm_ui::icons::Icon::Move, "Physical position");
                        lbm_ui::properties::location(
                            ui,
                            "right",
                            &lbm_ui::properties::Location {
                                x: 0.,
                                y: 0.,
                                scale_x: 100.,
                                scale_y: 100.,
                            },
                        );
                        ui.add_space(12.);
                        ui.separator();
                        panel.draw(ui, Some(&outputs), None, false, true, false);
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    let mut screens = two_screens();
                    screens[0].name = "Main screen";
                    screens[1].name = "Side screen";
                    let fit = map::fit(map::extent(&screens), ui.max_rect());
                    map::draw(ui, &screens, &fit, Some("right"));
                });
            });
        harness.run();
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

/// The Location mode, in a frame big enough to hold its rows.
///
/// The rows are **not** scaled with the frame — the name above them is, and the two are
/// deliberately different: in C# the mode's content is an ordinary control at the app's
/// font size. A screen drawn small gets fewer rows, never smaller ones.
#[test]
fn a_monitor_frame_in_the_location_mode() {
    let outside = Rect::new(0.0, 0.0, 640.0, 380.0);
    let content = Rect::new(20.0, 20.0, 600.0, 340.0);
    let drawn = frame::draw(outside, content, (0.0, 0.0), Ratio { x: 0.6, y: 0.6 });
    let rows = [
        ("at", "40, 20 mm".to_owned()),
        ("size", "600 × 340 mm".to_owned()),
        ("pixels", "3840 × 2160".to_owned()),
        ("dpi", "163 × 161".to_owned()),
        ("pnp", "SAM".to_owned()),
    ];
    let mut harness = harness((420.0, 260.0), |ui| {
        frame::monitor(
            ui,
            &drawn,
            &frame::Look {
                name: "Odyssey G80SD",
                details: &rows,
                ..Default::default()
            },
        );
    });
    harness.run();
    harness.snapshot("monitor-frame-location");
}

#[test]
fn resistance_values_and_bands_stay_readable_in_map_and_list() {
    use lbm_layout::model::{BorderResistance, BorderSection};
    use lbm_ui::resistance::{Editor, Monitor, Selection, Side};
    let screens = two_screens();
    let mut resistance = BorderResistance::default();
    resistance.right.sections = vec![
        BorderSection::new(30.0, 130.0, 0.0, true, 8.0, false),
        BorderSection::new(170.0, 300.0, 0.0, true, 0.0, true),
    ];
    resistance.top.sections = vec![BorderSection::new(60.0, 320.0, 6.0, false, 0.0, false)];
    resistance.bottom.sections = vec![BorderSection::new(200.0, 480.0, 0.0, false, 0.0, true)];
    let monitors: Vec<_> = screens
        .iter()
        .map(|s| Monitor {
            id: s.id.into(),
            bounds: s.mm_content,
            excluded: false,
            resistance: resistance.clone(),
        })
        .collect();
    let mut results = egui_kittest::SnapshotResults::new();
    for (width, height, list, name) in [
        (1100.0, 720.0, false, "resistance-map"),
        (1100.0, 720.0, true, "resistance-list"),
        (480.0, 620.0, false, "resistance-narrow"),
    ] {
        let mut editor = Editor::default();
        editor.selected = Some(Selection {
            monitor: "left".into(),
            side: Side::Right,
            index: 0,
        });
        let mut harness = Harness::builder()
            .with_size(egui::vec2(width, height))
            .build_ui(|ui| {
                lbm_ui::theme::install(ui.ctx());
                lbm_ui::properties::panel(ui, |ui| {
                    lbm_ui::mode::heading(ui, Mode::Resistance);
                    lbm_ui::resistance::defaults(ui, &mut editor.defaults);
                    ui.label("Left screen");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        editor.panel(ui, &monitors, "left", 200.0);
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    let placements = if list {
                        lbm_ui::list::draw(ui, ui.max_rect(), &screens, Some("left"));
                        let fit = map::fit_in(
                            lbm_ui::list::screen_rect(ui.max_rect()),
                            screens[0].mm_outside,
                        );
                        vec![("left".into(), fit.place(&screens[0]).content)]
                    } else {
                        let fit = map::fit(map::extent(&screens), ui.max_rect());
                        map::draw(ui, &screens, &fit, Some("left"));
                        screens
                            .iter()
                            .map(|s| (s.id.into(), fit.place(s).content))
                            .collect()
                    };
                    editor.bands(ui, &monitors, &placements);
                });
            });
        harness.run();
        harness.snapshot(name);
        results.extend(harness.take_snapshot_results());
    }
}

#[test]
fn resistance_reference_reaches_the_offset_screen_and_disappears_with_ctrl() {
    use lbm_layout::model::{BorderResistance, BorderSection};
    use lbm_ui::resistance::{Editor, Monitor, SnapKind};
    use std::cell::RefCell;
    let mut screens = two_screens();
    screens[1].mm_outside = Rect::new(700., 40., 640., 380.);
    screens[1].mm_content = Rect::new(720., 60., 600., 340.);
    let mut monitors: Vec<_> = screens
        .iter()
        .map(|s| Monitor {
            id: s.id.into(),
            bounds: s.mm_content,
            excluded: false,
            resistance: BorderResistance::default(),
        })
        .collect();
    monitors[1]
        .resistance
        .left
        .sections
        .push(BorderSection::new(100., 200., 0., true, 0., false));
    let editor = RefCell::new(Editor::default());
    let source = RefCell::new(egui::Rect::NOTHING);
    let mut h = harness((1100., 550.), |ui| {
        ui.label("Snap guides: white = midpoint · cyan = screen edge · yellow = section boundary");
        let fit = map::fit(map::extent(&screens), ui.available_rect_before_wrap());
        map::draw(ui, &screens, &fit, Some("left"));
        let placements: Vec<_> = screens
            .iter()
            .map(|s| (s.id.to_owned(), fit.place(s).content))
            .collect();
        *source.borrow_mut() = placements[0].1;
        editor.borrow_mut().bands(ui, &monitors, &placements);
    });
    h.run();
    let r = *source.borrow();
    let at = |mm: f32| egui::pos2(r.right() - 6., r.top() + mm / 340. * r.height());
    h.drag_at(at(70.));
    h.step();
    h.hover_at(at(138.));
    h.step();
    assert_eq!(editor.borrow().guide().unwrap().kind, SnapKind::Section);
    assert_eq!(editor.borrow().guide().unwrap().layout_mm, 160.);
    h.snapshot("resistance-reference-map");
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
    h.step();
    assert!(editor.borrow().guide().is_none());
    h.snapshot("resistance-reference-ctrl");
}

#[test]
fn resistance_mitres_and_short_caps_at_two_scales() {
    use lbm_layout::model::{BorderResistance, BorderSection};
    use lbm_ui::resistance::{Editor, Monitor, Selection, Side};
    let mut screens = two_screens();
    screens[1].mm_outside = Rect::new(700., 0., 340., 210.);
    screens[1].mm_content = Rect::new(710., 10., 320., 190.);
    let monitors: Vec<_> = screens
        .iter()
        .map(|s| {
            let mut resistance = BorderResistance::default();
            for side in Side::ALL {
                let length = side.length(s.mm_content);
                let (move_block, drag_block) = match side {
                    Side::Left => (true, true),
                    Side::Top => (true, false),
                    Side::Right => (false, true),
                    Side::Bottom => (false, false),
                };
                let make = |a, b| BorderSection::new(a, b, 6., move_block, 0., drag_block);
                *side.sections_mut(&mut resistance) = vec![
                    make(0., length * 0.4),
                    make(length * 0.5, length * 0.5 + 9.),
                    make(length * 0.6, length),
                ];
            }
            Monitor {
                id: s.id.into(),
                bounds: s.mm_content,
                excluded: false,
                resistance,
            }
        })
        .collect();
    let mut editor = Editor::default();
    editor.selected = Some(Selection {
        monitor: "left".into(),
        side: Side::Top,
        index: 0,
    });
    let mut h = harness((1050., 460.), |ui| {
        let fit = map::fit(map::extent(&screens), ui.max_rect());
        map::draw(ui, &screens, &fit, Some("left"));
        let placements = screens
            .iter()
            .map(|s| (s.id.into(), fit.place(s).content))
            .collect::<Vec<_>>();
        editor.bands(ui, &monitors, &placements);
    });
    h.run();
    h.snapshot("resistance-mitres");
}

#[test]
fn desktop_wallpapers_remain_below_the_glass_reflection_in_map_and_list() {
    let mut results = egui_kittest::SnapshotResults::new();
    for list_view in [false, true] {
        let mut textures = None;
        let mut h = harness((1050., 540.), |ui| {
            let textures = textures.get_or_insert_with(|| {
                [
                    egui::ColorImage::new(
                        [256, 144],
                        (0..144)
                            .flat_map(|y| {
                                (0..256).map(move |x| {
                                    if y > 95 {
                                        egui::Color32::from_rgb(18, 65, 59)
                                    } else {
                                        egui::Color32::from_rgb(
                                            40 + (x / 2) as u8,
                                            80 + y as u8,
                                            140 + (y / 2) as u8,
                                        )
                                    }
                                })
                            })
                            .collect(),
                    ),
                    egui::ColorImage::filled([256, 144], egui::Color32::from_rgb(32, 64, 96)),
                ]
                .map(|image| {
                    ui.ctx()
                        .load_texture("desktop-test", image, egui::TextureOptions::LINEAR)
                })
            });
            let mut screens = two_screens();
            for (screen, texture) in screens.iter_mut().zip(textures.iter()) {
                screen.wallpaper = Some(texture);
            }
            if list_view {
                lbm_ui::list::draw(ui, ui.max_rect(), &screens, Some("left"));
            } else {
                let fit = map::fit(map::extent(&screens), ui.max_rect());
                map::draw(ui, &screens, &fit, Some("left"));
            }
        });
        h.run();
        h.snapshot(if list_view {
            "desktop-wallpaper-list"
        } else {
            "desktop-wallpaper-map"
        });
        results.extend(h.take_snapshot_results());
    }
}
