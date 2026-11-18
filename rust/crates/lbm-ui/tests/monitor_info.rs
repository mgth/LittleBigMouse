#![cfg(not(windows))]

use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::{
    map::{self, MapMonitor},
    monitor_info::{self, Info, Presentation},
};

#[test]
fn original_monitor_presentations() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (width, height, scale) in [
        (760., 500., "wide"),
        (330., 290., "compact"),
        (240., 190., "tiny"),
        (320., 510., "portrait"),
    ] {
        for location in [false, true] {
            let mut picture = None;
            let mut harness = Harness::builder()
                .with_size(egui::vec2(width, height))
                .build_ui(|ui| {
                    lbm_ui::theme::install(ui.ctx());
                    let texture = picture.get_or_insert_with(|| {
                        let mut pixels = egui::ColorImage::filled([240, 120], egui::Color32::BLACK);
                        for y in 0..120 {
                            for x in 0..240 {
                                pixels[(x, y)] = egui::Color32::from_rgb(
                                    (30 + x / 2) as u8,
                                    (60 + y) as u8,
                                    (160 - x / 3) as u8,
                                );
                            }
                        }
                        ui.ctx().load_texture("picture", pixels, Default::default())
                    });
                    egui::CentralPanel::default().show(ui, |ui| {
                        let portrait = scale == "portrait";
                        let (w, h) = if portrait { (360., 620.) } else { (620., 360.) };
                        let monitor = MapMonitor {
                            id: "a",
                            name: "Studio Monitor",
                            mm_outside: Rect::new(0., 0., w, h),
                            mm_content: Rect::new(10., 10., w - 20., h - 25.),
                            logo: None,
                            wallpaper: Some(texture),
                            details: &[],
                            probe: None,
                        };
                        let fit = map::fit(monitor.mm_outside, ui.max_rect());
                        map::draw(ui, &[monitor], &fit, Some("a"));
                        let content = fit.place(&monitor).content;
                        monitor_info::draw(
                            ui,
                            content,
                            &Info {
                                number: "1".into(),
                                pnp: "DEL42A1".into(),
                                name: "Dell UltraSharp U2723QE".into(),
                                connector: "DisplayPort · DP-1".into(),
                                serial: "CN0R9K73 · 8J2L41".into(),
                                resolution: "3840 × 2160 px".into(),
                                density: "163 dpi".into(),
                                diagonal: "27.0 in".into(),
                                refresh: "60 Hz".into(),
                                primary: true,
                                excluded: false,
                            },
                            if location {
                                Presentation::Location
                            } else {
                                Presentation::Info
                            },
                        );
                        if location {
                            lbm_ui::inline::location(
                                ui,
                                "a",
                                content,
                                &lbm_ui::properties::Location {
                                    x: -25.,
                                    y: 10.,
                                    scale_x: 100.,
                                    scale_y: 100.,
                                },
                            );
                        }
                    });
                });
            harness.run();
            harness.snapshot(format!(
                "monitor-{}-{scale}",
                if location {
                    "location-overlay"
                } else {
                    "info-overlay"
                }
            ));
            results.extend(harness.take_snapshot_results());
        }
    }
}
