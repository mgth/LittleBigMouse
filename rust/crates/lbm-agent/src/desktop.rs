//! The desktop background, one screen at a time.
//!
//! Port of C#'s `IWallpaperService` and its Plasma implementation. The agent says what each
//! screen should show — a file, or a colour — and the desktop environment is asked to show
//! it; nothing here decides what that should be, and nothing here ever reads the desktop
//! back (C#'s note: the wallpaper travels one way, out).
//!
//! Plasma has no per-screen wallpaper API: what it has is `evaluateScript`, which runs a
//! line of its own scripting language inside plasmashell. C# spawned `busctl` for that
//! call; the agent already speaks D-Bus (`sleep.rs`), so it makes the call itself.

use std::path::PathBuf;

use lbm_store::wallpaper_settings::WallpaperStyle;

/// What one screen should show. The position is how the screen is found: the desktop knows
/// its own screens by where they are, not by the names the layout uses.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenWallpaper {
    pub x: f64,
    pub y: f64,
    /// The image to show; `None` shows `color` instead.
    pub image: Option<PathBuf>,
    pub style: WallpaperStyle,
    /// `#RRGGBB`, as the settings hold it.
    pub color: String,
}

/// Whether this desktop is one we know how to set. Plasma answers its own script.
#[cfg(target_os = "linux")]
pub async fn is_supported() -> bool {
    plasma::evaluate(r#"print("ok")"#).await.as_deref() == Some("ok")
}

/// Shows `screens`. Answers whether the desktop took it.
///
/// A wallpaper is never a reason to fail anything: a desktop that refuses, or one that is
/// not Plasma, leaves the background as it was and says so once.
#[cfg(target_os = "linux")]
pub async fn apply(screens: &[ScreenWallpaper]) -> bool {
    if screens.is_empty() {
        return true;
    }
    plasma::evaluate(&plasma::script(screens)).await.is_some()
}

#[cfg(not(target_os = "linux"))]
pub async fn is_supported() -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
pub async fn apply(_screens: &[ScreenWallpaper]) -> bool {
    // Windows sets the background through IDesktopWallpaper; not ported yet.
    false
}

/// `org.kde.image`'s `FillMode` is a QML `Image.fillMode`, and the numbers are QML's.
/// Fill (`PreserveAspectCrop`) is the default; Span means nothing per screen, because a
/// span is already cut to the screen and arrives as Stretch.
pub fn fill_mode(style: WallpaperStyle) -> u8 {
    match style {
        WallpaperStyle::Stretch => 0,
        WallpaperStyle::Fit => 1,
        WallpaperStyle::Tile => 3,
        WallpaperStyle::Center => 6,
        WallpaperStyle::Fill | WallpaperStyle::Span => 2,
    }
}

/// `#RRGGBB` as Plasma writes colours: `R,G,B` in decimal. An unreadable colour is black,
/// which is what C#'s converter does with one.
pub fn plasma_color(color: &str) -> String {
    let hex = color.strip_prefix('#').unwrap_or(color);
    let channel = |at: usize| {
        hex.get(at..at + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .unwrap_or(0)
    };
    format!("{},{},{}", channel(0), channel(2), channel(4))
}

/// A local path as Plasma wants it: `org.kde.image` stores a URL.
pub fn file_url(path: &std::path::Path) -> String {
    let mut url = String::from("file://");
    // Percent-encoding, the set .NET's `Uri.AbsoluteUri` leaves alone for a path.
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'/'
            | b':'
            | b'@'
            | b'!'
            | b'$'
            | b'&'
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'=' => {
                url.push(byte as char);
            }
            other => url.push_str(&format!("%{other:02X}")),
        }
    }
    url
}

#[cfg(target_os = "linux")]
mod plasma {
    use serde_json::json;

    use super::{file_url, fill_mode, plasma_color, ScreenWallpaper};

