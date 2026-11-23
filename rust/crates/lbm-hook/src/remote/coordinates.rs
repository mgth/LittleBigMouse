//! Pixel positions as the absolute coordinates the two systems want.
//!
//! Pure arithmetic, compiled everywhere, so that the part of the Windows injector
//! which decides where the cursor lands is tested on every machine and not only on
//! the one that can run `SendInput`.

/// The `SendInput` coordinate, over one axis of the virtual screen, that puts the
/// cursor on pixel `position`.
///
/// `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK` takes 0..=65535 across the whole
/// virtual screen, which this process sees in physical pixels (it is per-monitor DPI
/// aware v2, `platform::windows::init`). The way back to a pixel is assumed to be
/// `origin + n × extent / 65536`, rounded down. Under that assumption the classic
/// `position × 65535 / (extent - 1)` lands one pixel short for many positions (pixel
/// 1 of a 1920-pixel screen gives 34, which comes back as pixel 0), whereas the smallest `n` that rounds down onto the pixel, `⌈p × 65536 / extent⌉`,
/// lands on it exactly for every extent below 65536 — what the test below checks.
///
/// A position off the virtual screen is held at its edge: the receiver's desktop is
/// the sender's idea of it, and the two may briefly disagree after a display change.
pub fn normalised_absolute(position: i32, origin: i32, extent: i32) -> i32 {
    if extent <= 0 {
        return 0;
    }
    let pixel = (i64::from(position) - i64::from(origin)).clamp(0, i64::from(extent) - 1);
    let extent = i64::from(extent);
    ((pixel * 65536 + extent - 1) / extent).min(65535) as i32
}

/// The uinput ABS value for pixel `position` of a desktop starting at `origin` and
/// `extent` pixels long: the device's range is `0..=extent - 1`, mapped 1:1 onto the
/// desktop by the compositor (see `hook::linux::evdev::uinput::build_pointer`).
pub fn desktop_relative(position: i32, origin: i32, extent: i32) -> i32 {
    position.saturating_sub(origin).clamp(0, extent.max(1) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The assumed way back from a `SendInput` coordinate to a pixel.
    fn landing(n: i32, origin: i32, extent: i32) -> i32 {
        origin + (i64::from(n) * i64::from(extent) / 65536) as i32
    }

    #[test]
    fn every_pixel_of_common_virtual_screens_is_reached_exactly() {
        for (origin, extent) in [
            (0, 1),
            (0, 1024),
            (0, 1920),
            (-1920, 5760),
            (0, 3840),
            (-2560, 7680),
            (0, 1366),
            (-1080, 4920),
            (0, 15360),
            (0, 65535),
        ] {
            for position in origin..origin + extent {
                let n = normalised_absolute(position, origin, extent);
                assert!((0..=65535).contains(&n));
                assert_eq!(
                    landing(n, origin, extent),
                    position,
                    "pixel {position} of a {extent}-pixel screen starting at {origin}"
                );
            }
        }
    }

    #[test]
    fn a_position_off_the_virtual_screen_is_held_at_its_edge() {
        assert_eq!(normalised_absolute(-5000, -1920, 5760), 0);
        assert_eq!(
            landing(normalised_absolute(99_999, -1920, 5760), -1920, 5760),
            3839
        );
        assert_eq!(normalised_absolute(10, 0, 0), 0);
    }

    #[test]
    fn a_desktop_relative_position_stays_inside_the_device_range() {
        assert_eq!(desktop_relative(-1920, -1920, 5760), 0);
        assert_eq!(desktop_relative(3839, -1920, 5760), 5759);
        assert_eq!(desktop_relative(4000, -1920, 5760), 5759);
        assert_eq!(desktop_relative(-3000, -1920, 5760), 0);
        assert_eq!(desktop_relative(i32::MIN, 0, 1920), 0);
        assert_eq!(desktop_relative(7, 0, 0), 0);
    }
}
