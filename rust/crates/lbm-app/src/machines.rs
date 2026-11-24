//! The multi-PC mode in the window (docs/multi-pc.md, lot 7): what the agent says about
//! the other machines, the remote monitors it puts on the map, and the questions the rest
//! of the window asks about a monitor ("is it on this machine?").
//!
//! **The window builds the layout the agent builds.** The agent refuses a document whose
//! `LayoutId` is not the one it holds, and the id is every monitor's, remote ones
//! included. So the remotes are merged here exactly where the agent merges them
//! ([`build`]): this machine's monitors, then each connected remote in machine-id order,
//! then the id, then the store, then the placement — the order of `lbm_layout::linux::
//! populate`, with the merge inserted before the id (`lbm_layout::remote::merge_remote`).

use std::borrow::Cow;
use std::collections::HashMap;

use lbm_ipc::peer::PeerDisplay;
use lbm_layout::model::Layout;
use lbm_ui::machines::{Did, Peer, Role, State};
use serde_json::{json, Value};

/// The agent's capability for everything here.
pub const CAPABILITY: &str = "MultiPc";

/// A machine whose screens are on this map: paired, connected, and the remote end of
/// its link with this one.
#[derive(Clone, Debug, PartialEq)]
pub struct Remote {
    pub machine_id: String,
    pub name: String,
    pub displays: Vec<PeerDisplay>,
}

/// The agent's `Peers` state, read: the page's view of it, and the remotes to merge.
///
/// Read by hand, field by field, like every other answer here: a state from a newer
/// agent with fields this window does not know still reads.
pub fn read(value: &Value) -> Option<(State, Vec<Remote>)> {
    let object = value.as_object()?;
    let text = |v: &Value, key: &str| v[key].as_str().unwrap_or_default().to_owned();
    let mut peers = Vec::new();
    let mut remotes = Vec::new();
    // The agent drives one remote at a time and merges its screens only; an agent older
    // than that rule says nothing and merged every linked remote.
    let selected = object
        .get("SelectedRemote")
        .map(|selected| selected.as_str().unwrap_or_default());
    for entry in value["Peers"].as_array().into_iter().flatten() {
        let machine_id = text(entry, "MachineId");
        if machine_id.is_empty() {
            continue;
        }
        let role = match entry["Role"].as_str() {
            Some("Master") => Role::Master,
            _ => Role::Remote,
        };
        let pending = entry["State"].as_str() == Some("Pending");
        let connected = entry["Connected"].as_bool().unwrap_or(false);
        // A display that does not read is left out rather than failing the whole state:
        // the others are still worth showing.
        let displays: Vec<PeerDisplay> = entry["Displays"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| serde_json::from_value(d.clone()).ok())
            .collect();
        let merged = selected.is_none_or(|selected| selected == machine_id);
        if role == Role::Remote && connected && !pending && merged {
            remotes.push(Remote {
                machine_id: machine_id.clone(),
                name: text(entry, "Name"),
                displays: displays.clone(),
            });
        }
        peers.push(Peer {
            machine_id,
            name: text(entry, "Name"),
            address: text(entry, "Address"),
            connected,
            role,
            pending,
            displays: displays.len(),
        });
    }
    // Byte order of the fingerprints, as `Layout::hosts` sorts them: the order the agent
    // merges in, so that a panel two remotes share is claimed by the same one on both
    // sides.
    remotes.sort_by(|a, b| a.machine_id.cmp(&b.machine_id));
    let state = State {
        enabled: object
            .get("Enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        port: object
            .get("Port")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(0),
        name: text(value, "Name"),
        machine_id: value["MachineId"].as_str().map(str::to_owned),
        listen_address: value["ListenAddress"].as_str().map(str::to_owned),
        error: value["Error"].as_str().map(str::to_owned),
        peers,
    };
    Some((state, remotes))
}

/// Merges `remotes` into a layout that holds this machine's monitors and nothing else
/// yet (`lbm_display::peer::merge_peer`, in the order given).
pub fn merge(layout: &mut Layout, remotes: &[Remote]) {
    for remote in remotes {
        lbm_display::peer::merge_peer(layout, &remote.machine_id, &remote.name, &remote.displays);
    }
}

/// The layout with remotes, in the agent's order: `add_local` maps this machine's
/// monitors; the remotes are merged; the id is computed; `load` applies the store; then
/// the monitors the store did not place are placed (remote ones to the right of the
/// rest) and everything is anchored on the primary.
///
/// What `lbm_layout::linux::populate` and `windows::populate` do, with the merge where
/// `lbm_layout::remote::merge_remote` says it goes. Composed here from their public
/// pieces because the two `populate` take no remotes.
pub fn build(
    layout: &mut Layout,
    add_local: impl FnOnce(&mut Layout),
    remotes: &[Remote],
    load: impl FnOnce(&mut Layout),
) {
    add_local(layout);
    merge(layout, remotes);
    layout.id = layout.compute_id();
    load(layout);
    layout.set_locations_from_system_configuration(false);
    layout.anchor_on_primary();
}

/// The machine a source is on, by name: `None` for this machine. The merge writes the
/// remote's name into `interface_name` ("where the map says which machine a source is
/// on").
fn machine_of(layout: &Layout, source: &str) -> Option<String> {
    let source = &layout.source(source)?.source;
    source.host.as_ref()?;
    Some(
        source
            .interface_name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "another machine".to_owned()),
    )
}

