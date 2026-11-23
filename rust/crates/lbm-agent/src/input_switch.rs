//! Input switching over VCP 0x60 (docs/multi-pc.md, "Input switching", lot 8): a panel
//! wired to two machines on two inputs, with no KVM, is switched by writing the input
//! value of the machine chosen to show it.
//!
//! Two halves, and only one of them ever writes:
//!
//! - **learning**, on display events only: a machine reads 0x60 of the panels it shows,
//!   and the reading is taken as the value of the source shown — never of another one,
//!   since a monitor that keeps both inputs alive is attached to both machines whatever
//!   it shows. A remote publishes what it read (`PeerDisplay.InputSource`); its master
//!   adopts it under the same rule.
//! - **switching**, on the user's explicit choice only (`SetActiveSource`): the chosen
//!   source's value is written if the monitor shows another input. The master tries its
//!   own DDC link first; when it has none, or the monitor does not answer on it (many
//!   stop answering on the cable they are not showing), it asks the remote that has
//!   the panel with `SelectInput`.
//!
//! A display event never writes, so a panel that detaches from one machine and attaches
//! to the other after a switch cannot set off another switch. And a source with no
//! known value switches nothing: with a hardware KVM on a single input, choosing a
//! machine changes only where the pointer goes.

use lbm_layout::model::Layout;
use lbm_layout::remote::remote_source_id;

use crate::peers::RemoteScreens;
use crate::vcp::{self, Ddc, Target};

/// What choosing a source asks of the monitor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputSwitch {
    pub monitor: String,
    pub source: String,
    /// The VCP 0x60 value that shows `source`.
    pub value: u16,
    /// This machine's DDC link to the panel; else why it has none.
    pub local: Result<Target, String>,
    /// The remote to ask when this machine cannot switch the panel itself.
    pub remote: Option<RemoteAsk>,
}

/// A remote that has the panel, and its own id of it (the `Id` of its `PeerDisplay`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteAsk {
    pub machine_id: String,
    pub display: String,
}

/// A panel whose input this machine is to read, and the source a reading is the value
/// of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputRead {
    pub monitor: String,
    pub source: String,
    pub target: Target,
}

/// How a switch went on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The monitor already showed that input: nothing written.
    AlreadyShown,
    Written,
    /// This machine could not; that remote is to be asked. Why this machine could not.
    Ask(RemoteAsk, String),
    /// Nobody can: why.
    Failed(String),
}

/// What choosing `source` on `monitor` asks of DDC, `previous` being the source shown
/// until now. `None` when the source has no known input value: only the active source
/// changes then, which is all a hardware KVM needs.
pub fn plan(
    layout: &Layout,
    remotes: &[RemoteScreens],
    monitor: &str,
    source: &str,
    previous: Option<&str>,
) -> Option<InputSwitch> {
    let value = layout.source(source)?.source.input_source?;
    let panel = layout.monitor(monitor)?;
    // The remote most likely to reach the panel over DDC is the one it shows now, then
    // the one it is to show, then any other machine wired to it.
    let host_of = |id: &str| layout.source(id).and_then(|s| s.source.host.clone());
    let mut hosts: Vec<String> = previous
        .and_then(host_of)
        .into_iter()
        .chain(host_of(source))
        .chain(panel.sources.iter().filter_map(|s| host_of(s)))
        .collect();
    let mut seen = Vec::new();
    hosts.retain(|h| {
        let new = !seen.contains(h);
        seen.push(h.clone());
        new
    });
    let remote = hosts.into_iter().find_map(|host| {
        let source_there = panel
            .sources
            .iter()
            .find(|s| host_of(s).as_deref() == Some(host.as_str()))?;
        let screens = remotes.iter().find(|r| r.machine_id == host)?;
        let display = screens
            .displays
            .iter()
            .find(|d| remote_source_id(&host, &d.id) == *source_there)?;
        Some(RemoteAsk {
            machine_id: host.clone(),
            display: display.id.clone(),
        })
    });
    Some(InputSwitch {
        monitor: monitor.to_owned(),
        source: source.to_owned(),
        value,
        local: vcp::target(layout, monitor),
        remote,
    })
}

/// Whether `source` is the one its panel shows, and is on its machine's desktop: only
/// then is a reading of the panel's input that source's value.
fn shown(layout: &Layout, monitor: &str, source: &str) -> bool {
    layout
        .monitor(monitor)
        .is_some_and(|m| m.active_source.as_deref() == Some(source))
        && layout
            .source(source)
            .is_some_and(|s| s.source.attached_to_desktop)
}

