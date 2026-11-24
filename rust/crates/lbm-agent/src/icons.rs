//! The tray icons: the C# tray's SVGs (`LittleBigMouse.Ui.Avalonia/Assets/Icon`), rendered
//! once at the sizes trays ask for (`icons/render.sh`), embedded, decoded on first use.
//!
//! The multi-PC mode marks them where the pointer is ([`Badge`]), drawn over the decoded
//! pixels rather than shipped as more PNGs: the same marks on every state and size, and
//! on both platforms — a StatusNotifierItem's `OverlayIconPixmap` is not drawn by every
//! host, and the Windows notification area has nothing of the kind.

use std::sync::OnceLock;

/// What the icon says about the engine (C#: `TrayIconController.IconFor`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIcon {
    On,
    Off,
    Dead,
    Paused,
}

impl TrayIcon {
    /// C# `IconFor`. A display that went to sleep shares the paused icon: from the tray's
    /// side it looks the same as an engine standing down — not running, not broken.
    pub fn for_state(engine: &str, suspended: bool) -> TrayIcon {
        match engine {
            _ if suspended => TrayIcon::Paused,
            "Running" => TrayIcon::On,
            "Stopped" => TrayIcon::Off,
            "Paused" => TrayIcon::Paused,
            _ => TrayIcon::Dead,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What the icon says of the multi-PC pointer, over the engine's state: something that
/// shows without hovering the icon, since its tooltip can only be read with the pointer
/// on this machine — exactly when it is not on the other one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Badge {
    /// The pointer is this machine's own, or the mode is off.
    #[default]
    None,
    /// This machine is a master and its mouse and keyboard are on another machine: the
    /// icon fades, its pointer gone.
    Away,
    /// A master's mouse and keyboard drive this machine: a dot in the corner, the
    /// master's pointer come in.
    Driven,
}

impl Badge {
    const ALL: [Badge; 3] = [Badge::None, Badge::Away, Badge::Driven];

    fn index(self) -> usize {
        self as usize
    }
}

/// The dot of [`Badge::Driven`]: a colour none of the states use, ringed in dark so that
/// it stands out on a light panel as on a dark one.
const DOT: [u8; 3] = [0xE0, 0x40, 0xFB];
const RING: [u8; 3] = [0x1A, 0x1A, 0x1A];
/// What is left of the icon's opacity under [`Badge::Away`].
const FADED: f32 = 0.35;

/// One size of an icon, ARGB32 in network byte order (what a StatusNotifierItem carries).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixmap {
    pub width: u32,
    pub height: u32,
    pub argb: Vec<u8>,
}

use TrayIcon::*;

const PNGS: [(TrayIcon, u32, &[u8]); 24] = [
    (On, 16, include_bytes!("../icons/lbm_on-16.png")),
    (On, 22, include_bytes!("../icons/lbm_on-22.png")),
    (On, 24, include_bytes!("../icons/lbm_on-24.png")),
    (On, 32, include_bytes!("../icons/lbm_on-32.png")),
    (On, 48, include_bytes!("../icons/lbm_on-48.png")),
    (On, 64, include_bytes!("../icons/lbm_on-64.png")),
    (Off, 16, include_bytes!("../icons/lbm_off-16.png")),
    (Off, 22, include_bytes!("../icons/lbm_off-22.png")),
    (Off, 24, include_bytes!("../icons/lbm_off-24.png")),
    (Off, 32, include_bytes!("../icons/lbm_off-32.png")),
    (Off, 48, include_bytes!("../icons/lbm_off-48.png")),
    (Off, 64, include_bytes!("../icons/lbm_off-64.png")),
    (Dead, 16, include_bytes!("../icons/lbm_dead-16.png")),
    (Dead, 22, include_bytes!("../icons/lbm_dead-22.png")),
    (Dead, 24, include_bytes!("../icons/lbm_dead-24.png")),
    (Dead, 32, include_bytes!("../icons/lbm_dead-32.png")),
    (Dead, 48, include_bytes!("../icons/lbm_dead-48.png")),
    (Dead, 64, include_bytes!("../icons/lbm_dead-64.png")),
    (Paused, 16, include_bytes!("../icons/lbm_paused-16.png")),
    (Paused, 22, include_bytes!("../icons/lbm_paused-22.png")),
    (Paused, 24, include_bytes!("../icons/lbm_paused-24.png")),
    (Paused, 32, include_bytes!("../icons/lbm_paused-32.png")),
    (Paused, 48, include_bytes!("../icons/lbm_paused-48.png")),
    (Paused, 64, include_bytes!("../icons/lbm_paused-64.png")),
];

/// Every size of `icon` marked with `badge`, smallest first.
pub fn badged(icon: TrayIcon, badge: Badge) -> &'static [Pixmap] {
    static DRAWN: OnceLock<Vec<Vec<Pixmap>>> = OnceLock::new();
    let drawn = DRAWN.get_or_init(|| {
        [On, Off, Dead, Paused]
            .iter()
            .flat_map(|&icon| {
                Badge::ALL.map(|badge| pixmaps(icon).iter().map(|p| mark(p, badge)).collect())
            })
            .collect()
    });
    &drawn[icon.index() * Badge::ALL.len() + badge.index()]
}

