//! The rulers on the remotes' screens (docs/multi-pc.md, "Rulers on a remote's screens").
//!
//! "Rulers on screens" draws, on every screen, a ruler graduated in millimetres from the
//! selected panel. This window draws them on this machine's screens itself; a remote's
//! screens are another desktop, so their rulers are computed here — from the same layout,
//! in millimetres, and in that remote's own pixels, which its report gave — and sent to
//! the agent (`PeerOverlay`), which passes them on. The remote checks them against its
//! screens and draws them, passive: the pointer goes through them there.
//!
//! Sent only when they change, never on a timer: a drag sends a new set as the ruler's
//! zero moves, a still layout sends nothing. Turning the rulers off sends an empty set to
//! every remote that had some; the agent takes them down by itself if this window goes.

use std::collections::{BTreeMap, HashMap};

use lbm_layout::model::Layout;
use lbm_overlay::scene::{self, Band, Mode};
use serde_json::{json, Value};

/// The agent's capability for the request.
pub const CAPABILITY: &str = "PeerOverlay";

/// The rulers each remote gets, by machine id, and why some could not be computed.
pub type Wanted = (BTreeMap<String, Vec<Band>>, Vec<String>);

/// The rulers each remote machine on the map gets, by machine id, for `selected`; a
/// remote with no screen on its desktop gets none. Errors are per remote, in the words
/// the map shows: one remote's odd screen does not take the others' rulers away.
pub fn wanted(layout: &Layout, selected: Option<&str>) -> Wanted {
    let mut machines: Vec<String> = layout
        .sources()
        .filter_map(|s| s.source.host.clone())
        .collect();
    machines.sort();
    machines.dedup();
    let mut wanted = BTreeMap::new();
    let mut errors = Vec::new();
    for machine in machines {
        let view = machine_view(layout, &machine);
        if !view.sources().any(|s| {
            s.source.host.as_deref() == Some(machine.as_str()) && s.source.attached_to_desktop
        }) {
            continue;
        }
        match scene::from_layout(&view, Mode::Rulers, selected) {
            Ok(bands) => {
                wanted.insert(machine, bands);
            }
            Err(error) => errors.push(error),
        }
    }
    (wanted, errors)
}

/// `layout` as the screens of `machine` alone are drawn: every other machine's sources
/// off its desktop. The monitors stay, the selected one with them, so that the ruler
/// still measures it.
fn machine_view(layout: &Layout, machine: &str) -> Layout {
    let mut view = layout.clone();
    let others: Vec<_> = layout
        .sources()
        .filter(|s| s.source.host.as_deref() != Some(machine))
        .cloned()
        .collect();
    for mut source in others {
        source.source.attached_to_desktop = false;
        view.add_or_update_source(source);
    }
    view
}

#[derive(Default)]
pub struct RemoteRulers {
    /// The agent announced the capability; without it nothing is sent.
    offered: bool,
    /// What the agent was last asked to show, per remote.
    sent: HashMap<String, Vec<Band>>,
    /// The requests out, and the remote each is about.
    asking: HashMap<u64, String>,
    /// Why some remote's rulers could not be computed here, asked or shown there.
    computed: Vec<String>,
    refused: BTreeMap<String, String>,
    remote: BTreeMap<String, String>,
}

impl RemoteRulers {
    /// A new connection to the agent. Whatever an earlier one asked for went with it: the
    /// agent takes down what a frontend showed when it goes.
    pub fn connected(&mut self, capabilities: &[String]) {
        *self = RemoteRulers {
            offered: capabilities.iter().any(|c| c == CAPABILITY),
            ..Default::default()
        };
    }

    pub fn disconnected(&mut self) {
        *self = RemoteRulers::default();
    }

