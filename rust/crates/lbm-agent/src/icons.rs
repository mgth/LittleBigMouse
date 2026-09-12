//! The tray icons: the C# tray's SVGs (`LittleBigMouse.Ui.Avalonia/Assets/Icon`), rendered
//! once at the sizes trays ask for (`icons/render.sh`), embedded, decoded on first use.

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
