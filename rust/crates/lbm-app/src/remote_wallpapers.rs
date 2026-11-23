//! The remotes' wallpapers on this machine's map (docs/multi-pc.md, "Wallpapers").
//!
//! The window reads this desktop's wallpaper itself (`desktop_wallpaper`), but a remote's
//! is on another machine: its agent draws each screen small and sends it to this machine's
//! agent, which keeps the latest and says in its `Peers` state when they changed
//! (`WallpaperRevision`). This asks for them then (`PeerWallpapers`), and only then —
//! the pictures never travel in the state, which every frontend gets at every change —
//! decodes them once, and says which monitor of the layout each belongs to: a screen
//! showing a remote gets that remote's picture, so a panel two machines share shows the
//! wallpaper of the machine it shows.

use std::collections::HashMap;

use base64::Engine;
use image::RgbaImage;
use lbm_ipc::peer::PeerWallpaper;
use lbm_layout::model::Layout;
use serde_json::{json, Value};

/// The agent's capability for the request.
pub const CAPABILITY: &str = "PeerWallpapers";

/// The pictures of one remote: its revision as the agent stamped it, and one picture per
/// display, by the display's id on that machine.
#[derive(Default)]
struct Fetched {
    revision: u64,
    pictures: HashMap<String, RgbaImage>,
}

#[derive(Default)]
pub struct RemoteWallpapers {
    /// The agent announced the capability; without it nothing is asked.
    offered: bool,
    /// The revision the agent holds, per remote on the map.
    known: HashMap<String, u64>,
    fetched: HashMap<String, Fetched>,
    /// The requests out, and the remote each is about.
    asking: HashMap<u64, String>,
    /// The pictures changed since [`RemoteWallpapers::take_changed`] last said so.
    changed: bool,
}

impl RemoteWallpapers {
    /// A new connection: the pictures are kept — the remotes are the same ones until the
    /// agent says otherwise, and the map does not flicker — and asked for again if their
    /// revision is not the one known.
    pub fn connected(&mut self, capabilities: &[String]) {
        self.offered = capabilities.iter().any(|c| c == CAPABILITY);
        self.asking.clear();
        self.known.clear();
    }

    pub fn disconnected(&mut self) {
        self.offered = false;
        self.asking.clear();
    }

    /// The agent's `Peers` state: the revision of every remote on the map
    /// (`machines` — the merged remotes). A remote no longer on the map takes its pictures
    /// with it.
    pub fn heard(&mut self, state: &Value, machines: &[String]) {
        self.known.clear();
        for entry in state["Peers"].as_array().into_iter().flatten() {
            let Some(machine_id) = entry["MachineId"].as_str() else {
                continue;
            };
            if machines.iter().any(|m| m == machine_id) {
                let revision = entry["WallpaperRevision"].as_u64().unwrap_or(0);
                self.known.insert(machine_id.to_owned(), revision);
            }
        }
        let before = self.fetched.len();
        self.fetched.retain(|machine, fetched| {
            // Nothing held any more (revision 0) is no picture, not the old one.
            self.known
                .get(machine)
                .is_some_and(|&known| known > 0 || fetched.revision == 0)
        });
        self.changed |= self.fetched.len() != before;
    }

    /// The remotes whose pictures are to be asked for now: the agent holds others than
    /// those in hand, and no request is out for them.
    pub fn wanted(&self) -> Vec<(String, Value)> {
        if !self.offered {
            return Vec::new();
        }
        let mut wanted: Vec<_> = self
            .known
            .iter()
            .filter(|(machine, &known)| {
                known > 0
                    && self
                        .fetched
                        .get(*machine)
                        .is_none_or(|f| f.revision != known)
                    && !self.asking.values().any(|m| m == *machine)
            })
            .map(|(machine, _)| (machine.clone(), json!({ "MachineId": machine })))
            .collect();
        wanted.sort_by(|a, b| a.0.cmp(&b.0));
        wanted
    }

    /// A request for `machine`'s pictures went out under `id`.
    pub fn sent(&mut self, id: u64, machine: String) {
        self.asking.insert(id, machine);
    }