    /// The requests that make the agent show `wanted` and nothing else, each with the
    /// remote it is about; empty when it already does, or cannot.
    pub fn changes(
        &mut self,
        wanted: BTreeMap<String, Vec<Band>>,
        errors: Vec<String>,
    ) -> Vec<(String, Value)> {
        self.computed = errors;
        if !self.offered {
            return Vec::new();
        }
        let mut requests = Vec::new();
        let mut gone: Vec<String> = self
            .sent
            .keys()
            .filter(|m| !wanted.contains_key(*m))
            .cloned()
            .collect();
        gone.sort();
        for machine in gone {
            self.sent.remove(&machine);
            self.refused.remove(&machine);
            requests.push((machine.clone(), json!({"MachineId": machine, "Bands": []})));
        }
        for (machine, bands) in wanted {
            if self.sent.get(&machine) == Some(&bands) {
                continue;
            }
            self.refused.remove(&machine);
            requests.push((
                machine.clone(),
                json!({"MachineId": machine, "Bands": bands}),
            ));
            self.sent.insert(machine, bands);
        }
        requests
    }

    /// Takes down everything sent: the requests that do it.
    pub fn clear(&mut self) -> Vec<(String, Value)> {
        self.changes(BTreeMap::new(), Vec::new())
    }

    pub fn sent(&mut self, id: u64, machine: String) {
        self.asking.insert(id, machine);
    }

    /// An answer, if it is to one of these requests.
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> bool {
        let Some(machine) = self.asking.remove(&id) else {
            return false;
        };
        match result {
            Ok(_) => {
                self.refused.remove(&machine);
            }
            Err(error) => {
                self.refused.insert(machine, error.clone());
            }
        }
        true
    }

    /// The agent's `Peers` state: why a remote says it does not show its rulers.
    pub fn heard(&mut self, state: &Value) {
        self.remote.clear();
        for entry in state["Peers"].as_array().into_iter().flatten() {
            if let (Some(machine), Some(error)) =
                (entry["MachineId"].as_str(), entry["OverlayError"].as_str())
            {
                let name = entry["Name"].as_str().unwrap_or(machine);
                self.remote
                    .insert(machine.to_owned(), format!("{name}: {error}"));
            }
        }
    }