/// The machine a monitor is showing, when it is not this one.
pub fn remote_machine(layout: &Layout, monitor: &str) -> Option<String> {
    let monitor = layout.monitor(monitor)?;
    machine_of(layout, monitor.active_source.as_deref()?)
}

/// Whether a monitor has sources on more than one machine: a panel the user can switch.
pub fn is_shared(layout: &Layout, monitor: &str) -> bool {
    let Some(monitor) = layout.monitor(monitor) else {
        return false;
    };
    let mut hosts: Vec<Option<&str>> = monitor
        .sources
        .iter()
        .filter_map(|id| layout.source(id))
        .map(|s| s.source.host.as_deref())
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts.len() > 1
}

/// What the map's badge says about one monitor, owned: `None` for a monitor that only
/// ever shows this machine, which is drawn as it always was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub machine: String,
    pub remote: bool,
    pub shared: bool,
}

impl Tag {
    pub fn host(&self) -> lbm_ui::frame::Host<'_> {
        lbm_ui::frame::Host {
            machine: &self.machine,
            remote: self.remote,
            shared: self.shared,
        }
    }
}

/// The badge for one monitor. `here` is this machine's name, for a shared panel showing
/// this machine.
pub fn tag(layout: &Layout, monitor: &str, here: &str) -> Option<Tag> {
    let remote = remote_machine(layout, monitor);
    let shared = is_shared(layout, monitor);
    if remote.is_none() && !shared {
        return None;
    }
    Some(Tag {
        remote: remote.is_some(),
        machine: remote.unwrap_or_else(|| this_machine(here)),
        shared,
    })
}

fn this_machine(here: &str) -> String {
    if here.is_empty() {
        "this machine".to_owned()
    } else {
        here.to_owned()
    }
}

/// The sources of a shared panel, for its switch.
pub fn sources(layout: &Layout, monitor: &str, here: &str) -> Vec<lbm_ui::machines::Source> {
    let Some(monitor) = layout.monitor(monitor) else {
        return Vec::new();
    };
    monitor
        .sources
        .iter()
        .filter_map(|id| layout.source(id))
        .map(|s| lbm_ui::machines::Source {
            id: s.source.id.clone(),
            machine: machine_of(layout, &s.source.id).unwrap_or_else(|| this_machine(here)),
            attached: s.source.attached_to_desktop,
        })
        .collect()
}

