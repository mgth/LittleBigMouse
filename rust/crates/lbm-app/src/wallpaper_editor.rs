//! Request ownership and stale-layout protection are independent of layout Save/Live.
use lbm_store::wallpaper_settings::LayoutWallpaperSettings;
use serde_json::{json, Value};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub layout: String,
    pub revision: u64,
}
enum Pending {
    Read(u64, Key),
    Apply(u64, Key, LayoutWallpaperSettings),
}
#[derive(Default)]
pub struct Editor {
    pub key: Option<Key>,
    pub draft: Option<LayoutWallpaperSettings>,
    base: Option<LayoutWallpaperSettings>,
    pending: Option<Pending>,
    pub supported: bool,
    pub stale: bool,
    pub error: Option<String>,
    pub status: Option<String>,
    pub preview_generation: u64,
}
impl Editor {
    pub fn dirty(&self) -> bool {
        self.draft != self.base
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn disconnect(&mut self) {
        self.pending = None;
        self.supported = false;
        self.stale = true;
        self.preview_generation += 1;
    }
    pub fn observes(&mut self, key: &Key) {
        if !self.stale && self.key.as_ref().is_some_and(|old| old != key) {
            self.pending = None;
            self.stale = true;
            self.preview_generation += 1;
        }
    }
    pub fn read(&mut self, id: u64, key: Key) -> Value {
        self.stale = false;
        self.supported = false;
        self.pending = Some(Pending::Read(id, key.clone()));
        self.key = Some(key.clone());
        self.error = None;
        self.status = None;
        self.preview_generation += 1;
        json!({"LayoutId":key.layout,"LayoutRevision":key.revision})
    }
    pub fn apply(&mut self, id: u64) -> Option<Value> {
        if self.busy() || self.stale || !self.supported {
            return None;
        }
        let key = self.key.clone()?;
        let settings = self.draft.clone()?;
        let extra =
            json!({"LayoutId":key.layout,"LayoutRevision":key.revision,"Settings":settings});
        self.pending = Some(Pending::Apply(id, key, settings));
        self.error = None;
        self.status = None;
        Some(extra)
    }
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> bool {
        let matches = match &self.pending {
            Some(Pending::Read(wanted, _)) | Some(Pending::Apply(wanted, _, _)) => *wanted == id,
            None => false,
        };
        if !matches {
            return false;
        }
        match self.pending.take().unwrap() {
            Pending::Read(_, key) => {
                if self.key.as_ref() != Some(&key) {
                    return true;
                }
                match result.as_ref().map_err(Clone::clone).and_then(|v| {
                    serde_json::from_value::<LayoutWallpaperSettings>(v["Settings"].clone())
                        .map(|s| (s, v["Supported"].as_bool() == Some(true)))
                        .map_err(|e| e.to_string())
                }) {
                    Ok((settings, supported)) => {
                        self.draft = Some(settings.clone());
                        self.base = Some(settings);
                        self.supported = supported;
                        self.stale = false;
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Pending::Apply(_, key, sent) => {
                if self.key.as_ref() != Some(&key) {
                    return true;
                }
                match result {
                    Ok(v) if v["Saved"].as_bool() == Some(true) => {
                        self.base = Some(sent);
                        if v["Applied"].as_bool() == Some(true) {
                            self.status = Some("Wallpaper saved and applied.".into());
                        } else {
                            self.error = Some(format!(
                                "Settings saved; desktop update failed: {}",
                                v["Error"].as_str().unwrap_or("unknown failure")
                            ));
                        }
                    }
                    Ok(_) => {
                        self.error =
                            Some("The agent did not acknowledge the wallpaper update.".into())
                    }
                    Err(e) => self.error = Some(e.clone()),
                }
            }
        }
        true
    }
    pub fn reset(&mut self) {
        self.draft = self.base.clone();
        self.error = None;
        self.status = None;
        self.preview_generation += 1;
    }
}
