//! The desktop background, one screen at a time.
//!
//! The agent supplies an image or colour per screen. Plasma receives a script;
//! Windows uses IDesktopWallpaper and reads the previous paths for rollback.
//! These adapters apply the request; layout and rendering policy live elsewhere.
//!
//! Plasma has no per-screen wallpaper API: what it has is `evaluateScript`, which runs a
//! line of its own scripting language inside plasmashell. C# spawned `busctl` for that
//! call; the agent already speaks D-Bus (`sleep.rs`), so it makes the call itself.
//!
//! **Only the binary reaches the real desktop.** Everything goes through [`Desktop`], and
//! an [`Agent`](crate::runtime::Agent) starts with [`NoDesktop`]: `main` installs
//! [`SystemDesktop`]. A test agent once painted the developer's own first screen with an
//! image cut into its tempdir — Plasma matches screens by position, and the fallback
//! output of a test sits at 0,0 like every real primary screen does — and the background
//! then pointed at a file deleted with the tempdir. A test that wants to see what would
//! have been painted hands the agent a [`FakeDesktop`].

use std::collections::BTreeSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Mutex;

use lbm_store::wallpaper_settings::WallpaperStyle;

/// What a [`Desktop`] answers, later: a desktop environment can take its time, or hang.
pub type DesktopFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The desktop environment's background, as the agent writes it.
pub trait Desktop: Send + Sync {
    /// Whether this desktop is one we know how to set.
    fn is_supported(&self) -> DesktopFuture<bool>;
    /// Shows `screens`. An empty list is nothing to do, never a refusal.
    fn apply(&self, screens: Vec<ScreenWallpaper>) -> DesktopFuture<Result<(), String>>;
    /// Complete native references, including stored desktops which are not connected.
    /// An unsupported plugin or an incomplete answer prevents cache collection.
    fn references(&self) -> DesktopFuture<Result<BTreeSet<PathBuf>, String>>;
    /// What each output shows now, read without changing anything: what a remote draws
    /// for its master's map (`peers::wallpapers`).
    fn shown(&self) -> DesktopFuture<Result<Vec<Shown>, String>>;
}

/// What one output of the desktop shows (`lbm_wallpaper::desktop`'s reading, shared with
/// the window).
pub type Shown = lbm_wallpaper::desktop::Entry;

/// The desktop of an agent nobody gave one: it paints nothing and says it cannot, so the
/// editor reports an unsupported desktop and the wallpaper cache is never collected
/// against references nobody read.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDesktop;

const NO_DESKTOP: &str = "This agent has no desktop to paint.";

impl Desktop for NoDesktop {
    fn is_supported(&self) -> DesktopFuture<bool> {
        Box::pin(async { false })
    }
    fn apply(&self, screens: Vec<ScreenWallpaper>) -> DesktopFuture<Result<(), String>> {
        let empty = screens.is_empty();
        Box::pin(async move {
            if empty {
                Ok(())
            } else {
                Err(NO_DESKTOP.into())
            }
        })
    }
    fn references(&self) -> DesktopFuture<Result<BTreeSet<PathBuf>, String>> {
        Box::pin(async { Err(NO_DESKTOP.into()) })
    }
    fn shown(&self) -> DesktopFuture<Result<Vec<Shown>, String>> {
        Box::pin(async { Err(NO_DESKTOP.into()) })
    }
}

/// A desktop that records what it is asked to show, for the tests. Supported, and it
/// takes every request; it vouches for no references, so the cache keeps what it holds.
#[derive(Debug, Default)]
pub struct FakeDesktop {
    applied: Mutex<Vec<Vec<ScreenWallpaper>>>,
    /// What it answers it shows; `None`: it cannot say.
    showing: Mutex<Option<Vec<Shown>>>,
    /// How many times it was asked what it shows.
    asked: std::sync::atomic::AtomicUsize,
}

impl FakeDesktop {
    /// Every non-empty request, oldest first.
    pub fn applied(&self) -> Vec<Vec<ScreenWallpaper>> {
        self.applied.lock().unwrap().clone()
    }

    /// Answers `shown` from now on, as a desktop whose wallpaper the user just changed.
    pub fn show(&self, shown: Vec<Shown>) {
        *self.showing.lock().unwrap() = Some(shown);
    }

