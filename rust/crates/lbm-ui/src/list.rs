//! The list mode: the screens down one side, the chosen one drawn large.
//!
//! The other half of the map. Where the map shows the desktop's shape and hides the
//! detail, the list shows one screen at a size worth reading and the rest as names. The
//! two share a selection, which is the whole reason they are two views of one state and
//! not two screens.
//!
//! Ported from `MonitorsListPresenterView.axaml:30-53`: one margin around the pair, then
//! `ColumnDefinitions="*,2*"` — the list takes a third, the screen two. Its `GetRatio`
//! (`:61-75`) is [`crate::map::fit_in`] on the selected screen's outside bounds, and its
//! "nothing selected" case is a ratio of 1, which falls out of an empty extent.

use crate::frame::{self, Look};
use crate::map::{self, MapMonitor};
use lbm_layout::geo::Rect;

/// A row is this tall, whatever is in it — `Height="100"` on the row template
/// (`MonitorsListPresenterView.axaml:38`).
pub const ROW_HEIGHT: f32 = 100.0;

/// The list takes one share of the width and the drawn screen two.
const LIST_SHARE: f32 = 1.0 / 3.0;

/// Where the two halves of the list mode go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panes {
    /// The names, down the left.
    pub list: egui::Rect,
    /// The chosen screen, drawn large. Already inside the margin, so it is fitted with
    /// [`crate::map::fit_in`] and not [`crate::map::fit`].
    pub screen: egui::Rect,
}

/// Splits the window into the two panes.
pub fn panes(available: egui::Rect) -> Panes {
    // The same box the map is fitted into, and for the same reason: a window narrower
    // than its own two margins would otherwise give panes of negative width — which my
    // own test caught here after I had already guarded it in `map`, one rule written
    // twice being one rule too many.
    let inner = map::inner(available);
    let split = inner.left() + inner.width() * LIST_SHARE;
    Panes {
        list: egui::Rect::from_min_max(inner.min, egui::pos2(split, inner.max.y)),
        screen: egui::Rect::from_min_max(egui::pos2(split, inner.min.y), inner.max),
    }
}

/// The extent the chosen screen is fitted to: its own outside bounds, or nothing at all.
///
/// `Rect::EMPTY` when nothing is chosen, which is how the C# reaches its ratio of 1
/// without a second branch — and why [`crate::map::fit_in`] has to treat an empty extent
/// as "nothing to fit" rather than dividing by its negative infinities.
pub fn extent_of(chosen: Option<&MapMonitor>) -> Rect {
    chosen.map(|m| m.mm_outside).unwrap_or(Rect::EMPTY)
}

/// Draws the list of names and gives back the one clicked.
pub fn rows<'a>(
    ui: &mut egui::Ui,
    at: egui::Rect,
    monitors: &[MapMonitor<'a>],
    selected: Option<&str>,
) -> Option<&'a str> {
    let mut clicked = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| {
        for (row, m) in monitors.iter().enumerate() {
            let rect = egui::Rect::from_min_size(
                egui::pos2(at.left(), at.top() + row as f32 * ROW_HEIGHT),
                egui::vec2(at.width(), ROW_HEIGHT),
            );
            // Off the bottom of the pane: the C# `ListBox` scrolls, and scrolling is a
            // thing this view will need. Until it has one, a row that would be drawn
            // outside is not drawn at all rather than painted over the screen beside it.
            if rect.top() >= at.bottom() {
                break;
            }
            if ui
                .put(
                    rect,
                    egui::Button::selectable(selected == Some(m.id), m.name),
                )
                .clicked()
            {
                clicked = Some(m.id);
            }
        }
    });
    clicked
}

