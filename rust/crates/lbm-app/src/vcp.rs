//! A response belongs to one selection and display revision, never to the next screen.
use lbm_ipc::vcp::{Monitor, Write};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub layout: String,
    pub revision: u64,
    pub monitor: String,
}
#[derive(Default)]
pub struct Controller {
    key: Option<Key>,
    pub monitor: Option<Monitor>,
    pub error: Option<String>,
    pending: Option<(u64, Key)>,
    pub panel: lbm_ui::vcp::Panel,
}
impl Controller {
    pub fn select(&mut self, key: Option<Key>) {
        if self.key != key {
            self.key = key;
            self.monitor = None;
            self.error = None;
            self.panel = Default::default();
        }
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn needs_read(&self) -> bool {
        self.key.is_some() && !self.busy() && self.monitor.is_none() && self.error.is_none()
    }
    pub fn request(&mut self, id: u64, write: Option<Write>) -> Option<Value> {
        if self.busy() {
            return None;
        }
        let key = self.key.clone()?;
        self.pending = Some((id, key.clone()));
        self.error = None;
        Some(
            json!({"LayoutId":key.layout, "LayoutRevision":key.revision, "Monitor":key.monitor, "Write":write}),
        )
    }
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> bool {
        let Some((expected, key)) = self.pending.as_ref() else {
            return false;
        };
        if *expected != id {
            return false;
        }
        let current = Some(key) == self.key.as_ref();
        self.pending = None;
        if current {
            match result
                .clone()
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
            {
                Ok(monitor) => {
                    self.monitor = Some(monitor);
                    self.error = None;
                }
                Err(error) => {
                    self.monitor = None;
                    self.error = Some(error);
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(name: &str) -> Key {
        Key {
            layout: "layout".into(),
            revision: 2,
            monitor: name.into(),
        }
    }
    #[test]
    fn selection_and_revision_changes_discard_late_values_but_release_the_pending_request() {
        let mut c = Controller::default();
        c.select(Some(key("A")));
        assert!(c.needs_read());
        assert_eq!(c.request(1, None).unwrap()["Monitor"], "A");
        c.select(Some(key("B")));
        assert!(!c.answered(9, &Err("other request".into())));
        assert!(c.busy());
        assert!(c.answered(1, &Err("screen A failed".into())));
        assert!(c.error.is_none());
        assert!(c.needs_read());
        c.request(2, None);
        let mut newer = key("B");
        newer.revision += 1;
        c.select(Some(newer));
        let state = Monitor {
            token: 1,
            connection: "old".into(),
            features: vec![],
            note: None,
        };
        c.answered(2, &Ok(serde_json::to_value(state).unwrap()));
        assert!(c.monitor.is_none());
        assert!(c.needs_read());
    }
    #[test]
    fn failed_writes_drop_optimistic_values_and_wait_for_explicit_retry() {
        let mut c = Controller::default();
        c.select(Some(key("A")));
        c.request(
            1,
            Some(Write {
                token: 1,
                code: 0x10,
                value: 80,
            }),
        );
        c.answered(1, &Err("unplugged".into()));
        assert!(c.monitor.is_none());
        assert!(!c.needs_read());
        assert!(c.request(2, None).is_some());
        assert!(c.error.is_none());
    }
}
