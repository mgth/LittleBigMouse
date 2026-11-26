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

/// The pictogram of an entry of the tray's menu.
///
/// Under Linux the menu host draws a themed icon by its name when the Icon Naming Spec
/// has one, so that it follows KDE's light or dark theme as the rest of the menu does;
/// the UI's pictograms are black where they mean "the foreground", which a dark menu
/// would swallow. Only Open, the logo, has no such name and goes as a PNG: its colours
/// read on either background. Windows has no theme to ask: every entry is a bitmap of
/// the UI's own SVGs (`icons/render.sh`), drawn on the light menu Win32 shows to an
/// application that did not opt into the dark one — the agent does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuIcon {
    /// LittleBigMouse's logo: Open shows the application.
    Logo,
    Start,
    Stop,
    Refresh,
    Exit,
}

impl MenuIcon {
    pub const ALL: [MenuIcon; 5] = [
        MenuIcon::Logo,
        MenuIcon::Start,
        MenuIcon::Stop,
        MenuIcon::Refresh,
        MenuIcon::Exit,
    ];

    /// Its name in the freedesktop Icon Naming Spec, which every conforming theme (Breeze,
    /// Adwaita, hicolor's fallbacks) draws; `None` for the logo, which is ours.
    pub fn theme_name(self) -> Option<&'static str> {
        match self {
            MenuIcon::Logo => None,
            MenuIcon::Start => Some("media-playback-start"),
            MenuIcon::Stop => Some("media-playback-stop"),
            MenuIcon::Refresh => Some("view-refresh"),
            MenuIcon::Exit => Some("application-exit"),
        }
    }

    /// The embedded PNGs, smallest first.
    pub fn pngs(self) -> &'static [(u32, &'static [u8])] {
        &MENU_PNGS[self as usize]
    }

    /// Every size, decoded, smallest first.
    pub fn pixmaps(self) -> &'static [Pixmap] {
        static DECODED: OnceLock<Vec<Vec<Pixmap>>> = OnceLock::new();
        let decoded = DECODED.get_or_init(|| {
            MENU_PNGS
                .iter()
                .map(|sizes| sizes.iter().map(|(_, png)| decode(png)).collect())
                .collect()
        });
        &decoded[self as usize]
    }
}

macro_rules! menu_pngs {
    ($name:literal) => {
        menu_pngs!(@ $name, 16, 20, 24, 32, 40, 48)
    };
    (@ $name:literal, $($size:literal),*) => {
        [$((
            $size,
            include_bytes!(concat!("../icons/menu/", $name, "-", $size, ".png")).as_slice(),
        )),*]
    };
}

/// In [`MenuIcon`] order.
const MENU_PNGS: [[(u32, &[u8]); 6]; 5] = [
    menu_pngs!("open"),
    menu_pngs!("start"),
    menu_pngs!("stop"),
    menu_pngs!("refresh"),
    menu_pngs!("exit"),
];

/// The first of `pixmaps` (smallest first) at least `wanted` wide, or the largest: a
/// smaller one would be stretched into a blur, a larger one is drawn shrunk.
pub fn at_least(pixmaps: &[Pixmap], wanted: u32) -> &Pixmap {
    pixmaps
        .iter()
        .find(|p| p.width >= wanted)
        .unwrap_or_else(|| pixmaps.last().expect("icons of every size"))
}

