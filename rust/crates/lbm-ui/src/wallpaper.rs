//! What the desktop picture will look like on one screen: the geometry, not the pixels.
//!
//! The frame draws a thumbnail of the wallpaper so that the map shows the desktop rather
//! than a set of grey rectangles. For five of the six styles the *desktop environment* is
//! what actually paints the picture — the agent hands it the file and the style name and
//! Plasma or Windows does the rest — so the thumbnail is LittleBigMouse's own rendering
//! of what the desktop is about to do, and it has to agree with it.
//!
//! Only the arithmetic is here. Deciding a crop is where the rules and the surprises
//! live; pushing the pixels afterwards is a call to an image library. Keeping them apart
//! means the rules can be tested without decoding anything.
//!
//! Ported from `WallpaperRendererHelper.cs:186-284`. Everything is integer pixels and
//! **truncating** division, as there: the rounding is part of the result, and #492 is
//! what happens when it is not respected.

/// A size in pixels. Integers, and the division that makes them truncates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

/// A rectangle in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn size(self) -> Size {
        Size {
            w: self.w,
            h: self.h,
        }
    }

    /// The part of `self` that is inside a `bounds`-sized image at the origin.
    ///
    /// `CropClamped`, and the reason it exists is worth keeping: the truncating division
    /// above can leave the crop up to a pixel past the resized image, and in the C# that
    /// threw from a background thread and **took the whole UI down on startup** — issue
    /// #492. The clamp is not tidiness.
    fn clamped(self, bounds: Size) -> Rect {
        let left = self.x.max(0);
        let top = self.y.max(0);
        let right = (self.x + self.w).min(bounds.w);
        let bottom = (self.y + self.h).min(bounds.h);
        Rect {
            x: left,
            y: top,
            w: (right - left).max(0),
            h: (bottom - top).max(0),
        }
    }
}

/// How the picture is laid on a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Cover the screen, cropping what does not fit.
    Fill,
    /// Show all of it, filling the rest with the colour.
    Fit,
    /// Distort it to the screen's shape.
    Stretch,
    /// Repeat it from the desktop's corner.
    Tile,
    /// One copy, its top-left corner, padded with the colour.
    Center,
    /// One picture across the whole desktop, this screen's share of it.
    Span,
}

/// One step of the recipe, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Take this rectangle out of what we have.
    Crop(Rect),
    /// Scale what we have to this.
    Resize(Size),
    /// Lay what we have on a canvas of `to` at `at`, the rest in the fill colour.
    Pad { to: Size, at: (i32, i32) },
    /// Repeat what we have from the corner of a canvas of `to`.
    Tile { to: Size },
}

/// The thumbnail is rendered at a quarter of the size, per axis
/// (`MonitorFrameViewModel.cs:177`, `const int shrink = 4`).
pub const SHRINK: i32 = 4;

/// What the source is reduced to first, when it is reduced at all.
///
/// Truncating, and `shrink == 1` is not a resize at all — the C# returns the context
/// untouched, which matters because resizing to the same size is not free.
pub fn shrunk(source: Size, by: i32) -> Option<Size> {
    if by <= 1 {
        return None;
    }
    Some(Size {
        w: source.w / by,
        h: source.h / by,
    })
}

/// The recipe for the styles that only look at one screen.
///
/// `source` is the picture's size (already shrunk, if it is being shrunk) and `target`
/// the screen's, in the same units.
pub fn on_one_screen(style: Style, source: Size, target: Size) -> Vec<Step> {
    match style {
        Style::Stretch => vec![Step::Resize(target)],

        // Cover: scale so the smaller overflow wins, then keep the middle.
        Style::Fill => {
            let ratio = f64::max(
                target.w as f64 / source.w as f64,
                target.h as f64 / source.h as f64,
            );
            let w = (target.w as f64 / ratio) as i32;
            let h = (target.h as f64 / ratio) as i32;
            vec![
                Step::Crop(Rect {
                    x: (source.w - w) / 2,
                    y: (source.h - h) / 2,
                    w,
                    h,
                }),
                Step::Resize(target),
            ]
        }

        // Contain: the whole picture, centred, the rest in the colour.
        Style::Fit => {
            let ratio = f64::min(
                target.w as f64 / source.w as f64,
                target.h as f64 / source.h as f64,
            );
            let resize = Size {
                w: (ratio * source.w as f64) as i32,
                h: (ratio * source.h as f64) as i32,
            };
            vec![
                Step::Resize(resize),
                Step::Pad {
                    to: target,
                    at: ((target.w - resize.w) / 2, (target.h - resize.h) / 2),
                },
            ]
        }

        // **Not centred, and this is the C#'s own behaviour rather than a slip of the
        // port**: `Center` crops from `(0, 0)`, so a picture larger than the screen
        // shows its *top-left corner* and not its middle. Only the padding is centred,
        // which is what makes the name fit the smaller-picture case and not the larger
        // one. Kept as it is — a thumbnail that disagreed with the desktop would be
        // worse than one that agrees with something odd — but it is worth a decision.
        Style::Center => {
            let w = source.w.min(target.w);
            let h = source.h.min(target.h);
            vec![
                Step::Crop(Rect { x: 0, y: 0, w, h }),
                Step::Pad {
                    to: target,
                    at: ((target.w - w) / 2, (target.h - h) / 2),
                },
            ]
        }

        // These two need the whole desktop; see `across_the_desktop`.
        Style::Tile | Style::Span => Vec::new(),
    }
}

