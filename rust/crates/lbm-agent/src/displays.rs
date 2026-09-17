use lbm_ipc::displays::{Output, Outputs};
use lbm_layout::linux::LinuxMonitor;

pub fn outputs(live: Vec<LinuxMonitor>) -> Outputs {
    Outputs {
        signature: lbm_display::linux::display_signature(&live),
        outputs: live
            .into_iter()
            .map(|m| Output {
                name: m
                    .edid
                    .as_ref()
                    .and_then(|e| e.model.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| m.connector_name.clone()),
                connector: m.connector_name,
                enabled: m.enabled,
                primary: m.primary,
            })
            .collect(),
    }
}