    /// What the user is told about the remotes' rulers while they are on, if anything
    /// went wrong.
    pub fn error(&self) -> Option<String> {
        let lines: Vec<&str> = self
            .computed
            .iter()
            .map(String::as_str)
            .chain(self.refused.values().map(String::as_str))
            .chain(
                self.remote
                    .iter()
                    .filter(|(m, _)| self.sent.contains_key(*m))
                    .map(|(_, e)| e.as_str()),
            )
            .collect();
        (!lines.is_empty()).then(|| format!("Rulers on the other machines: {}", lines.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_overlay::scene::{Content, Pixels, Side};

    fn band(monitor: &str, zero_mm: f64) -> Band {
        let screen = Pixels {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        Band {
            monitor: monitor.into(),
            connector: "DP-1".into(),
            screen,
            rect: Pixels {
                height: 100,
                ..screen
            },
            side: Side::Top,
            scale: 1.0,
            axis_mm: 527.0,
            cross_pixels_per_mm: 3.6,
            content: Content::Ruler {
                zero_mm,
                measured_mm: 600.0,
            },
            selected: None,
            passive: false,
        }
    }

    fn wanted(entries: &[(&str, f64)]) -> BTreeMap<String, Vec<Band>> {
        entries
            .iter()
            .map(|(m, zero)| (m.to_string(), vec![band(m, *zero)]))
            .collect()
    }

    /// This machine's fallback screen and a remote's laptop panel, built as the window
    /// builds its layout.
    fn layout_with_a_remote() -> (Layout, String) {
        let local = lbm_layout::linux::LinuxMonitor::fallback();
        let remote = crate::machines::Remote {
            machine_id: "0a00".into(),
            name: "laptop".into(),
            displays: vec![serde_json::from_value(json!({"Id": "eDP-1",
                "Connector": "eDP-1", "X": 0, "Y": 0, "Width": 1920, "Height": 1200,
                "Scale": 1.25, "EffectiveDpi": 120.0, "Attached": true,
                "WidthMm": 302.0, "HeightMm": 189.0}))
            .unwrap()],
        };
        let mut layout = Layout::new(Default::default());
        crate::machines::build(
            &mut layout,
            |l| lbm_layout::linux::add_monitor(l, &local),
            &[remote],
            |_| {},
        );
        let here = layout
            .monitors()
            .iter()
            .find(|m| crate::machines::remote_machine(&layout, &m.id).is_none())
            .unwrap()
            .id
            .clone();
        (layout, here)
    }

    #[test]
    fn a_remotes_rulers_are_in_its_own_pixels_and_measure_the_selected_panel() {
        let (layout, here) = layout_with_a_remote();
        let (wanted, errors) = super::wanted(&layout, Some(&here));
        assert!(errors.is_empty(), "{errors:?}");
        let bands = &wanted["0a00"];
        assert_eq!(bands.len(), 4, "one ruler per side of the laptop's panel");
        for band in bands {
            assert_eq!(band.connector, "eDP-1");
            assert_eq!(
                band.screen,
                Pixels {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1200
                }
            );
            assert_eq!(band.scale, 1.25);
            assert!(!band.passive);
            let Content::Ruler { measured_mm, .. } = band.content else {
                panic!("rulers");
            };
            // The selected panel, here, is what the ruler measures.
            let selected = layout
                .depth_projection(layout.monitor(&here).unwrap())
                .unwrap()
                .bounds();
            let expected = if band.side.vertical() {
                selected.height()
            } else {
                selected.width()
            };
            assert!((measured_mm - expected).abs() < 1e-9);
        }
        // Nothing on this machine's own screen, which this window draws itself.
        assert!(bands.iter().all(|b| b.monitor != here));
    }

    #[test]
    fn a_remote_off_its_desktop_gets_no_rulers() {
        let (mut layout, here) = layout_with_a_remote();
        let remote: Vec<_> = layout
            .sources()
            .filter(|s| s.source.host.is_some())
            .cloned()
            .collect();
        for mut source in remote {
            source.source.attached_to_desktop = false;
            layout.add_or_update_source(source);
        }
        let (wanted, errors) = super::wanted(&layout, Some(&here));
        assert!(wanted.is_empty() && errors.is_empty());
    }

    #[test]
    fn only_what_changed_is_sent_and_turning_off_empties_every_remote() {
        let mut rulers = RemoteRulers::default();
        rulers.connected(&[CAPABILITY.into()]);
        let first = rulers.changes(wanted(&[("a", 0.0), ("b", 0.0)]), vec![]);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].1["MachineId"], "a");
        assert_eq!(first[0].1["Bands"][0]["monitor"], "a");
        // Nothing moved: nothing sent.
        assert!(rulers
            .changes(wanted(&[("a", 0.0), ("b", 0.0)]), vec![])
            .is_empty());
        // A drag moves the zero on one remote; the other one leaves the map.
        let next = rulers.changes(wanted(&[("a", 12.0)]), vec![]);
        assert_eq!(
            next.iter()
                .map(|(m, v)| (m.as_str(), v["Bands"].as_array().unwrap().len()))
                .collect::<Vec<_>>(),
            [("b", 0), ("a", 1)]
        );
        let off = rulers.clear();
        assert_eq!(off.len(), 1);
        assert_eq!(off[0].1, json!({"MachineId": "a", "Bands": []}));
        assert!(rulers.clear().is_empty());
    }

    #[test]
    fn an_agent_without_the_capability_is_never_asked() {
        let mut rulers = RemoteRulers::default();
        rulers.connected(&["MultiPc".into()]);
        assert!(rulers.changes(wanted(&[("a", 0.0)]), vec![]).is_empty());
        assert!(rulers.clear().is_empty());
    }

    #[test]
    fn a_refusal_or_a_remotes_error_is_shown_while_its_rulers_are_asked_for() {
        let mut rulers = RemoteRulers::default();
        rulers.connected(&[CAPABILITY.into()]);
        let sent = rulers.changes(wanted(&[("a", 0.0)]), vec![]);
        rulers.sent(7, sent[0].0.clone());
        assert!(rulers.answered(7, &Err("the multi-PC mode is off".into())));
        assert!(!rulers.answered(7, &Ok(Value::Null)), "answered once");
        assert!(rulers.error().unwrap().contains("the multi-PC mode is off"));
        rulers.heard(&json!({"Peers": [
            {"MachineId": "a", "Name": "tower", "OverlayError": "DP-1 is not a screen of this desktop now"}
        ]}));
        assert!(rulers.error().unwrap().contains("tower: DP-1"));
        rulers.clear();
        assert_eq!(rulers.error(), None);
    }
}