    /// plasmashell's scripting entry point — the same one `plasma-apply-wallpaperimage`
    /// and C#'s `busctl` call go through.
    #[zbus::proxy(
        interface = "org.kde.PlasmaShell",
        default_service = "org.kde.plasmashell",
        default_path = "/PlasmaShell"
    )]
    trait PlasmaShell {
        /// Named, not derived: the method on the bus is `evaluateScript`, and zbus would
        /// otherwise ask for `EvaluateScript` — which plasma answers with "no such
        /// method", silently, as a wallpaper that never changes.
        #[zbus(name = "evaluateScript")]
        fn evaluate_script(&self, script: &str) -> zbus::Result<String>;
    }

    /// Runs `script` in plasmashell and answers what it printed, or nothing at all when
    /// there is no plasmashell to ask (a GNOME session, a bare one, a test runner).
    pub async fn evaluate(script: &str) -> Option<String> {
        let connection = zbus::Connection::session().await.ok()?;
        let shell = PlasmaShellProxy::new(&connection).await.ok()?;
        match shell.evaluate_script(script).await {
            Ok(printed) => Some(printed.trim().to_owned()),
            Err(error) => {
                eprintln!("[lbm-agent] wallpaper: plasma refused the script: {error}");
                None
            }
        }
    }

    /// The script C# builds, with the same shape: the screens as data, then one pass over
    /// plasma's own screens matching them by position.
    ///
    /// Matching by position, within two pixels, is what makes this work at all: the agent
    /// names screens by their layout id and plasma numbers them in its own order, so the
    /// only thing both sides agree on is where a screen is.
    pub fn script(screens: &[ScreenWallpaper]) -> String {
        let targets: Vec<serde_json::Value> = screens
            .iter()
            .map(|screen| match &screen.image {
                Some(path) => json!({
                    "x": screen.x,
                    "y": screen.y,
                    "image": file_url(path),
                    "fill": fill_mode(screen.style),
                }),
                None => json!({
                    "x": screen.x,
                    "y": screen.y,
                    "color": plasma_color(&screen.color),
                }),
            })
            .collect();

        format!(
            concat!(
                "var targets={};",
                "for(var i=0;i<screenCount;i++){{",
                "var g=screenGeometry(i);",
                "for(var j=0;j<targets.length;j++){{var t=targets[j];",
                "if(Math.abs(g.x-t.x)<2&&Math.abs(g.y-t.y)<2){{",
                "var d=desktopForScreen(i);",
                "if(t.image){{",
                "d.wallpaperPlugin=\"org.kde.image\";",
                "d.currentConfigGroup=[\"Wallpaper\",\"org.kde.image\",\"General\"];",
                "d.writeConfig(\"Image\",t.image);",
                "d.writeConfig(\"FillMode\",t.fill);",
                "}}else{{",
                "d.wallpaperPlugin=\"org.kde.color\";",
                "d.currentConfigGroup=[\"Wallpaper\",\"org.kde.color\",\"General\"];",
                "d.writeConfig(\"Color\",t.color);",
                "}}",
                "d.reloadConfig();",
                "}}}}}}",
                "print(\"ok\");"
            ),
            serde_json::Value::Array(targets)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_keeps_the_number_plasma_knows_it_by() {
        // These are QML Image.fillMode values, not ours: a wrong one silently shows the
        // wallpaper stretched, tiled or centred instead of what the user picked.
        assert_eq!(fill_mode(WallpaperStyle::Stretch), 0);
        assert_eq!(fill_mode(WallpaperStyle::Fit), 1);
        assert_eq!(fill_mode(WallpaperStyle::Tile), 3);
        assert_eq!(fill_mode(WallpaperStyle::Center), 6);
        assert_eq!(fill_mode(WallpaperStyle::Fill), 2);
        // A span arrives already cut to the screen, so it is shown as it is.
        assert_eq!(fill_mode(WallpaperStyle::Span), 2);
    }

    #[test]
    fn colours_travel_as_plasma_writes_them() {
        assert_eq!(plasma_color("#204060"), "32,64,96");
        assert_eq!(plasma_color("204060"), "32,64,96", "the hash is optional");
        assert_eq!(plasma_color("#FFFFFF"), "255,255,255");
        // Nothing readable: black rather than a script that fails to parse.
        assert_eq!(plasma_color("#nope"), "0,0,0");
        assert_eq!(plasma_color(""), "0,0,0");
    }

    #[test]
    fn a_path_becomes_a_url_plasma_can_open() {
        assert_eq!(
            file_url(std::path::Path::new("/home/someone/wall.png")),
            "file:///home/someone/wall.png"
        );
        // A space in a path is the ordinary case, and an unencoded one truncates the URL.
        assert_eq!(
            file_url(std::path::Path::new(
                "/home/someone/my pictures/wall (1).png"
            )),
            "file:///home/someone/my%20pictures/wall%20(1).png"
        );
        assert_eq!(
            file_url(std::path::Path::new("/home/someone/été.png")),
            "file:///home/someone/%C3%A9t%C3%A9.png",
            "not every home directory is ASCII"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_script_carries_the_screens_and_matches_them_by_position() {
        let script = super::plasma::script(&[
            ScreenWallpaper {
                x: 0.0,
                y: 0.0,
                image: Some(PathBuf::from("/data/span_1.png")),
                style: WallpaperStyle::Stretch,
                color: "#204060".to_owned(),
            },
            ScreenWallpaper {
                x: 1920.0,
                y: 0.0,
                image: None,
                style: WallpaperStyle::Fill,
                color: "#204060".to_owned(),
            },
        ]);

        assert!(script.starts_with("var targets=[{"));
        assert!(script.contains(r#""image":"file:///data/span_1.png""#));
        assert!(script.contains(r#""fill":0"#));
        assert!(script.contains(r#""color":"32,64,96""#));
        assert!(
            script.contains(r#""x":1920.0"#),
            "the second screen's position: {script}"
        );
        // The screen with no image goes to org.kde.color, the other to org.kde.image.
        assert!(script.contains("org.kde.color"));
        assert!(script.contains("org.kde.image"));
        // Positions are compared, never screen numbers: plasma orders its own.
        assert!(script.contains("Math.abs(g.x-t.x)<2"));
        assert!(script.ends_with(r#"print("ok");"#));
    }

    /// Not run by default: it needs a live Plasma session on the other end of the bus.
    /// `cargo test -p lbm-agent --lib -- --ignored plasma_answers` on a KDE desktop is
    /// how the D-Bus half of this module gets exercised at all — everything else here
    /// stops at the script it would have sent.
    #[cfg(target_os = "linux")]
    #[ignore = "needs a running plasmashell"]
    #[tokio::test]
    async fn plasma_answers_its_own_script() {
        assert!(
            is_supported().await,
            "no plasmashell answered evaluateScript"
        );
    }

    /// The other half of the same problem: a script that does not parse is refused the
    /// same silent way a misnamed method is. This one is sent for a screen at a position
    /// no desktop has, so plasma parses it, runs it, matches nothing and changes nothing
    /// — and answers, which is the whole assertion.
    #[cfg(target_os = "linux")]
    #[ignore = "needs a running plasmashell"]
    #[tokio::test]
    async fn plasma_runs_the_script_it_is_sent() {
        let nowhere = ScreenWallpaper {
            x: 500_000.0,
            y: 500_000.0,
            image: Some(PathBuf::from("/nowhere/none.png")),
            style: WallpaperStyle::Fill,
            color: "#204060".to_owned(),
        };

        assert!(
            super::plasma::evaluate(&super::plasma::script(&[nowhere]))
                .await
                .is_some(),
            "plasma did not run the script"
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn nothing_to_show_is_not_a_failure() {
        // Not a call to plasma: an empty list is what a layout with no wallpaper settings
        // produces, and it must not be reported as a desktop that refused.
        assert!(apply(&[]).await);
    }
}
