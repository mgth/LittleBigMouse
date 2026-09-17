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

/// A returned path belongs to the draft and destination captured when Browse was pressed.
/// Comparing the generation also rejects edit/undo/reload cycles returning to identical data.
#[derive(Clone)]
pub struct FileChoice {
    key: Option<Key>,
    generation: u64,
    draft: LayoutWallpaperSettings,
    screen: Option<String>,
}
impl Editor {
    pub fn choose_file(&self, selected: Option<&str>) -> Option<FileChoice> {
        use lbm_store::wallpaper_settings::{ScreenWallpaperKind, WallpaperMode};
        if self.busy() {
            return None;
        }
        let draft = self.draft.as_ref()?;
        let screen = if draft.mode == WallpaperMode::Span {
            None
        } else {
            let id = selected?;
            if draft.per_screen.get(id)?.kind != ScreenWallpaperKind::Image {
                return None;
            }
            Some(id.to_owned())
        };
        Some(FileChoice {
            key: self.key.clone(),
            generation: self.preview_generation,
            draft: draft.clone(),
            screen,
        })
    }
    /// A valid selection requests a local preview, including reselecting the same file.
    /// No IPC/save: reselecting a repaired file can clear a previous preview error.
    pub fn file_chosen(
        &mut self,
        choice: FileChoice,
        selected: Option<&str>,
        result: crate::image_picker::Selection,
    ) -> bool {
        if self.busy()
            || self.key != choice.key
            || self.preview_generation != choice.generation
            || self.draft.as_ref() != Some(&choice.draft)
            || choice
                .screen
                .as_deref()
                .is_some_and(|id| Some(id) != selected)
        {
            return false;
        }
        let path = match result {
            Ok(Some(path)) => path,
            Ok(None) => return false,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        let draft = self.draft.as_mut().unwrap();
        let dest = match choice.screen {
            Some(id) => &mut draft.per_screen.get_mut(&id).unwrap().image_path,
            None => &mut draft.span_image_path,
        };
        *dest = Some(path);
        self.preview_generation += 1;
        self.error = None;
        self.status = Some("Image selected. Preview is local; Apply updates the desktop.".into());
        true
    }
}
impl FileChoice {
    pub fn current_path(&self) -> Option<&str> {
        match &self.screen {
            Some(id) => self.draft.per_screen.get(id)?.image_path.as_deref(),
            None => self.draft.span_image_path.as_deref(),
        }
    }
}