/// `pixmap` with `badge` drawn on it.
fn mark(pixmap: &Pixmap, badge: Badge) -> Pixmap {
    let mut marked = pixmap.clone();
    match badge {
        Badge::None => {}
        Badge::Away => {
            for pixel in marked.argb.as_chunks_mut::<4>().0 {
                pixel[0] = (f32::from(pixel[0]) * FADED).round() as u8;
            }
        }
        Badge::Driven => {
            // In the bottom right corner, a little under half the icon wide; the ring a
            // pixel thick at the small sizes, more at the large ones.
            let size = pixmap.width.min(pixmap.height) as f32;
            let radius = (size * 0.22).max(3.5);
            let ring = (size / 16.0).max(1.0);
            let centre = (pixmap.width as f32 - radius, pixmap.height as f32 - radius);
            for y in 0..pixmap.height {
                for x in 0..pixmap.width {
                    let distance = (x as f32 + 0.5 - centre.0).hypot(y as f32 + 0.5 - centre.1);
                    // Antialiased: how much of the pixel each disc covers.
                    let covered = |r: f32| (r + 0.5 - distance).clamp(0.0, 1.0);
                    let i = ((y * pixmap.width + x) * 4) as usize;
                    let pixel = &mut marked.argb[i..i + 4];
                    over(pixel, RING, covered(radius));
                    over(pixel, DOT, covered(radius - ring));
                }
            }
        }
    }
    marked
}

/// `colour` at opacity `alpha` over the ARGB `pixel` (straight alpha).
fn over(pixel: &mut [u8], colour: [u8; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let below = f32::from(pixel[0]) / 255.0;
    let out = alpha + below * (1.0 - alpha);
    for (channel, &c) in pixel[1..].iter_mut().zip(&colour) {
        let mixed = (f32::from(c) * alpha + f32::from(*channel) * below * (1.0 - alpha)) / out;
        *channel = mixed.round() as u8;
    }
    pixel[0] = (out * 255.0).round() as u8;
}

/// Every size of `icon`, smallest first.
pub fn pixmaps(icon: TrayIcon) -> &'static [Pixmap] {
    static DECODED: OnceLock<[Vec<Pixmap>; 4]> = OnceLock::new();
    let decoded = DECODED.get_or_init(|| {
        let mut all: [Vec<Pixmap>; 4] = Default::default();
        for (icon, size, png) in PNGS {
            let pixmap = decode(png);
            debug_assert_eq!((pixmap.width, pixmap.height), (size, size));
            all[icon.index()].push(pixmap);
        }
        all
    });
    &decoded[icon.index()]
}

