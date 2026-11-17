//! Correlate output reads/actions separately from Save/Start acknowledgements.
use lbm_ipc::displays::{Change, Outputs};
use serde_json::{json, Value};

#[derive(Default)]
pub struct Displays {
    pub outputs: Option<Outputs>,
    pub error: Option<String>,
    pub panel: lbm_ui::displays::Panel,
    generation: u64,
    pending: Option<(u64, bool, u64)>,
}
impl Displays {
    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.outputs = None;
        self.panel.confirm = None;
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn changing(&self) -> bool {
        self.pending.is_some_and(|(_, change, _)| change)
    }
    pub fn needs_read(&self) -> bool {
        !self.busy() && self.outputs.is_none() && self.error.is_none()
    }
    pub fn sent(&mut self, id: u64, change: bool) {
        self.error = None;
        if change {
            self.panel.confirm = None;
        }
        self.pending = Some((id, change, self.generation));
    }
    /// Some(change) means the reply was ours, even if its inventory is obsolete.
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> Option<bool> {
        let (expected, change, generation) = self.pending?;
        if expected != id {
            return None;
        }
        self.pending = None;
        if generation == self.generation {
            match result
                .clone()
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
            {
                Ok(outputs) => self.outputs = Some(outputs),
                Err(error) => {
                    self.outputs = None;
                    self.error = Some(error);
                }
            }
        }
        Some(change)
    }
    pub fn request(
        &self,
        layout_id: &str,
        revision: u64,
        connector: &str,
        change: Change,
    ) -> Option<Value> {
        let outputs = self.outputs.as_ref()?;
        if self.busy() || !outputs.outputs.iter().any(|o| o.connector == connector) {
            return None;
        }
        Some(json!({"LayoutId":layout_id, "LayoutRevision":revision,
            "Signature":outputs.signature, "Connector":connector, "Change":change}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inventory() -> Value {
        json!({"Signature":"live", "Outputs":[{"Connector":"DP-1", "Name":"Main", "Enabled":true, "Primary":true}]})
    }
    #[test]
    fn late_inventories_cannot_restore_a_confirmation_or_acknowledge_another_request() {
        let mut displays = Displays::default();
        displays.sent(1, false);
        displays.panel.confirm = Some(("DP-1".into(), Change::Detach));
        displays.invalidate();
        assert!(displays.panel.confirm.is_none());
        assert_eq!(displays.answered(99, &Ok(inventory())), None);
        assert!(displays.busy());
        assert_eq!(displays.answered(1, &Ok(inventory())), Some(false));
        assert!(displays.outputs.is_none());
        assert!(displays.needs_read());
        displays.sent(2, false);
        displays.answered(2, &Ok(inventory()));
        let request = displays
            .request("layout", 4, "DP-1", Change::Primary)
            .unwrap();
        assert_eq!(request["Signature"], "live");
        assert_eq!(request["LayoutRevision"], 4);
        assert!(displays
            .request("layout", 4, "missing", Change::Attach)
            .is_none());
    }
    #[test]
    fn an_action_error_survives_the_following_topology_rebuild() {
        let mut displays = Displays::default();
        displays.sent(1, true);
        assert_eq!(
            displays.answered(1, &Err("partially applied".into())),
            Some(true)
        );
        displays.invalidate();
        assert_eq!(displays.error.as_deref(), Some("partially applied"));
        assert!(
            !displays.needs_read(),
            "wait for an explicit refresh after the error"
        );
        displays.sent(2, false);
        displays.answered(2, &Ok(inventory()));
        assert!(displays.error.is_none());
        assert!(displays.outputs.is_some());
    }
}