    /// How many times it was asked what it shows.
    pub fn asked(&self) -> usize {
        self.asked.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Desktop for FakeDesktop {
    fn is_supported(&self) -> DesktopFuture<bool> {
        Box::pin(async { true })
    }
    fn apply(&self, screens: Vec<ScreenWallpaper>) -> DesktopFuture<Result<(), String>> {
        if !screens.is_empty() {
            self.applied.lock().unwrap().push(screens);
        }
        Box::pin(async { Ok(()) })
    }
    fn references(&self) -> DesktopFuture<Result<BTreeSet<PathBuf>, String>> {
        Box::pin(async { Err("a fake desktop vouches for no references".into()) })
    }
    fn shown(&self) -> DesktopFuture<Result<Vec<Shown>, String>> {
        self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let shown = self.showing.lock().unwrap().clone();
        Box::pin(async move { shown.ok_or_else(|| "a fake desktop shows nothing".to_owned()) })
    }
}

/// This session's desktop: Plasma over D-Bus, or IDesktopWallpaper. `main`'s alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemDesktop(());

impl Desktop for SystemDesktop {
    fn is_supported(&self) -> DesktopFuture<bool> {
        Box::pin(is_supported())
    }
    fn apply(&self, screens: Vec<ScreenWallpaper>) -> DesktopFuture<Result<(), String>> {
        Box::pin(async move { apply_checked(&screens).await })
    }
    fn references(&self) -> DesktopFuture<Result<BTreeSet<PathBuf>, String>> {
        Box::pin(references())
    }
    fn shown(&self) -> DesktopFuture<Result<Vec<Shown>, String>> {
        Box::pin(async {
            tokio::task::spawn_blocking(lbm_wallpaper::desktop::query)
                .await
                .map_err(|e| e.to_string())?
        })
    }
}

/// What one screen should show. The position is how the screen is found: the desktop knows
/// its own screens by where they are, not by the names the layout uses.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenWallpaper {
    /// Native dimensions expected by an editor transaction; legacy callers may omit them.
    pub expected_size: Option<(u32, u32)>,
    pub x: f64,
    pub y: f64,
    /// The image to show; `None` shows `color` instead.
    pub image: Option<PathBuf>,
    pub style: WallpaperStyle,
    /// `#RRGGBB`, as the settings hold it.
    pub color: String,
}

/// [`Desktop::references`] of this session. Private, as everything below that reaches the
/// desktop is: [`SystemDesktop`] is the only way to it.
async fn references() -> Result<BTreeSet<PathBuf>, String> {
    #[cfg(target_os = "linux")]
    {
        let reply = plasma::evaluate(plasma::REFERENCES)
            .await
            .ok_or("Plasma references unavailable")?;
        plasma::parse_references(&reply)
    }
    #[cfg(windows)]
    {
        // IDesktopWallpaper enumerates monitor wallpapers, but Windows also keeps
        // pictures per virtual desktop. That is not a complete reference inventory.
        // Keep the cache until all virtual desktops can be inspected reliably.
        Err("Windows virtual-desktop wallpaper references are not yet supported".into())
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err("Desktop reference query unsupported".into())
    }
}

/// Whether this desktop is one we know how to set. Plasma answers its own script.
#[cfg(target_os = "linux")]
async fn is_supported() -> bool {
    plasma::evaluate(r#"print("ok")"#).await.as_deref() == Some("ok")
}

#[cfg(windows)]
async fn is_supported() -> bool {
    tokio::task::spawn_blocking(crate::windows_wallpaper::supported)
        .await
        .unwrap_or(false)
}

#[cfg(not(any(target_os = "linux", windows)))]
async fn is_supported() -> bool {
    false
}

/// Shows `screens`, and answers whether the desktop took them.
///
/// A wallpaper is never a reason to fail anything: a desktop that refuses, or one that is
/// not Plasma, leaves the background as it was and says so once.
async fn apply_checked(screens: &[ScreenWallpaper]) -> Result<(), String> {
    if screens.is_empty() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        let screens = screens.to_vec();
        tokio::task::spawn_blocking(move || crate::windows_wallpaper::apply(&screens))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(target_os = "linux")]
    {
        if plasma::evaluate(&plasma::script(screens)).await.as_deref() == Some("ok") {
            Ok(())
        } else {
            Err("The desktop refused the wallpaper update.".into())
        }
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        Err("Desktop wallpapers are unsupported on this platform.".into())
    }
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

    pub const REFERENCES: &str = include_str!("plasma_wallpaper_references.js");

    pub fn parse_references(
        reply: &str,
    ) -> Result<std::collections::BTreeSet<std::path::PathBuf>, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            version: u8,
            paths: Vec<String>,
        }
        let reply: Reply = serde_json::from_str(reply).map_err(|e| e.to_string())?;
        if reply.version != 1 {
            return Err("Unknown Plasma reference format".into());
        }
        reply
            .paths
            .into_iter()
            .map(|s| {
                let path = if s.starts_with('/') {
                    std::path::PathBuf::from(&s)
                } else {
                    let url = url::Url::parse(&s).map_err(|e| e.to_string())?;
                    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
                        return Err("Non-local Plasma wallpaper reference".into());
                    }
                    url.to_file_path()
                        .map_err(|_| "Invalid local wallpaper URL".to_owned())?
                };
                if !path.is_absolute()
                    || s.contains('\0')
                    || path.as_os_str().as_encoded_bytes().contains(&0)
                {
                    return Err("Invalid Plasma wallpaper path".into());
                }
                Ok(path)
            })
            .collect()
    }

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
                "var matched=[];var used={{}};",
                "for(var j=0;j<targets.length;j++){{var t=targets[j];var hits=[];",
                "for(var i=0;i<screenCount;i++){{var g=screenGeometry(i);",
                "if(Math.abs(g.x-t.x)<2&&Math.abs(g.y-t.y)<2)hits.push(i);}}",
                "if(hits.length!==1||used[hits[0]])throw new Error(\"Wallpaper screen changed or ambiguous\");",
                "used[hits[0]]=true;matched.push(hits[0]);}}",
                "for(var j=0;j<targets.length;j++){{var t=targets[j];",
                "var i=matched[j];",

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
                "}}",
                "print(\"ok\");"
            ),
            serde_json::Value::Array(targets)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn plasma_reference_urls_preserve_escaped_local_paths() {
        let reply =
            r#"{"version":1,"paths":["file:///tmp/%C3%A9cran%20%23%25.png","/tmp/plain.png"]}"#;
        assert_eq!(
            plasma::parse_references(reply).unwrap(),
            BTreeSet::from([
                PathBuf::from("/tmp/écran #%.png"),
                PathBuf::from("/tmp/plain.png")
            ])
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn ambiguous_plasma_replies_abort_collection() {
        for reply in [
            "",
            "ok",
            "[]",
            r#"{"version":2,"paths":[]}"#,
            r#"{"version":1}"#,
            r#"{"version":1,"paths":["relative.png"]}"#,
            r#"{"version":1,"paths":["https://example.test/image.png"]}"#,
            r#"{"version":1,"paths":["file://remote/image.png"]}"#,
            r#"{"version":1,"paths":["file:///tmp/image.png#fragment"]}"#,
            r#"{"version":1,"paths":["file:///tmp/%00.png"]}"#,
            r#"{"version":1,"paths":[""]}"#,
        ] {
            assert!(plasma::parse_references(reply).is_err(), "{reply}");
        }
    }

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
                expected_size: None,
                x: 0.0,
                y: 0.0,
                image: Some(PathBuf::from("/data/span_1.png")),
                style: WallpaperStyle::Stretch,
                color: "#204060".to_owned(),
            },
            ScreenWallpaper {
                expected_size: None,
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
            expected_size: None,
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

    #[tokio::test]
    async fn nothing_to_show_is_not_a_failure() {
        // Not a call to plasma: an empty list is what a layout with no wallpaper settings
        // produces, and it must not be reported as a desktop that refused.
        assert_eq!(apply_checked(&[]).await, Ok(()));
        assert_eq!(NoDesktop.apply(Vec::new()).await, Ok(()));
    }

    #[tokio::test]
    async fn an_agent_given_no_desktop_paints_nothing_and_says_so() {
        let screen = ScreenWallpaper {
            expected_size: None,
            x: 0.0,
            y: 0.0,
            image: None,
            style: WallpaperStyle::Fill,
            color: "#204060".to_owned(),
        };
        assert!(!NoDesktop.is_supported().await);
        assert!(NoDesktop.apply(vec![screen]).await.is_err());
        assert!(
            NoDesktop.references().await.is_err(),
            "an unread desktop must keep the wallpaper cache from being collected"
        );
        assert!(NoDesktop.shown().await.is_err(), "nor does it read one");
    }
}