/// The recipe for the two styles that are cut out of a desktop-wide picture.
///
/// `screen` is this screen's rectangle and `desktop` the whole span, both in desktop
/// pixels; the result is this screen's share.
pub fn across_the_desktop(style: Style, source: Size, screen: Rect, desktop: Rect) -> Vec<Step> {
    match style {
        // The picture is repeated across the desktop first, so what is cut out of it is
        // already the right size — the cover-scale below is a no-op on it, which is why
        // the C# reaches this through the same `SpanOrigin` with no special case.
        Style::Tile => vec![
            Step::Tile { to: desktop.size() },
            Step::Crop(
                Rect {
                    x: screen.x - desktop.x,
                    y: screen.y - desktop.y,
                    w: screen.w,
                    h: screen.h,
                }
                .clamped(desktop.size()),
            ),
        ],

        Style::Span => {
            let ratio = f64::max(
                desktop.w as f64 / source.w as f64,
                desktop.h as f64 / source.h as f64,
            );
            let resize = Size {
                w: (ratio * source.w as f64) as i32,
                h: (ratio * source.h as f64) as i32,
            };
            // The horizontal overflow is halved and **the vertical overflow is divided
            // by three**. Not a typo and not a guess: the comment in the C# records it
            // as measured against real Windows Span output across image heights 1000 to
            // 2400 — the visible band starts at `(H - desktop.h) / 3`. Centring it
            // instead would put every spanned wallpaper a few percent out.
            let offset_x = (resize.w - desktop.w) / 2;
            let offset_y = (resize.h - desktop.h) / 3;
            vec![
                Step::Resize(resize),
                Step::Crop(
                    Rect {
                        x: screen.x - desktop.x + offset_x,
                        y: screen.y - desktop.y + offset_y,
                        w: screen.w,
                        h: screen.h,
                    }
                    .clamped(resize),
                ),
            ]
        }

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: i32, h: i32) -> Size {
        Size { w, h }
    }

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn shrinking_is_a_quarter_each_way_and_nothing_at_all_at_one() {
        assert_eq!(shrunk(size(1920, 1080), SHRINK), Some(size(480, 270)));
        assert_eq!(shrunk(size(1920, 1080), 1), None, "1 is not a resize");
        assert_eq!(shrunk(size(1920, 1080), 0), None);
        // Truncating, as the C# `(int)` cast is.
        assert_eq!(shrunk(size(1921, 1081), 4), Some(size(480, 270)));
    }

    /// Cover: nothing of the screen is left unpainted, and the picture keeps its shape.
    #[test]
    fn fill_crops_the_middle_and_covers_the_screen() {
        // A 4:3 picture on a 16:9 screen: the top and bottom go.
        let steps = on_one_screen(Style::Fill, size(1600, 1200), size(1920, 1080));

        let Step::Crop(crop) = steps[0] else {
            panic!("{steps:?}")
        };
        assert_eq!(crop.w, 1600, "the full width is kept");
        assert_eq!(crop.h, 900, "and 4:3 becomes 16:9 by losing height");
        assert_eq!(crop.x, 0);
        assert_eq!(crop.y, (1200 - 900) / 2, "what is kept is the middle");
        assert_eq!(steps[1], Step::Resize(size(1920, 1080)));
    }

    /// Contain: all of the picture, centred, the rest painted.
    #[test]
    fn fit_shows_all_of_it_and_centres_what_is_left() {
        let steps = on_one_screen(Style::Fit, size(1600, 1200), size(1920, 1080));

        let Step::Resize(resize) = steps[0] else {
            panic!("{steps:?}")
        };
        assert_eq!(resize, size(1440, 1080), "bound by the height");
        assert_eq!(
            steps[1],
            Step::Pad {
                to: size(1920, 1080),
                at: ((1920 - 1440) / 2, 0),
            }
        );
    }

    #[test]
    fn stretch_is_a_resize_and_nothing_else() {
        assert_eq!(
            on_one_screen(Style::Stretch, size(800, 600), size(1920, 1080)),
            vec![Step::Resize(size(1920, 1080))]
        );
    }

    /// The C#'s own oddity, kept deliberately: a picture bigger than the screen shows
    /// its **top-left corner**, not its middle, however much the style is called Center.
    #[test]
    fn center_crops_from_the_corner_and_not_from_the_centre() {
        let steps = on_one_screen(Style::Center, size(3840, 2160), size(1920, 1080));

        assert_eq!(
            steps[0],
            Step::Crop(rect(0, 0, 1920, 1080)),
            "if this ever becomes a centred crop it is a change of behaviour, not a fix"
        );

        // A picture smaller than the screen is what the name is really about, and that
        // case is centred by the padding.
        let small = on_one_screen(Style::Center, size(800, 600), size(1920, 1080));
        assert_eq!(small[0], Step::Crop(rect(0, 0, 800, 600)));
        assert_eq!(
            small[1],
            Step::Pad {
                to: size(1920, 1080),
                at: ((1920 - 800) / 2, (1080 - 600) / 2),
            }
        );
    }

    /// The measured Windows quirk. Centring the vertical overflow instead of thirding it
    /// is the kind of change that looks like a cleanup and moves every spanned wallpaper.
    #[test]
    fn span_halves_the_overflow_across_and_thirds_it_down() {
        // A 4000x3000 picture over a 3840x1080 desktop: cover-scaling makes it very tall.
        let desktop = rect(0, 0, 3840, 1080);
        let steps = across_the_desktop(Style::Span, size(4000, 3000), desktop, desktop);

        let Step::Resize(resize) = steps[0] else {
            panic!("{steps:?}")
        };
        assert_eq!(resize, size(3840, 2880));

        let Step::Crop(crop) = steps[1] else {
            panic!("{steps:?}")
        };
        assert_eq!(crop.x, 0, "no overflow across, so no offset");
        assert_eq!(
            crop.y,
            (2880 - 1080) / 3,
            "the vertical overflow is divided by three, not two"
        );
        assert_ne!(crop.y, (2880 - 1080) / 2, "and it is not centred");
    }

    /// Each screen gets its own slice, offset by where it sits on the desktop.
    #[test]
    fn span_gives_each_screen_its_own_part() {
        let desktop = rect(0, 0, 3840, 1080);
        let left = across_the_desktop(
            Style::Span,
            size(3840, 1080),
            rect(0, 0, 1920, 1080),
            desktop,
        );
        let right = across_the_desktop(
            Style::Span,
            size(3840, 1080),
            rect(1920, 0, 1920, 1080),
            desktop,
        );

        let (Step::Crop(l), Step::Crop(r)) = (left[1], right[1]) else {
            panic!()
        };
        assert_eq!(l.x, 0);
        assert_eq!(r.x, 1920, "the second screen takes the second half");
        assert_eq!(l.w, r.w);
    }

    /// A desktop whose corner is negative — a screen above or left of the primary — is
    /// counted from its own corner, or every slice is off by the whole offset.
    #[test]
    fn a_desktop_that_starts_at_a_negative_corner_still_slices_from_zero() {
        let desktop = rect(-1920, -100, 3840, 1080);
        let steps = across_the_desktop(
            Style::Span,
            size(3840, 1080),
            rect(-1920, -100, 1920, 1080),
            desktop,
        );

        let Step::Crop(crop) = steps[1] else { panic!() };
        assert_eq!((crop.x, crop.y), (0, 0));
    }

    #[test]
    fn tile_repeats_across_the_desktop_then_takes_this_screens_part() {
        let desktop = rect(0, 0, 3840, 1080);
        let steps = across_the_desktop(
            Style::Tile,
            size(256, 256),
            rect(1920, 0, 1920, 1080),
            desktop,
        );

        assert_eq!(
            steps[0],
            Step::Tile {
                to: size(3840, 1080)
            }
        );
        assert_eq!(steps[1], Step::Crop(rect(1920, 0, 1920, 1080)));
    }

    /// Issue #492: truncating division can put the crop a pixel past the image, and in
    /// the C# that threw from a background thread and took the UI down on startup. The
    /// clamp is the fix, and it has to survive the port.
    #[test]
    fn a_crop_that_falls_past_the_image_is_cut_back_rather_than_thrown() {
        // Sizes chosen so the cover-scale truncates: 1000/333 is not exact.
        let desktop = rect(0, 0, 1000, 1000);
        let steps = across_the_desktop(Style::Span, size(333, 333), desktop, desktop);

        let Step::Resize(resize) = steps[0] else {
            panic!()
        };
        let Step::Crop(crop) = steps[1] else { panic!() };
        assert!(
            crop.x >= 0 && crop.y >= 0,
            "the crop starts outside the image: {crop:?}"
        );
        assert!(
            crop.x + crop.w <= resize.w && crop.y + crop.h <= resize.h,
            "the crop runs past a {resize:?} image: {crop:?}"
        );

        // And the clamp itself, on a rectangle that plainly overhangs.
        assert_eq!(
            rect(-5, -5, 100, 100).clamped(size(50, 50)),
            rect(0, 0, 50, 50)
        );
        assert_eq!(rect(60, 60, 10, 10).clamped(size(50, 50)).w, 0);
    }
}
