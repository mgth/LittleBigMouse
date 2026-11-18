use super::{Entry, Rect, Repeat, Style};
use std::{path::PathBuf, process::Command};

// No writes: read the live containment for each active screen, preserving its group.
const SCRIPT: &str = r##"var out=[];
for(var i=0;i<screenCount;i++) {
    var g=screenGeometry(i), d=desktopForScreen(i), previous=d.currentConfigGroup;
    try {
        d.currentConfigGroup=["Wallpaper",d.wallpaperPlugin,"General"];
        out.push({x:g.x,y:g.y,width:g.width,height:g.height,plugin:d.wallpaperPlugin,
            image:d.readConfig("Image",""),fill:d.readConfig("FillMode",2),
            color:String(d.readConfig("Color","#000000"))});
    } finally { d.currentConfigGroup=previous; }
}
print(JSON.stringify(out));"##;

pub(super) fn query() -> Result<Vec<Entry>, String> {
    let output = Command::new("busctl")
        .args([
            "--user",
            "--timeout=3",
            "--json=short",
            "call",
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell",
            "evaluateScript",
            "s",
            SCRIPT,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Plasma wallpaper unavailable".into());
    }
    let reply: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    parse(reply["data"][0].as_str().ok_or("Invalid Plasma reply")?)
}
fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let values: Vec<serde_json::Value> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(values
        .into_iter()
        .filter_map(|v| {
            let path = match v["plugin"].as_str()? {
                "org.kde.image" => Some(resolve(v["image"].as_str()?)?),
                "org.kde.color" => None,
                _ => return None,
            };
            // Qt's `Image.fillMode`, which is what Plasma stores.
            let (style, repeat) = match v["fill"].as_i64().unwrap_or(2) {
                0 => (Style::Stretch, Repeat::Both),
                1 => (Style::Fit, Repeat::Both),
                3 => (Style::Tile, Repeat::Both),
                4 => (Style::Tile, Repeat::Vertically),
                5 => (Style::Tile, Repeat::Horizontally),
                6 => (Style::Center, Repeat::Both),
                _ => (Style::Fill, Repeat::Both),
            };
            Some(Entry {
                bounds: Rect::new(
                    v["x"].as_f64()?,
                    v["y"].as_f64()?,
                    v["width"].as_f64()?,
                    v["height"].as_f64()?,
                ),
                path,
                style,
                repeat,
                color: color(v["color"].as_str().unwrap_or("#000000")),
            })
        })
        .collect())
}
fn color(text: &str) -> [u8; 4] {
    if let Ok(color) = lbm_wallpaper::render::color(text) {
        return color;
    }
    let channels: Option<Vec<u8>> = text.split(',').map(|s| s.trim().parse().ok()).collect();
    match channels.as_deref() {
        Some([r, g, b, ..]) => [*r, *g, *b, 255],
        _ => [0, 0, 0, 255],
    }
}
fn resolve(value: &str) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }
    let path = if value.starts_with("file:") {
        url::Url::parse(value).ok()?.to_file_path().ok()?
    } else {
        PathBuf::from(value)
    };
    if !path.is_dir() {
        return Some(path);
    }
    // Wallpaper packages contain several resolutions; prefer the largest image.
    std::fs::read_dir(path.join("contents/images"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension().is_some_and(|e| {
                ["png", "jpg", "jpeg", "bmp"]
                    .iter()
                    .any(|x| e.eq_ignore_ascii_case(x))
            })
        })
        .max_by_key(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            stem.split_once('x')
                .and_then(|(w, h)| Some(w.parse::<u64>().ok()?.saturating_mul(h.parse().ok()?)))
                .unwrap_or(0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_entries_keep_logical_geometry_uri_escaping_and_solid_colors() {
        let entries = parse(r##"[
          {"x":-1536,"y":0,"width":1536,"height":864,"plugin":"org.kde.image","image":"file:///tmp/a%20b%23c.png","fill":1,"color":"#203040"},
          {"x":0,"y":0,"width":1536,"height":864,"plugin":"org.kde.color","color":"32,64,96"},
          {"x":0,"y":0,"width":100,"height":100,"plugin":"unsupported"}
        ]"##).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, Some(PathBuf::from("/tmp/a b#c.png")));
        assert_eq!(entries[0].style, Style::Fit);
        assert_eq!(entries[0].bounds.x(), -1536.);
        assert_eq!(entries[1].color, [32, 64, 96, 255]);
        assert!(entries[1].path.is_none());
    }
    #[test]
    fn qt_fill_modes_keep_the_axis_they_repeat_on() {
        for (fill, style, repeat) in [
            (3, Style::Tile, Repeat::Both),
            (4, Style::Tile, Repeat::Vertically),
            (5, Style::Tile, Repeat::Horizontally),
            (6, Style::Center, Repeat::Both),
        ] {
            let entries = parse(&format!(
                r#"[{{"x":0,"y":0,"width":10,"height":10,"plugin":"org.kde.image",
                     "image":"/tmp/a.png","fill":{fill}}}]"#
            ))
            .unwrap();
            assert_eq!((entries[0].style, entries[0].repeat), (style, repeat));
        }
    }
    #[test]
    fn wallpaper_packages_choose_the_largest_image() {
        let dir = tempfile::tempdir().unwrap();
        let images = dir.path().join("contents/images");
        std::fs::create_dir_all(&images).unwrap();
        for name in ["1920x1080.jpg", "3840x2160.png", "metadata.json"] {
            std::fs::write(images.join(name), "").unwrap();
        }
        assert_eq!(
            resolve(dir.path().to_str().unwrap()),
            Some(images.join("3840x2160.png"))
        );
    }
}