/// Draws the whole list mode. Returns the monitor the user picked, if any.
pub fn draw<'a>(
    ui: &mut egui::Ui,
    available: egui::Rect,
    monitors: &[MapMonitor<'a>],
    selected: Option<&str>,
) -> Option<&'a str> {
    let panes = panes(available);
    let clicked = rows(ui, panes.list, monitors, selected);

    let chosen = monitors.iter().find(|m| Some(m.id) == selected);
    if let Some(m) = chosen {
        let fit = map::fit_in(panes.screen, extent_of(Some(m)));
        let drawn = fit.place(m);
        frame::monitor(
            ui,
            &drawn,
            &Look {
                name: m.name,
                // Nothing to tell apart: it is the only screen drawn. Highlighting it
                // would be shouting the obvious.
                selected: false,
                logo: m.logo,
            },
        );
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))
    }

    fn screens() -> Vec<MapMonitor<'static>> {
        vec![
            MapMonitor {
                id: "left",
                name: "Left screen",
                mm_outside: Rect::new(0.0, 0.0, 640.0, 380.0),
                mm_content: Rect::new(20.0, 20.0, 600.0, 340.0),
                logo: None,
            },
            MapMonitor {
                id: "right",
                name: "Right screen",
                mm_outside: Rect::new(700.0, 0.0, 640.0, 380.0),
                mm_content: Rect::new(720.0, 20.0, 600.0, 340.0),
                logo: None,
            },
        ]
    }

    #[test]
    fn the_screen_gets_two_thirds_of_what_is_left_after_the_margin() {
        let win = window(900.0, 600.0);
        let p = panes(win);

        let inner = win.shrink(crate::map::MARGIN);
        assert!((p.list.width() - inner.width() / 3.0).abs() < 1e-3);
        assert!((p.screen.width() - inner.width() * 2.0 / 3.0).abs() < 1e-3);
        // Side by side, no gap and no overlap, and both inside the margin.
        assert_eq!(p.list.right(), p.screen.left());
        assert_eq!(p.list.left(), inner.left());
        assert_eq!(p.screen.right(), inner.right());
        assert_eq!(p.list.top(), inner.top());
        assert_eq!(p.screen.bottom(), inner.bottom());
    }

    /// The margin belongs to the pair, not to each pane. Fitting the screen with the
    /// window-taking `fit` would take it a second time and draw the screen smaller than
    /// its column.
    #[test]
    fn the_margin_is_taken_once_and_not_twice() {
        let win = window(900.0, 600.0);
        let p = panes(win);
        let one = screens()[0];

        let right = map::fit_in(p.screen, extent_of(Some(&one)));
        let wrong = map::fit(extent_of(Some(&one)), p.screen);

        assert!(
            right.ratio > wrong.ratio,
            "the margin was not taken twice: {} against {}",
            right.ratio,
            wrong.ratio
        );
        // And the screen really does fill its column on the binding axis.
        let drawn = right.place(&one).outside;
        let fills = (drawn.width() - p.screen.width()).abs() < 0.1
            || (drawn.height() - p.screen.height()).abs() < 0.1;
        assert!(fills, "{drawn:?} in {:?}", p.screen);
    }

    /// The point of the list mode is size — but only once there is more than one screen
    /// to share the map with. Measured rather than assumed: two screens side by side
    /// give 0.875 against 0.627, and my first go at this test asserted a factor of 1.5
    /// that the arithmetic does not produce.
    #[test]
    fn the_more_screens_there_are_the_more_the_list_mode_is_worth() {
        let win = window(900.0, 600.0);
        let two = screens();
        let alone = map::fit_in(panes(win).screen, extent_of(Some(&two[1]))).ratio;
        assert!(
            alone > map::fit(map::extent(&two), win).ratio,
            "two screens: {alone}"
        );

        // Four in a row, where the map has to shrink much harder.
        let four: Vec<MapMonitor> = (0..4)
            .map(|i| MapMonitor {
                id: "id",
                name: "name",
                mm_outside: Rect::new(i as f64 * 700.0, 0.0, 640.0, 380.0),
                mm_content: Rect::new(i as f64 * 700.0 + 20.0, 20.0, 600.0, 340.0),
                logo: None,
            })
            .collect();
        let on_the_map = map::fit(map::extent(&four), win).ratio;
        let one_of_four = map::fit_in(panes(win).screen, extent_of(Some(&four[0]))).ratio;
        assert!(
            one_of_four > on_the_map * 2.5,
            "four screens: {one_of_four} against {on_the_map}"
        );
    }

    /// And the boundary, which is not a defect but is worth knowing: on a **single**
    /// screen the list mode draws it *smaller* than the map does, because its column is
    /// two thirds of the width while the map has all of it. The C# does the same.
    #[test]
    fn a_lone_screen_is_smaller_in_the_list_than_on_the_map() {
        let win = window(900.0, 600.0);
        let one = vec![screens()[0]];

        let on_the_map = map::fit(map::extent(&one), win).ratio;
        let in_the_list = map::fit_in(panes(win).screen, extent_of(Some(&one[0]))).ratio;

        assert!(
            in_the_list < on_the_map,
            "a lone screen came out larger in the list: {in_the_list} against {on_the_map}"
        );
    }

    /// Nothing chosen is not an error and not a guess: no extent, and the scale is left
    /// alone. Choosing the first screen for the user would be choosing for them.
    #[test]
    fn nothing_chosen_fits_nothing() {
        assert!(extent_of(None).is_empty());
        assert_eq!(
            map::fit_in(panes(window(900.0, 600.0)).screen, extent_of(None)).ratio,
            1.0
        );
    }

    #[test]
    fn a_window_smaller_than_its_margins_still_gives_two_panes() {
        let p = panes(window(40.0, 40.0));
        assert!(p.list.width() >= 0.0 && p.screen.width() >= 0.0);
        assert_eq!(p.list.right(), p.screen.left());
    }
}