    /// An answer, if it is one of ours. A refusal leaves the pictures as they were; the
    /// next change of revision asks again.
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> bool {
        let Some(machine) = self.asking.remove(&id) else {
            return false;
        };
        let Ok(answer) = result else {
            // Not asked again for this revision: an agent that refuses once would refuse
            // at every frame.
            if let Some(&known) = self.known.get(&machine) {
                self.fetched.entry(machine).or_default().revision = known;
            }
            return true;
        };
        let revision = answer["Revision"].as_u64().unwrap_or(0);
        let pictures = answer["Wallpapers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| serde_json::from_value::<PeerWallpaper>(w.clone()).ok())
            .filter_map(|w| Some((w.display.clone(), decode(&w)?)))
            .collect();
        self.fetched.insert(machine, Fetched { revision, pictures });
        self.changed = true;
        true
    }

    /// Whether the pictures changed since this was last asked: the textures are made
    /// again then, and only then.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Every picture in hand, by the monitor-independent key [`key_of`] gives.
    pub fn pictures(&self) -> impl Iterator<Item = (String, &RgbaImage)> {
        self.fetched.iter().flat_map(|(machine, fetched)| {
            fetched
                .pictures
                .iter()
                .map(move |(display, pixels)| (key(machine, display), pixels))
        })
    }
}

fn key(machine: &str, display: &str) -> String {
    format!("{machine}/{display}")
}

/// The picture key of the screen `monitor` shows, when that is a remote's: its machine
/// and its display id there. `None` for a monitor showing this machine, which keeps this
/// desktop's picture.
pub fn key_of(layout: &Layout, monitor: &str) -> Option<String> {
    let monitor = layout.monitor(monitor)?;
    let source = &layout.source(monitor.active_source.as_deref()?)?.source;
    let machine = source.host.as_deref()?;
    // `{display}@{first 8 characters of the machine id}` (lbm_layout::remote).
    let (display, _) = source.id.rsplit_once('@')?;
    Some(key(machine, display))
}

/// A picture as sent: the image decoded, or the colour as a single pixel, which the map
/// stretches over the screen.
pub fn decode(wallpaper: &PeerWallpaper) -> Option<RgbaImage> {
    if let Some(image) = &wallpaper.image {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(image)
            .ok()?;
        return image::load_from_memory(&bytes)
            .ok()
            .map(|picture| picture.to_rgba8());
    }
    let color = lbm_wallpaper::render::color(wallpaper.color.as_deref()?).ok()?;
    Some(RgbaImage::from_pixel(1, 1, image::Rgba(color)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(revision: u64) -> Value {
        json!({ "Peers": [
            { "MachineId": "0a", "WallpaperRevision": revision },
            { "MachineId": "ff", "WallpaperRevision": 9 },
        ]})
    }

    fn jpeg(color: [u8; 3]) -> String {
        let mut bytes = Vec::new();
        image::RgbImage::from_pixel(8, 4, image::Rgb(color))
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn pictures_are_asked_for_once_per_revision_and_only_for_remotes_on_the_map() {
        let mut remotes = RemoteWallpapers::default();
        remotes.connected(&["MultiPc".into(), CAPABILITY.into()]);
        remotes.heard(&state(1), &["0a".into()]);
        let wanted = remotes.wanted();
        assert_eq!(wanted, [("0a".to_owned(), json!({ "MachineId": "0a" }))]);
        remotes.sent(7, "0a".into());
        assert!(remotes.wanted().is_empty(), "one request at a time");
        let answer = json!({ "Revision": 1, "Wallpapers": [
            { "Display": "eDP-1", "Image": jpeg([200, 10, 10]) },
            { "Display": "HDMI-1", "Color": "#203040" },
            { "Display": "DP-1", "Image": "not base64!" },
        ]});
        assert!(remotes.answered(7, &Ok(answer)));
        assert!(remotes.take_changed());
        assert!(!remotes.take_changed());
        let mut keys: Vec<_> = remotes
            .pictures()
            .map(|(k, p)| (k, p.dimensions()))
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                ("0a/HDMI-1".to_owned(), (1, 1)),
                ("0a/eDP-1".to_owned(), (8, 4))
            ],
            "what does not decode is left out"
        );
        remotes.heard(&state(1), &["0a".into()]);
        assert!(
            remotes.wanted().is_empty(),
            "the same revision is not asked again"
        );
        remotes.heard(&state(2), &["0a".into()]);
        assert_eq!(remotes.wanted().len(), 1);

        // The remote leaves the map: its pictures go with it.
        remotes.heard(&state(2), &[]);
        assert_eq!(remotes.pictures().count(), 0);
        assert!(remotes.take_changed());
    }

    #[test]
    fn an_agent_without_the_capability_is_never_asked() {
        let mut remotes = RemoteWallpapers::default();
        remotes.connected(&["MultiPc".into()]);
        remotes.heard(&state(1), &["0a".into()]);
        assert!(remotes.wanted().is_empty());
    }

    #[test]
    fn a_refusal_is_not_asked_again_until_the_revision_changes() {
        let mut remotes = RemoteWallpapers::default();
        remotes.connected(&[CAPABILITY.into()]);
        remotes.heard(&state(3), &["0a".into()]);
        remotes.sent(1, "0a".into());
        assert!(remotes.answered(1, &Err("no".into())));
        assert!(remotes.wanted().is_empty());
        remotes.heard(&state(4), &["0a".into()]);
        assert_eq!(remotes.wanted().len(), 1);
        assert!(!remotes.answered(99, &Ok(Value::Null)), "not ours");
    }
}