/// The layout as the local-only helpers must see it: every remote source detached from
/// the desktop, so the screen overlays, the desktop thumbnails and anything else that
/// walks the attached sources reach this machine's screens only. A remote source's
/// pixels are another desktop's, and its connector names an output over there — the
/// same name as one of this machine's, as often as not.
///
/// Borrowed as it is when there is no remote source, which is every layout outside the
/// multi-PC mode.
pub fn local_view(layout: &Layout) -> Cow<'_, Layout> {
    if layout.sources().all(|s| s.source.is_local()) {
        return Cow::Borrowed(layout);
    }
    let mut local = layout.clone();
    let remote: Vec<_> = layout
        .sources()
        .filter(|s| !s.source.is_local())
        .cloned()
        .collect();
    for mut source in remote {
        source.source.attached_to_desktop = false;
        local.add_or_update_source(source);
    }
    Cow::Owned(local)
}

/// The requests in flight, and what the window knows of the mode.
#[derive(Default)]
pub struct Controller {
    /// The agent announced `MultiPc`. Without it nothing here is shown or asked.
    pub offered: bool,
    /// What the agent last said; `None` until it has.
    pub state: Option<State>,
    /// The machines whose screens are on the map, as last said.
    pub remotes: Vec<Remote>,
    pub panel: lbm_ui::machines::Panel,
    /// The last request the agent refused.
    pub error: Option<String>,
    asked: HashMap<u64, &'static str>,
}

impl Controller {
    /// A new connection: what the agent can do, and nothing it said before.
    ///
    /// The remotes are kept for an agent that has the mode — its answer to `Peers` says
    /// soon enough whether they are still there, and the map does not flicker in the
    /// meantime — and dropped for one without it, which builds its layout without them.
    /// The reconnection reads the displays again either way.
    pub fn connected(&mut self, capabilities: &[String]) {
        let offered = capabilities.iter().any(|c| c == CAPABILITY);
        *self = Controller {
            offered,
            panel: std::mem::take(&mut self.panel),
            remotes: if offered {
                std::mem::take(&mut self.remotes)
            } else {
                Vec::new()
            },
            ..Default::default()
        };
    }

    /// The connection is gone. The remotes are kept: the map they are on is still the
    /// layout in hand, and the next agent will say what has changed.
    pub fn disconnected(&mut self) {
        self.offered = false;
        self.state = None;
        self.error = None;
        self.asked.clear();
    }

    /// Whether a request is out: the page greys its controls until it is answered.
    pub fn busy(&self) -> bool {
        !self.asked.is_empty()
    }

    /// The request for what the user did.
    pub fn request(&self, did: &Did) -> (&'static str, Value) {
        let enabled = self.state.as_ref().is_some_and(|s| s.enabled);
        match did {
            Did::Enable(on) => ("SetMultiPc", json!({ "Enabled": on })),
            Did::Rename(name) => ("SetMultiPc", json!({ "Enabled": enabled, "Name": name })),
            Did::Connect(address) => ("PeerConnect", json!({ "Address": address })),
            Did::Accept(id) => ("PeerAccept", json!({ "MachineId": id })),
            Did::Refuse(id) => ("PeerRefuse", json!({ "MachineId": id })),
            Did::Forget(id) => ("PeerForget", json!({ "MachineId": id })),
        }
    }

    /// A request went out under `id`.
    pub fn sent(&mut self, id: u64, method: &'static str) {
        self.asked.insert(id, method);
        self.error = None;
    }

    /// An answer, if it is one of ours. `Some(changed)` says whether the remotes on the
    /// map changed with it.
    pub fn answered(&mut self, id: u64, result: &Result<Value, String>) -> Option<bool> {
        let method = self.asked.remove(&id)?;
        match result {
            // These two answer with the whole state.
            Ok(state) if matches!(method, "Peers" | "SetMultiPc") => Some(self.heard(state)),
            // The rest answer with an entry or null: the `Peers` event is the news. A
            // machine reached is the address's job done; one not reached keeps it typed,
            // to be corrected.
            Ok(_) => {
                if method == "PeerConnect" {
                    self.panel.address.clear();
                }
                Some(false)
            }
            Err(why) => {
                self.error = Some(format!("{method} refused: {why}"));
                Some(false)
            }
        }
    }

