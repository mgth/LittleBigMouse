//! Match an edge report to the exact document requested, never to the latest reply.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use lbm_ipc::probe as wire;
use lbm_layout::model::Layout;
use lbm_ui::probe::{Edge, Overlay, Run, Side};
use serde_json::{json, Value};

pub const DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    layout_id: String,
    revision: u64,
    document: Value,
}

impl Key {
    pub fn of(layout: &Layout, revision: u64) -> Self {
        Self {
            layout_id: layout.id.clone(),
            revision,
            document: json!(crate::settings::document(layout)),
        }
    }
    fn request(&self) -> Value {
        json!({"LayoutId": self.layout_id, "LayoutRevision": self.revision, "Document": self.document})
    }
}

#[derive(Default)]
pub struct Probe {
    wanted: Option<Key>,
    due: Option<Instant>,
    pending: Option<(u64, Key)>,
    report: Option<Report>,
    pub error: Option<String>,
}

impl Probe {
    pub fn want(&mut self, key: Option<Key>, now: Instant) {
        if self.wanted != key {
            self.wanted = key;
            self.report = None;
            self.error = None;
            self.due = Some(now + DEBOUNCE);
        }
    }

    /// At most one request in flight. Old answers are still consumed, but an edit
    /// invalidates their right to be painted as soon as the document changes.
    pub fn ready(&self, now: Instant) -> Option<Value> {
        if self.pending.is_some()
            || self.report.is_some()
            || self.error.is_some()
            || self.due.is_none_or(|due| now < due)
        {
            return None;
        }
        Some(self.wanted.as_ref()?.request())
    }

    pub fn sent(&mut self, id: u64) {
        if let Some(key) = self.wanted.clone() {
            self.pending = Some((id, key));
        }
    }

    /// True means this was our answer, including obsolete answers and refusals.
    /// It must not settle a Save or Start waiting elsewhere in the window.
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> bool {
        if self
            .pending
            .as_ref()
            .is_none_or(|(expected, _)| *expected != id)
        {
            return false;
        }
        let (_, key) = self.pending.take().unwrap();
        if self.wanted.as_ref() != Some(&key) {
            return true;
        }
        let report = result.clone().and_then(|value| {
            let report: wire::Report = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if report.layout_id != key.layout_id || report.layout_revision != key.revision {
                return Err("the report describes another display configuration".into());
            }
            Report::from_wire(report)
        });
        match report {
            Ok(report) => self.report = Some(report),
            Err(why) => self.error = Some(why),
        }
        true
    }

    pub fn overlay(&self, device_id: &str, pixels: lbm_layout::geo::Rect) -> Option<Overlay<'_>> {
        let report = self.report.as_ref()?;
        Some(Overlay {
            pixels,
            edges: report.zones.get(device_id)?,
            names: &report.names,
        })
    }

    pub fn waiting(&self) -> bool {
        self.wanted.is_some() && self.report.is_none() && self.error.is_none()
    }

    pub fn retry(&mut self, now: Instant) {
        self.error = None;
        self.report = None;
        self.due = Some(now);
    }
}

struct Report {
    zones: HashMap<String, Vec<Edge>>,
    names: Vec<(i32, String)>,
}

impl Report {
    fn from_wire(report: wire::Report) -> Result<Self, String> {
        let mut zones = HashMap::new();
        let mut ids = HashSet::new();
        let mut names = Vec::new();
        for zone in report.zones {
            if zone.device_id.is_empty()
                || !ids.insert(zone.id)
                || zones.contains_key(&zone.device_id)
            {
                return Err("ambiguous source in edge report".into());
            }
            names.push((
                zone.id,
                if zone.name.is_empty() {
                    zone.device_id.clone()
                } else {
                    zone.name
                },
            ));
            let edges = zone
                .edges
                .into_iter()
                .map(|edge| Edge {
                    side: match edge.side {
                        wire::Side::Left => Side::Left,
                        wire::Side::Right => Side::Right,
                        wire::Side::Top => Side::Top,
                        wire::Side::Bottom => Side::Bottom,
                    },
                    runs: edge
                        .runs
                        .into_iter()
                        .map(|run| Run {
                            from: run.from,
                            to: run.to,
                            target: run.target,
                        })
                        .collect(),
                })
                .collect();
            zones.insert(zone.device_id, edges);
        }
        Ok(Self { zones, names })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x: i32) -> Key {
        Key {
            layout_id: "screens".into(),
            revision: 1,
            document: json!({"x":x}),
        }
    }
    fn report() -> Value {
        json!({"LayoutId":"screens", "LayoutRevision":1, "Zones":[
            {"Id":7,"DeviceId":"source-B","Name":"Right screen", "Edges":[
                {"Side":"Left","Runs":[{"From":0,"To":99,"Target":2}]}
            ]},
            {"Id":2,"DeviceId":"source-A","Name":"Left screen", "Edges":[]}
        ]})
    }

    #[test]
    fn editing_discards_the_previous_report_and_its_delayed_answer() {
        let mut probe = Probe::default();
        let now = Instant::now();
        probe.want(Some(key(0)), now);
        assert!(probe.ready(now).is_none());
        assert!(probe.ready(now + DEBOUNCE).is_some());
        probe.sent(8);
        probe.want(Some(key(10)), now);
        assert!(
            probe.ready(now + DEBOUNCE).is_none(),
            "one outstanding sweep"
        );
        assert!(probe.answered(8, &Ok(report())));
        assert!(probe.report.is_none());
        assert!(probe.ready(now + DEBOUNCE).is_some());
        probe.sent(9);
        assert!(
            !probe.answered(42, &Ok(report())),
            "another request's reply is not ours"
        );
        probe.answered(9, &Ok(report()));
        assert!(probe.report.is_some());
        probe.want(None, now);
        assert!(probe.report.is_none());
    }

    #[test]
    fn sources_and_target_names_are_identified_independently_of_zone_order() {
        let mut probe = Probe::default();
        probe.want(Some(key(0)), Instant::now());
        probe.sent(1);
        probe.answered(1, &Ok(report()));
        let pixels = lbm_layout::geo::Rect::new(1920.0, 0.0, 1920.0, 100.0);
        let overlay = probe.overlay("source-B", pixels).unwrap();
        let strips = lbm_ui::probe::strips(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0)),
            pixels,
            overlay.edges,
            |id| {
                overlay
                    .names
                    .iter()
                    .find(|(n, _)| *n == id)
                    .unwrap()
                    .1
                    .clone()
            },
        );
        assert_eq!(strips[0].tip, "Left: crosses into Left screen");
        assert_eq!(strips[0].rect.height(), 200.0);
        assert!(probe.overlay("monitor-id-not-source-id", pixels).is_none());
    }

    #[test]
    fn invalid_and_refused_reports_remain_hidden_until_an_explicit_retry() {
        for result in [
            Err("unsupported".into()),
            Ok(json!({})),
            Ok(json!({
                "LayoutId":"other", "LayoutRevision":1, "Zones":[]
            })),
        ] {
            let mut probe = Probe::default();
            let now = Instant::now();
            probe.want(Some(key(0)), now);
            probe.sent(1);
            probe.answered(1, &result);
            assert!(probe.error.is_some());
            assert!(probe.ready(now + DEBOUNCE).is_none());
            probe.retry(now);
            assert!(probe.ready(now).is_some());
        }
    }
}
