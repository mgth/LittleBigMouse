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

/// Windows uses stable monitor interface paths as connectors, including detached targets.
pub fn windows(live: lbm_display::windows::topology::Configuration) -> Result<Outputs, String> {
    Ok(Outputs {
        signature: live.signature(),
        outputs: live
            .outputs()?
            .into_iter()
            .map(|m| Output {
                name: if m.name.is_empty() {
                    m.connector.clone()
                } else {
                    m.name
                },
                connector: m.connector,
                enabled: m.enabled,
                primary: m.primary,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_display::windows::topology::{Configuration, Id, Path, PathId};
    #[test]
    fn detached_windows_targets_keep_their_exact_identity_through_the_ipc_document() {
        let connector = r"\\?\DISPLAY#SAME035#5&123&0&UID1#{guid}";
        let live = Configuration {
            paths: vec![Path {
                id: PathId {
                    source: Id { adapter: 1, id: 0 },
                    target: Id { adapter: 1, id: 42 },
                },
                connector: connector.into(),
                name: "Studio display".into(),
                available: true,
                active: false,
            }],
            ..Default::default()
        };
        let signature = live.signature();
        let output = windows(live).unwrap();
        let json = serde_json::to_string(&output).unwrap();
        let output: Outputs = serde_json::from_str(&json).unwrap();
        assert_eq!(output.signature, signature);
        assert_eq!(output.outputs[0].connector, connector);
        assert_eq!(output.outputs[0].name, "Studio display");
        assert!(!output.outputs[0].enabled && !output.outputs[0].primary);
    }
}