    /// The state, from an answer or the `Peers` event. Says whether the remotes on the
    /// map changed — the one thing that makes the window read its displays again.
    pub fn heard(&mut self, value: &Value) -> bool {
        let Some((state, remotes)) = read(value) else {
            return false;
        };
        self.state = Some(state);
        if remotes == self.remotes {
            return false;
        }
        self.remotes = remotes;
        true
    }

    /// This machine's name, for a shared panel showing it.
    pub fn here(&self) -> &str {
        self.state.as_ref().map_or("", |s| s.name.as_str())
    }
}

/// A switch of a shared panel in flight: `SetActiveSource`, and what it changes once the
/// agent has taken it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switch {
    pub id: u64,
    pub layout: String,
    pub monitor: String,
    pub source: String,
}

impl Switch {
    /// The request: `SetActiveSource { LayoutId, Monitor, Source }`.
    pub fn request(&self) -> Value {
        json!({
            "LayoutId": self.layout,
            "Monitor": self.monitor,
            "Source": self.source,
        })
    }
}

/// What the window does with the answer to a switch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Switched {
    /// The agent switched the panel: the window's copy follows, and so does the
    /// reference, since the agent holds it now.
    Taken,
    /// The agent does not know the request (one older than the source switch): the
    /// window makes the switch an edit of its own, which Save carries in the document's
    /// `ActiveSource`.
    Unknown,
    /// Refused, for the reason given.
    Refused(String),
}