/// An embedded PNG (8-bit RGBA, as `rsvg-convert` writes them) as ARGB32.
fn decode(png: &[u8]) -> Pixmap {
    let mut decoder = png::Decoder::new(png);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().expect("an embedded PNG");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut rgba).expect("an embedded PNG");
    let channels = frame.color_type.samples();
    let argb = rgba[..frame.buffer_size()]
        .chunks_exact(channels)
        .flat_map(|p| match *p {
            [r, g, b, a] => [a, r, g, b],
            [r, g, b] => [255, r, g, b],
            [l, a] => [a, l, l, l],
            [l] => [255, l, l, l],
            _ => unreachable!("1 to 4 samples"),
        })
        .collect();
    Pixmap {
        width: frame.width,
        height: frame.height,
        argb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_has_every_size_decoded() {
        for icon in [On, Off, Dead, Paused] {
            let sizes: Vec<u32> = pixmaps(icon).iter().map(|p| p.width).collect();
            assert_eq!(sizes, [16, 22, 24, 32, 48, 64]);
            for p in pixmaps(icon) {
                assert_eq!(p.argb.len(), (p.width * p.height * 4) as usize);
            }
        }
    }

    #[test]
    fn the_colors_are_the_states_ones() {
        // Inside the top-left square of the 64 px icon: green, blue, red, orange.
        let at = |icon: TrayIcon| {
            let p = &pixmaps(icon)[5];
            let i = ((16 * p.width + 6) * 4) as usize;
            [p.argb[i], p.argb[i + 1], p.argb[i + 2], p.argb[i + 3]]
        };
        let [a, r, g, b] = at(On);
        assert!(
            a == 255 && g > 150 && r < 100 && b < 100,
            "on is green: {r},{g},{b}"
        );
        let [_, r, g, b] = at(Off);
        assert!(b > 150 && r < 100, "off is blue: {r},{g},{b}");
        let [_, r, g, b] = at(Dead);
        assert!(r > 150 && g < 100 && b < 100, "dead is red: {r},{g},{b}");
        let [_, r, g, b] = at(Paused);
        assert!(
            r > 150 && g > 100 && b < 100,
            "paused is orange: {r},{g},{b}"
        );
    }

    /// The pixel at (`x`, `y`) of the 64 px `icon` under `badge`, as ARGB.
    fn pixel(icon: TrayIcon, badge: Badge, x: u32, y: u32) -> [u8; 4] {
        let p = &badged(icon, badge)[5];
        let i = ((y * p.width + x) * 4) as usize;
        p.argb[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn no_badge_is_the_plain_icon() {
        for icon in [On, Off, Dead, Paused] {
            assert_eq!(badged(icon, Badge::None), pixmaps(icon));
            let sizes: Vec<u32> = badged(icon, Badge::Driven)
                .iter()
                .map(|p| p.width)
                .collect();
            assert_eq!(sizes, [16, 22, 24, 32, 48, 64]);
        }
    }

    #[test]
    fn away_fades_the_icon_and_keeps_its_colour() {
        let plain = pixel(On, Badge::None, 6, 16);
        let away = pixel(On, Badge::Away, 6, 16);
        assert_eq!(plain[1..], away[1..], "still the state's colour");
        assert!(plain[0] == 255 && away[0] < 100, "but faded: {}", away[0]);
    }

    #[test]
    fn driven_puts_a_dot_in_the_corner_on_every_size_and_state() {
        for icon in [On, Off, Dead, Paused] {
            for p in badged(icon, Badge::Driven) {
                let at = |x: u32, y: u32| {
                    let i = ((y * p.width + x) * 4) as usize;
                    [p.argb[i], p.argb[i + 1], p.argb[i + 2], p.argb[i + 3]]
                };
                // The dot's centre is its colour, opaque, whatever was under it.
                let r = (p.width as f32 * 0.22).max(3.5) as u32;
                let [a, red, green, blue] = at(p.width - r, p.height - r);
                assert_eq!(
                    (a, [red, green, blue]),
                    (255, DOT),
                    "{icon:?} at {} px",
                    p.width
                );
                // The far corner is the ring's edge or the icon, never the dot itself.
                assert_ne!(at(0, 0)[1..], DOT);
            }
        }
        // The rest of the icon is untouched.
        assert_eq!(
            pixel(On, Badge::Driven, 6, 16),
            pixel(On, Badge::None, 6, 16)
        );
    }

    #[test]
    fn the_icon_follows_the_csharp_mapping() {
        assert_eq!(TrayIcon::for_state("Running", false), On);
        assert_eq!(TrayIcon::for_state("Stopped", false), Off);
        assert_eq!(TrayIcon::for_state("Dead", false), Dead);
        assert_eq!(TrayIcon::for_state("Paused", false), Paused);
        assert_eq!(
            TrayIcon::for_state("Stopped", true),
            Paused,
            "the display is off"
        );
    }
}
