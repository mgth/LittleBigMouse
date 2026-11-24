//! Single-screen view. Selection belongs to the properties combo; the preview uses
//! the whole canvas, with the same outer margin as the desktop map.
use crate::frame::{self, Look};
use crate::map::{self, MapMonitor};
use lbm_layout::geo::Rect;

pub fn screen_rect(available: egui::Rect) -> egui::Rect {
    map::inner(available)
}

pub fn extent_of(chosen: Option<&MapMonitor>) -> Rect {
    chosen.map(|m| m.mm_outside).unwrap_or(Rect::EMPTY)
}

pub fn draw(
    ui: &mut egui::Ui,
    available: egui::Rect,
    monitors: &[MapMonitor<'_>],
    selected: Option<&str>,
) {
    draw_hosted(ui, available, monitors, &[], selected);
}

/// [`draw`], with the machine each monitor is showing (see [`map::draw_hosted`]).
pub fn draw_hosted(
    ui: &mut egui::Ui,
    available: egui::Rect,
    monitors: &[MapMonitor<'_>],
    hosts: &[Option<frame::Host<'_>>],
    selected: Option<&str>,
) {
    if let Some(i) = monitors.iter().position(|m| Some(m.id) == selected) {
        let m = &monitors[i];
        let fit = map::fit_in(screen_rect(available), extent_of(Some(m)));
        frame::monitor_on(
            ui,
            &fit.place(m),
            &Look {
                name: m.name,
                selected: false,
                logo: m.logo,
                wallpaper: m.wallpaper,
                details: m.details,
                probe: m.probe,
            },
            hosts.get(i).copied().flatten().as_ref(),
        );
    } else {
        ui.centered_and_justified(|ui| {
            ui.weak("Select a screen in the properties panel.");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_screen_uses_the_entire_canvas_with_a_single_margin() {
        let available = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900., 600.));
        let area = screen_rect(available);
        assert_eq!(area, available.shrink(map::MARGIN));
        let extent = Rect::new(0., 0., 640., 380.);
        assert_eq!(
            map::fit_in(area, extent).ratio,
            map::fit(extent, available).ratio
        );
    }
    #[test]
    fn tiny_canvases_and_empty_selections_are_valid() {
        let area = screen_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(40., 40.),
        ));
        assert!(area.width() >= 0. && area.height() >= 0.);
        assert!(extent_of(None).is_empty());
        assert_eq!(map::fit_in(area, extent_of(None)).ratio, 1.);
    }
}