/// Reads the answer to a switch. An agent that predates `SetActiveSource` answers
/// "bad request" — it cannot parse the method — and that is told apart from a refusal.
pub fn switched(result: &Result<Value, String>) -> Switched {
    match result {
        Ok(_) => Switched::Taken,
        Err(why) if why.starts_with("bad request") => Switched::Unknown,
        Err(why) => Switched::Refused(why.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(peers: Value) -> Value {
        json!({
            "Enabled": true, "Port": 24817, "Name": "desk",
            "MachineId": "a1b2c3d4", "ListenAddress": "0.0.0.0:24817", "Error": null,
            "Peers": peers,
        })
    }

    fn display(id: &str) -> Value {
        json!({"Id": id, "X": 0, "Y": 0, "Width": 1920, "Height": 1080, "Attached": true})
    }

    #[test]
    fn only_connected_paired_remotes_are_merged_and_in_fingerprint_order() {
        let (read_state, remotes) = read(&state(json!([
            {"MachineId": "ff", "Name": "late", "Connected": true, "Role": "Remote",
             "State": "Paired", "Displays": [display("eDP-1")]},
            {"MachineId": "0a", "Name": "early", "Connected": true, "Role": "Remote",
             "State": "Paired", "Displays": [display("HDMI-1"), display("DP-1")]},
            {"MachineId": "11", "Name": "asleep", "Connected": false, "Role": "Remote",
             "State": "Paired", "Displays": []},
            {"MachineId": "22", "Name": "stranger", "Connected": true, "Role": "Remote",
             "State": "Pending", "Displays": [display("X")]},
            {"MachineId": "33", "Name": "boss", "Connected": true, "Role": "Master",
             "State": "Paired", "Displays": []},
        ])))
        .unwrap();
        let ids: Vec<&str> = remotes.iter().map(|r| r.machine_id.as_str()).collect();
        assert_eq!(ids, ["0a", "ff"]);
        assert_eq!(remotes[0].displays.len(), 2);
        assert_eq!(read_state.peers.len(), 5, "the page shows every machine");
        assert!(read_state.peers[3].pending);
        assert_eq!(read_state.peers[4].role, Role::Master);
        assert_eq!(read_state.name, "desk");
        assert_eq!(read_state.port, 24817);
    }

    #[test]
    fn only_the_remote_the_agent_selected_is_merged() {
        let peers = json!([
            {"MachineId": "ff", "Name": "late", "Connected": true, "Role": "Remote",
             "State": "Paired", "Displays": [display("eDP-1")]},
            {"MachineId": "0a", "Name": "early", "Connected": true, "Role": "Remote",
             "State": "Paired", "Displays": [display("HDMI-1")]},
        ]);
        let mut selected = state(peers.clone());
        selected["SelectedRemote"] = json!("ff");
        let (read_state, remotes) = read(&selected).unwrap();
        let ids: Vec<&str> = remotes.iter().map(|r| r.machine_id.as_str()).collect();
        assert_eq!(ids, ["ff"]);
        assert_eq!(read_state.peers.len(), 2, "the page still shows both");

        let mut none = state(peers);
        none["SelectedRemote"] = Value::Null;
        assert!(read(&none).unwrap().1.is_empty());
    }

    #[test]
    fn a_new_state_says_whether_the_map_must_be_rebuilt() {
        let mut machines = Controller::default();
        machines.connected(&["IntrinsicSize".into(), "MultiPc".into()]);
        assert!(machines.offered);
        let remote = state(
            json!([{"MachineId": "0a", "Name": "laptop", "Connected": true,
            "Role": "Remote", "State": "Paired", "Displays": [display("eDP-1")]}]),
        );
        assert!(machines.heard(&remote), "a remote appeared");
        assert!(!machines.heard(&remote), "nothing changed");
        let renamed_here = {
            let mut s = remote.clone();
            s["Name"] = json!("office");
            s
        };
        assert!(
            !machines.heard(&renamed_here),
            "renaming this machine moves no screen"
        );
        assert!(machines.heard(&state(json!([]))), "the remote left");
    }

    #[test]
    fn an_agent_without_the_capability_offers_nothing_and_merges_nothing() {
        let mut machines = Controller::default();
        machines.connected(&["MultiPc".into()]);
        machines.heard(&state(json!([{"MachineId": "0a", "Name": "laptop",
            "Connected": true, "Role": "Remote", "State": "Paired",
            "Displays": [display("eDP-1")]}])));
        machines.connected(&["MultiPc".into()]);
        assert_eq!(
            machines.remotes.len(),
            1,
            "kept until the agent says otherwise"
        );
        machines.connected(&["IntrinsicSize".into()]);
        assert!(!machines.offered);
        assert!(
            machines.remotes.is_empty(),
            "an agent without the mode builds its layout without them"
        );
    }

    #[test]
    fn requests_carry_what_the_api_documents() {
        let mut machines = Controller::default();
        machines.heard(&state(json!([])));
        assert_eq!(
            machines.request(&Did::Rename("office".into())),
            ("SetMultiPc", json!({"Enabled": true, "Name": "office"}))
        );
        assert_eq!(
            machines.request(&Did::Accept("0a".into())),
            ("PeerAccept", json!({"MachineId": "0a"}))
        );
        assert_eq!(
            machines.request(&Did::Connect("10.0.0.2".into())),
            ("PeerConnect", json!({"Address": "10.0.0.2"}))
        );
    }

    #[test]
    fn a_refused_request_is_kept_to_be_shown_and_ends_the_wait() {
        let mut machines = Controller::default();
        machines.sent(4, "PeerConnect");
        assert!(machines.busy());
        assert_eq!(
            machines.answered(4, &Err("nobody there".into())),
            Some(false)
        );
        assert!(!machines.busy());
        assert!(machines.error.as_deref().unwrap().contains("nobody there"));
        assert_eq!(machines.answered(5, &Ok(Value::Null)), None, "not ours");
    }

    #[test]
    fn an_agent_older_than_the_switch_is_told_apart_from_a_refusal() {
        assert_eq!(switched(&Ok(Value::Null)), Switched::Taken);
        assert_eq!(
            switched(&Err("bad request: unknown variant `SetActiveSource`".into())),
            Switched::Unknown
        );
        assert_eq!(
            switched(&Err("no such source".into())),
            Switched::Refused("no such source".into())
        );
    }

    fn remote(machine_id: &str, name: &str) -> Remote {
        Remote {
            machine_id: machine_id.into(),
            name: name.into(),
            displays: vec![serde_json::from_value(json!({"Id": "eDP-1", "X": 0, "Y": 0,
                "Width": 1920, "Height": 1080, "Attached": true,
                "WidthMm": 302.0, "HeightMm": 188.0}))
            .unwrap()],
        }
    }

    /// The id is the agent's: computed over this machine's monitors and the remotes'
    /// together, before the store is read — so the store is asked for the profile of
    /// this set of monitors, remote ones included.
    #[test]
    fn the_layout_is_built_in_the_agents_order() {
        let local = lbm_layout::linux::LinuxMonitor::fallback();
        let remotes = [remote("ff00", "late"), remote("0a00", "early")];
        let mut sorted = remotes.to_vec();
        sorted.sort_by(|a, b| a.machine_id.cmp(&b.machine_id));

        let mut expected = Layout::new(Default::default());
        lbm_layout::linux::add_monitor(&mut expected, &local);
        merge(&mut expected, &sorted);
        let expected_id = expected.compute_id();

        let mut layout = Layout::new(Default::default());
        let mut id_at_load = None;
        build(
            &mut layout,
            |l| lbm_layout::linux::add_monitor(l, &local),
            &sorted,
            |l| id_at_load = Some(l.id.clone()),
        );
        assert_eq!(id_at_load.as_deref(), Some(expected_id.as_str()));
        assert_eq!(layout.id, expected_id);
        assert_eq!(layout.monitors().len(), 3);
    }

    /// A remote monitor the store never placed goes right of where the store put this
    /// machine's: the placement runs after the load, not before.
    #[test]
    fn a_remote_never_placed_goes_right_of_the_screens_as_stored() {
        let local = lbm_layout::linux::LinuxMonitor::fallback();
        let mut layout = Layout::new(Default::default());
        build(
            &mut layout,
            |l| lbm_layout::linux::add_monitor(l, &local),
            &[remote("0a00", "laptop")],
            |l| {
                // What a store does to a monitor it knows: a place, and placed.
                let id = l.monitors()[0].id.clone();
                l.set_location(&id, lbm_layout::geo::Point::new(800.0, 0.0));
                l.set_placed(&id, true);
            },
        );
        let right_of_local = layout
            .monitors()
            .iter()
            .filter(|m| remote_machine(&layout, &m.id).is_none())
            .filter_map(|m| layout.depth_projection(m))
            .map(|p| p.outside_bounds().right())
            .fold(f64::MIN, f64::max);
        let panel = layout
            .monitors()
            .iter()
            .find(|m| remote_machine(&layout, &m.id).is_some())
            .and_then(|m| layout.depth_projection(m))
            .unwrap()
            .outside_bounds();
        assert!(
            panel.left() >= right_of_local - 1e-6,
            "the remote screen overlaps the stored ones: {panel:?}, local right {right_of_local}"
        );
    }

    #[test]
    fn the_local_view_detaches_remote_sources_only() {
        let mut layout = Layout::new(Default::default());
        build(
            &mut layout,
            |l| lbm_layout::linux::add_monitor(l, &lbm_layout::linux::LinuxMonitor::fallback()),
            &[remote("0a00", "laptop")],
            |_| {},
        );
        let local = local_view(&layout);
        assert!(matches!(local, Cow::Owned(_)));
        for source in local.sources() {
            assert_eq!(
                source.source.attached_to_desktop,
                source.source.is_local(),
                "{}",
                source.source.id
            );
        }
        // The layout itself is left alone: the remote screen is still on the map.
        assert!(layout.sources().all(|s| s.source.attached_to_desktop));
    }
}
