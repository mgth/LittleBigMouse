use crate::vcp::Key;
use lbm_ipc::remote::{Action, Brand, Command, State};
use serde_json::{json, Value};
#[derive(Default)]
pub struct Controller {
    key: Option<Key>,
    generation: u64,
    pending: Option<(u64, u64, bool)>,
    pub state: Option<State>,
    pub error: Option<String>,
    pub panel: lbm_ui::remote::Panel,
}
impl Controller {
    pub fn select(&mut self, key: Option<Key>) {
        if key != self.key {
            self.generation += 1;
            self.key = key;
            self.state = None;
            self.error = None;
            self.panel = Default::default();
            if self
                .key
                .as_ref()
                .is_some_and(|k| k.monitor.starts_with("HEC"))
            {
                self.panel.brand = Brand::Hisense;
            }
        }
    }
    pub fn switch(&mut self, brand: Brand) {
        self.generation += 1;
        self.panel.brand = brand;
        self.state = None;
        self.error = None;
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn needs_read(&self) -> bool {
        self.key.is_some() && !self.busy() && self.state.is_none() && self.error.is_none()
    }
    pub fn request(&mut self, id: u64, command: Command) -> Option<Value> {
        if self.busy() {
            return None;
        }
        let key = self.key.as_ref()?;
        let load = matches!(
            command.action,
            Action::Info | Action::Save { .. } | Action::Pair { .. } | Action::RequestPin
        );
        self.pending = Some((id, self.generation, load));
        self.error = None;
        Some(
            json!({"LayoutId":key.layout,"LayoutRevision":key.revision,"Monitor":key.monitor,"Command":command}),
        )
    }
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> bool {
        let Some((expected, generation, load)) = self.pending else {
            return false;
        };
        if expected != id {
            return false;
        }
        self.pending = None;
        if generation == self.generation {
            match result
                .clone()
                .and_then(|v| serde_json::from_value::<State>(v).map_err(|e| e.to_string()))
            {
                Ok(state) => {
                    self.panel.observed(&state);
                    if load {
                        self.panel.load(&state);
                    }
                    self.state = Some(state);
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
        true
    }
    pub fn recognized(&self) -> bool {
        self.key
            .as_ref()
            .is_some_and(|k| k.monitor.starts_with("SAM") || k.monitor.starts_with("HEC"))
            || self.state.as_ref().is_some_and(|s| {
                !s.samsung.ip_address.is_empty() || !s.hisense.ip_address.is_empty()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(id: &str) -> Key {
        Key {
            layout: "layout".into(),
            revision: 1,
            monitor: id.into(),
        }
    }
    #[test]
    fn late_remote_responses_cannot_repopulate_another_monitor_or_brand() {
        let mut c = Controller::default();
        c.select(Some(key("SAM1")));
        c.request(
            1,
            Command {
                brand: Brand::Samsung,
                action: Action::Info,
            },
        );
        c.select(Some(key("HEC1")));
        assert!(!c.answered(2, &Err("other request".into())));
        c.answered(1, &Ok(serde_json::to_value(State::default()).unwrap()));
        assert!(c.state.is_none());
        assert!(c.needs_read());
        assert_eq!(c.panel.brand, Brand::Hisense);
        c.request(
            3,
            Command {
                brand: Brand::Hisense,
                action: Action::Info,
            },
        );
        c.switch(Brand::Samsung);
        c.answered(3, &Err("old Hisense error".into()));
        assert!(c.error.is_none());
        assert!(c.needs_read());
    }
}