/// `pixmap` as the rows of a 32-bit DIB: BGRA, its colours premultiplied by their alpha,
/// as `AlphaBlend` — which draws a menu item's bitmap — reads them. Straight colours there
/// come out brighter than they are on every half-transparent edge.
pub fn premultiplied_bgra(pixmap: &Pixmap) -> Vec<u8> {
    let times = |c: u8, a: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
    pixmap
        .argb
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[a, r, g, b]| [times(b, a), times(g, a), times(r, a), a])
        .collect()
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
    fn every_menu_icon_has_every_size_decoded() {
        for icon in MenuIcon::ALL {
            let sizes: Vec<u32> = icon.pixmaps().iter().map(|p| p.width).collect();
            assert_eq!(sizes, [16, 20, 24, 32, 40, 48], "{icon:?}");
            let named: Vec<u32> = icon.pngs().iter().map(|&(size, _)| size).collect();
            assert_eq!(named, sizes, "{icon:?}: the table says the sizes it embeds");
            for p in icon.pixmaps() {
                assert_eq!(
                    (p.height as usize, p.argb.len()),
                    (p.width as usize, (p.width * p.width * 4) as usize)
                );
                assert!(
                    p.argb.chunks(4).any(|px| px[0] == 255),
                    "{icon:?} draws something"
                );
                assert!(
                    p.argb.chunks(4).any(|px| px[0] == 0),
                    "{icon:?} is transparent around"
                );
            }
        }
    }

    #[test]
    fn the_menu_icons_are_the_uis_pictograms() {
        // The 48 px renders of the UI's SVGs: Start's green outline, Stop's red one, the
        // logo's blue screens; Refresh and Exit in the foreground's black.
        let colours = |icon: MenuIcon| -> Vec<[u8; 3]> {
            let p = &icon.pixmaps()[5];
            p.argb
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|px| px[0] == 255)
                .map(|px| [px[1], px[2], px[3]])
                .collect()
        };
        let has = |icon, test: fn(&[u8; 3]) -> bool| colours(icon).iter().any(test);
        assert!(has(MenuIcon::Start, |&[r, g, b]| g > 200
            && r < 100
            && b < 100));
        assert!(has(MenuIcon::Stop, |&[r, g, b]| r > 150
            && g < 60
            && b < 60));
        assert!(has(MenuIcon::Logo, |&[r, g, b]| b > 150
            && r < 60
            && g < 160));
        for icon in [MenuIcon::Refresh, MenuIcon::Exit] {
            assert!(
                colours(icon)
                    .iter()
                    .all(|&[r, g, b]| r < 40 && g < 40 && b < 40),
                "{icon:?}"
            );
        }
    }

    #[test]
    fn the_menu_names_are_the_icon_naming_specs() {
        let names: Vec<Option<&str>> = MenuIcon::ALL.iter().map(|i| i.theme_name()).collect();
        assert_eq!(
            names,
            [
                None,
                Some("media-playback-start"),
                Some("media-playback-stop"),
                Some("view-refresh"),
                Some("application-exit"),
            ]
        );
    }

    #[test]
    fn at_least_takes_the_next_size_up() {
        let sizes = MenuIcon::Stop.pixmaps();
        assert_eq!(at_least(sizes, 16).width, 16);
        assert_eq!(
            at_least(sizes, 18).width,
            20,
            "125 % of 15 rounds to 18 on some setups"
        );
        assert_eq!(at_least(sizes, 32).width, 32);
        assert_eq!(
            at_least(sizes, 64).width,
            48,
            "past the largest: the largest"
        );
        assert_eq!(at_least(sizes, 0).width, 16);
    }

    #[test]
    fn premultiplied_bgra_swaps_and_scales_the_colours() {
        let pixmap = Pixmap {
            width: 4,
            height: 1,
            argb: vec![
                255, 10, 20, 30, // opaque: unchanged, reordered
                0, 200, 100, 50, // transparent: black
                128, 255, 0, 64, // half: halved, rounded
                51, 255, 255, 255, // a fifth of white
            ],
        };
        assert_eq!(
            premultiplied_bgra(&pixmap),
            [30, 20, 10, 255, 0, 0, 0, 0, 32, 0, 128, 128, 51, 51, 51, 51]
        );
        // Never a colour over its alpha, which AlphaBlend would overflow.
        for icon in MenuIcon::ALL {
            for p in icon.pixmaps() {
                for px in premultiplied_bgra(p).as_chunks::<4>().0 {
                    assert!(px[..3].iter().all(|&c| c <= px[3]), "{icon:?}");
                }
            }
        }
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