/// Whether a panel is known to more than one machine.
fn shared(layout: &Layout, monitor: &lbm_layout::model::Monitor) -> bool {
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

/// The panels to read after a rebuild: this machine's, on its desktop, and reachable
/// over DDC. A panel two machines know is read only while this machine's source is the
/// one shown; any other panel only when `publishing`, that is when a master is to be
/// told what selects this machine — the panels of a machine alone are not read at all.
pub fn reads(layout: &Layout, publishing: bool) -> Vec<InputRead> {
    if layout.is_virtual() {
        return Vec::new();
    }
    layout
        .monitors()
        .iter()
        .filter_map(|monitor| {
            let local = monitor
                .sources
                .iter()
                .filter_map(|id| layout.source(id))
                .find(|s| s.source.is_local() && s.source.attached_to_desktop)?;
            let wanted = if shared(layout, monitor) {
                monitor.active_source.as_deref() == Some(local.source.id.as_str())
            } else {
                publishing
            };
            if !wanted {
                return None;
            }
            let target = vcp::target(layout, &monitor.id).ok()?;
            Some(InputRead {
                monitor: monitor.id.clone(),
                source: local.source.id.clone(),
                target,
            })
        })
        .collect()
}

/// Remembers what was read, for the sources still shown as they were when read.
/// Whether a value changed.
pub fn learn(layout: &mut Layout, readings: &[(InputRead, u16)]) -> bool {
    let mut changed = false;
    for (read, value) in readings {
        let still_shown = layout.monitor(&read.monitor).is_some_and(|m| {
            !shared(layout, m) || m.active_source.as_deref() == Some(read.source.as_str())
        }) && layout
            .source(&read.source)
            .is_some_and(|s| s.source.attached_to_desktop);
        let known = layout
            .source(&read.source)
            .and_then(|s| s.source.input_source);
        if still_shown && known != Some(*value) {
            layout.set_source_input(&read.source, Some(*value));
            changed = true;
        }
    }
    changed
}

/// The remotes' screens as the layout merges them: without the input values they read,
/// which [`adopt_published`] gives only to the sources shown.
pub fn without_inputs(remotes: &[RemoteScreens]) -> Vec<RemoteScreens> {
    remotes
        .iter()
        .map(|remote| RemoteScreens {
            displays: remote
                .displays
                .iter()
                .cloned()
                .map(|mut d| {
                    d.input_source = None;
                    d
                })
                .collect(),
            ..remote.clone()
        })
        .collect()
}

/// What a remote read of its panels, adopted for its sources that their panels show. A
/// remote cannot tell whether the panel it reads shows it or the other machine, when
/// the monitor keeps both inputs alive; the layout can. Whether a value changed.
pub fn adopt_published(layout: &mut Layout, remotes: &[RemoteScreens]) -> bool {
    let mut adopted = Vec::new();
    for remote in remotes {
        for display in &remote.displays {
            let Some(value) = display.input_source else {
                continue;
            };
            let source = remote_source_id(&remote.machine_id, &display.id);
            let Some(monitor) = layout.source(&source).map(|s| s.monitor.clone()) else {
                continue;
            };
            let known = layout.source(&source).and_then(|s| s.source.input_source);
            if shown(layout, &monitor, &source) && known != Some(value) {
                adopted.push((source, value));
            }
        }
    }
    for (source, value) in &adopted {
        layout.set_source_input(source, Some(*value));
    }
    !adopted.is_empty()
}

/// Switches the monitor `target` to `value` over this machine's link, unless it shows
/// that input already. Whether it wrote.
pub async fn switch_locally(ddc: &dyn Ddc, target: Target, value: u16) -> Result<bool, String> {
    // Read first: the monitor may show that input already (two sources on one input,
    // which is what a KVM is, or a switch made from the monitor's own menu), and a
    // write that changes nothing can still blank the panel while it resyncs.
    if ddc.read(target.clone(), vcp::INPUT).await? == value {
        return Ok(false);
    }
    ddc.write(target, vcp::INPUT, value).await.map(|()| true)
}

/// Carries out a switch on this machine's side.
pub async fn execute(ddc: &dyn Ddc, switch: &InputSwitch) -> Outcome {
    let why = match &switch.local {
        Ok(target) => match switch_locally(ddc, target.clone(), switch.value).await {
            Ok(false) => return Outcome::AlreadyShown,
            Ok(true) => return Outcome::Written,
            Err(error) => error,
        },
        Err(error) => error.clone(),
    };
    match &switch.remote {
        Some(ask) => Outcome::Ask(ask.clone(), why),
        None => Outcome::Failed(why),
    }
}

#[cfg(test)]
mod tests {
    use lbm_ipc::peer::PeerDisplay;
    use lbm_layout::linux::{add_monitor, LinuxEdid, LinuxMonitor};
    use lbm_layout::model::LayoutOptions;
    use lbm_layout::remote::{merge_remote, RemoteMonitor};

    use super::*;
    use crate::vcp::{DdcCall, FakeDdc};

    const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
    const DP: u16 = 0x0f;
    const HDMI: u16 = 0x11;

    /// This machine with the Dell on DP-1, and the laptop with it on HDMI-1, both
    /// attached as given: the layout and what the laptop reported.
    struct Desk {
        layout: Layout,
        remotes: Vec<RemoteScreens>,
        dell: String,
        here: String,
        there: String,
    }

    fn desk(vcp_control: bool, here_attached: bool, there_attached: bool) -> Desk {
        let mut layout = Layout::new(LayoutOptions {
            vcp_control,
            ..Default::default()
        });
        add_monitor(
            &mut layout,
            &LinuxMonitor {
                connector_name: "DP-1".into(),
                enabled: here_attached,
                edid: Some(LinuxEdid {
                    manufacturer_code: Some("DEL".into()),
                    product_code: Some("4065".into()),
                    serial_number: Some("ABC123".into()),
                    ..Default::default()
                }),
                ..LinuxMonitor::fallback()
            },
        );
        let dell = layout.monitors()[0].id.clone();
        let here = layout.monitors()[0].sources[0].clone();
        let panel_key = layout.monitors()[0].panel_key.clone();
        assert!(panel_key.is_some());
        merge_remote(
            &mut layout,
            LAPTOP,
            "laptop",
            &[RemoteMonitor {
                id: "HDMI-1".into(),
                connector: "HDMI-1".into(),
                panel_key,
                width: 1920,
                height: 1080,
                scale: 1.0,
                attached: there_attached,
                ..RemoteMonitor::default()
            }],
        );
        let there = remote_source_id(LAPTOP, "HDMI-1");
        assert!(layout.monitor(&dell).unwrap().sources.contains(&there));
        let remotes = vec![RemoteScreens {
            machine_id: LAPTOP.into(),
            name: "laptop".into(),
            displays: vec![peer_display("HDMI-1", there_attached, None)],
        }];
        Desk {
            layout,
            remotes,
            dell,
            here,
            there,
        }
    }

    fn peer_display(id: &str, attached: bool, input_source: Option<u16>) -> PeerDisplay {
        PeerDisplay {
            id: id.into(),
            connector: id.into(),
            name: id.into(),
            edid: None,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
            effective_dpi: 96.0,
            orientation: 0,
            frequency: 60.0,
            primary: false,
            attached,
            width_mm: 527.0,
            height_mm: 296.0,
            input_source,
        }
    }

    fn dp1() -> Target {
        Target {
            connector: "DP-1".into(),
            serial: Some("ABC123".into()),
            model: Some("DEL4065".into()),
        }
    }

    fn write(value: u16) -> DdcCall {
        DdcCall::Write {
            connector: "DP-1".into(),
            code: vcp::INPUT,
            value,
        }
    }

    #[test]
    fn a_known_input_is_switched_from_here_first_and_else_by_the_machine_with_the_panel() {
        let mut d = desk(true, true, true);
        d.layout.set_source_input(&d.there, Some(HDMI));
        let switch = plan(&d.layout, &d.remotes, &d.dell, &d.there, Some(&d.here)).unwrap();
        assert_eq!(switch.value, HDMI);
        assert_eq!(
            switch.local.as_ref().map(|t| t.connector.as_str()),
            Ok("DP-1")
        );
        assert_eq!(
            switch.remote,
            Some(RemoteAsk {
                machine_id: LAPTOP.into(),
                display: "HDMI-1".into(),
            })
        );

        // Without VCP control enabled here, only the laptop can.
        let mut d = desk(false, true, true);
        d.layout.set_source_input(&d.there, Some(HDMI));
        let switch = plan(&d.layout, &d.remotes, &d.dell, &d.there, Some(&d.here)).unwrap();
        assert!(switch.local.is_err());
        assert!(switch.remote.is_some());
    }

    #[test]
    fn a_source_whose_input_is_not_known_switches_nothing() {
        // A hardware KVM on one input: nothing is ever learnt, nothing is written.
        let d = desk(true, true, true);
        assert_eq!(
            plan(&d.layout, &d.remotes, &d.dell, &d.there, Some(&d.here)),
            None
        );
    }

    #[tokio::test]
    async fn the_input_is_written_only_when_the_monitor_shows_another() {
        let mut d = desk(true, true, true);
        d.layout.set_source_input(&d.there, Some(HDMI));
        let switch = plan(&d.layout, &d.remotes, &d.dell, &d.there, Some(&d.here)).unwrap();

        let ddc = FakeDdc::default();
        ddc.set("DP-1", vcp::INPUT, DP);
        assert_eq!(execute(&ddc, &switch).await, Outcome::Written);
        assert_eq!(ddc.writes(), [write(HDMI)]);
        // Shown now: a second choice of the same machine writes nothing.
        assert_eq!(execute(&ddc, &switch).await, Outcome::AlreadyShown);
        assert_eq!(ddc.writes(), [write(HDMI)]);
    }

    #[tokio::test]
    async fn a_monitor_silent_on_this_cable_is_switched_by_the_remote_and_else_by_nobody() {
        let mut d = desk(true, true, true);
        d.layout.set_source_input(&d.here, Some(DP));
        let switch = plan(&d.layout, &d.remotes, &d.dell, &d.here, Some(&d.there)).unwrap();
        let ddc = FakeDdc::default();
        ddc.silence("DP-1");
        let Outcome::Ask(ask, _) = execute(&ddc, &switch).await else {
            panic!("the laptop is asked");
        };
        assert_eq!(
            (ask.machine_id.as_str(), ask.display.as_str()),
            (LAPTOP, "HDMI-1")
        );

        let alone = InputSwitch {
            remote: None,
            ..switch
        };
        assert!(matches!(execute(&ddc, &alone).await, Outcome::Failed(_)));
        // A monitor that does not answer the read is not written to blindly.
        assert!(ddc.writes().is_empty());
    }

    #[test]
    fn a_shared_panel_is_read_only_while_this_machines_source_is_shown() {
        let mut d = desk(true, true, true);
        // Both attached: this machine's source is shown first.
        let reads = reads(&d.layout, false);
        assert_eq!(reads.len(), 1);
        assert_eq!(
            (reads[0].source.as_str(), &reads[0].target),
            (d.here.as_str(), &dp1())
        );
        // The laptop shown: what DP-1 reads is the laptop's input, not this machine's.
        d.layout.set_active_source(&d.dell, Some(d.there.clone()));
        assert!(super::reads(&d.layout, true).is_empty());
        // Without VCP control enabled, nothing is ever read.
        let d = desk(false, true, true);
        assert!(super::reads(&d.layout, true).is_empty());
    }

    #[test]
    fn a_panel_of_this_machine_alone_is_read_only_for_a_master() {
        let mut layout = Layout::new(LayoutOptions {
            vcp_control: true,
            ..Default::default()
        });
        add_monitor(&mut layout, &LinuxMonitor::fallback());
        assert!(reads(&layout, false).is_empty());
        assert_eq!(reads(&layout, true).len(), 1);
    }

    #[test]
    fn a_reading_is_learnt_for_the_source_shown_and_for_no_other() {
        let mut d = desk(true, true, true);
        let read = reads(&d.layout, false).remove(0);
        // Read while this machine was shown, but the laptop was chosen since.
        d.layout.set_active_source(&d.dell, Some(d.there.clone()));
        assert!(!learn(&mut d.layout, &[(read.clone(), HDMI)]));
        assert_eq!(d.layout.source(&d.here).unwrap().source.input_source, None);
        d.layout.set_active_source(&d.dell, Some(d.here.clone()));
        assert!(learn(&mut d.layout, &[(read.clone(), DP)]));
        assert_eq!(
            d.layout.source(&d.here).unwrap().source.input_source,
            Some(DP)
        );
        assert!(!learn(&mut d.layout, &[(read, DP)]), "nothing new");
    }

    #[test]
    fn what_a_remote_read_is_adopted_only_while_its_source_is_shown() {
        // Both machines see the Dell attached; this one is shown. What the laptop reads
        // on HDMI-1 is then this machine's input, and must not become the laptop's.
        let mut d = desk(true, true, true);
        d.remotes[0].displays[0].input_source = Some(DP);
        assert!(!adopt_published(&mut d.layout, &d.remotes));
        assert_eq!(d.layout.source(&d.there).unwrap().source.input_source, None);

        // The laptop alone has it: its reading is its input.
        let mut d = desk(true, false, true);
        d.layout.resolve_active_sources();
        d.remotes[0].displays[0].input_source = Some(HDMI);
        assert!(adopt_published(&mut d.layout, &d.remotes));
        assert_eq!(
            d.layout.source(&d.there).unwrap().source.input_source,
            Some(HDMI)
        );
        let merged = without_inputs(&d.remotes);
        assert_eq!(merged[0].displays[0].input_source, None);
    }
}
