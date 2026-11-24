//! The multi-PC mode in the window (docs/multi-pc.md, lot 7), through the accessibility
//! tree: the Machines page, the remote monitors' badge and bezel, and a shared panel's
//! switch.
//!
//! What these pin above all is the pairing rule: a machine is trusted by a person who
//! compared fingerprints, so a pending machine's fingerprint is on the page next to the
//! button that accepts it, and that button names the machine it accepts.

use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use lbm_layout::geo::Rect;
use lbm_ui::frame::{self, Host};
use lbm_ui::machines::{self, Did, Panel, Peer, Role, Source, State};
use lbm_ui::map::{self, MapMonitor};
use lbm_ui::options::{self, Page};

const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const DESK: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

fn state(peers: Vec<Peer>) -> State {
    State {
        enabled: true,
        port: 24817,
        name: "desk".into(),
        machine_id: Some(DESK.into()),
        listen_address: Some("0.0.0.0:24817".into()),
        error: None,
        pointer_lock_shortcut: String::new(),
        pointer_locked: false,
        peers,
    }
}

fn laptop(pending: bool) -> Peer {
    Peer {
        machine_id: LAPTOP.into(),
        name: "laptop".into(),
        address: "192.168.1.30:24817".into(),
        connected: true,
        role: Role::Remote,
        pending,
        displays: 2,
        problem: None,
        driven: false,
    }
}

struct Shown {
    state: Option<State>,
    draft: Panel,
    did: Option<Did>,
}

fn page(state: Option<State>) -> Harness<'static, Shown> {
    Harness::builder()
        .with_size(egui::vec2(480.0, 1100.0))
        .build_ui_state(
            |ui, page| {
                lbm_ui::theme::install(ui.ctx());
                if let Some(did) =
                    machines::panel(ui, page.state.as_ref(), &mut page.draft, false, None)
                {
                    page.did = Some(did);
                }
            },
            Shown {
                state,
                draft: Panel::default(),
                did: None,
            },
        )
}

#[test]
fn the_drawer_offers_no_machines_page_to_an_agent_without_the_mode() {
    for (machines, offered) in [(false, false), (true, true)] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(800.0, 700.0))
            .build_ui_state(
                move |ui, page: &mut Page| {
                    lbm_ui::theme::install(ui.ctx());
                    let mut open = true;
                    options::drawer_among(
                        ui.ctx(),
                        ui.max_rect(),
                        &mut open,
                        page,
                        Page::offered(machines),
                        true,
                        |_, _| {},
                    );
                },
                Page::General,
            );
        h.run();
        assert_eq!(
            h.query_by_label("Machines").is_some(),
            offered,
            "MultiPc announced: {machines}"
        );
    }
    // And without it the pages are exactly the ones there always were.
    assert_eq!(Page::offered(false), &Page::ALL);
}

#[test]
fn a_drawer_left_on_machines_falls_back_when_the_page_is_no_longer_offered() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(800.0, 700.0))
        .build_ui_state(
            |ui, page: &mut Page| {
                let mut open = true;
                options::drawer_among(
                    ui.ctx(),
                    ui.max_rect(),
                    &mut open,
                    page,
                    Page::offered(false),
                    true,
                    |_, _| {},
                );
            },
            Page::Machines,
        );
    h.run();
    assert_eq!(*h.state(), Page::General);
}

#[test]
fn this_machine_shows_its_name_fingerprint_and_address() {
    let mut h = page(Some(state(Vec::new())));
    h.run();
    let fingerprint = machines::fingerprint(DESK);
    assert!(fingerprint.starts_with("0F1E 2D3C"), "{fingerprint}");
    assert!(h
        .query_by_label(&format!("This machine's fingerprint {fingerprint}"))
        .is_some());
    assert!(h.query_by_label("Listening on 0.0.0.0:24817").is_some());
    assert_eq!(
        h.get_by_label("Name").accesskit_node().value().as_deref(),
        Some("desk")
    );
}

#[test]
fn turning_the_mode_on_is_asked_of_the_agent_not_assumed() {
    let mut off = state(Vec::new());
    off.enabled = false;
    let mut h = page(Some(off));
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Multi-PC mode")
        .click();
    h.run();
    assert_eq!(h.state().did, Some(Did::Enable(true)));
    // The page still shows what the agent said, until the agent says otherwise.
    assert!(!h.state().state.as_ref().unwrap().enabled);
}

#[test]
fn a_pending_machine_shows_its_fingerprint_beside_accept_and_refuse() {
    let mut h = page(Some(state(vec![laptop(true)])));
    h.run();
    let fingerprint = machines::fingerprint(LAPTOP);
    assert!(
        h.query_by_label(&format!("Fingerprint of laptop {fingerprint}"))
            .is_some(),
        "the fingerprint to compare is not shown"
    );
    assert!(h.query_by_label("Refuse laptop").is_some());
    assert!(
        h.query_by_label("Forget laptop").is_none(),
        "a machine not yet paired cannot be unpaired"
    );

    h.get_by_label("Accept laptop").click();
    h.run();
    assert_eq!(h.state().did, Some(Did::Accept(LAPTOP.into())));
}

#[test]
fn a_paired_machine_says_what_it_is_and_can_be_forgotten() {
    let mut h = page(Some(state(vec![laptop(false)])));
    h.run();
    assert!(h.query_by_label("Connected").is_some());
    assert!(h
        .query_by_label("Remote: its screens are on this map")
        .is_some());
    assert!(h.query_by_label("Accept laptop").is_none());
    h.get_by_label("Forget laptop").click();
    h.run();
    assert_eq!(h.state().did, Some(Did::Forget(LAPTOP.into())));
}

#[test]
fn a_machine_whose_link_failed_says_why() {
    let mut refused = laptop(false);
    refused.connected = false;
    refused.problem = Some("It refused this machine: the user refused the pairing".into());
    let mut h = page(Some(state(vec![refused])));
    h.run();
    assert!(h
        .query_by_label("It refused this machine: the user refused the pairing")
        .is_some());
    // Without a problem, nothing of the kind is said.
    let mut h = page(Some(state(vec![laptop(false)])));
    h.run();
    assert!(h.query_by_label_contains("refused").is_none());
}

#[test]
fn connecting_waits_for_the_mode_and_an_address() {
    let mut off = state(Vec::new());
    off.enabled = false;
    let mut h = page(Some(off));
    h.state_mut().draft.address = "192.168.1.30".into();
    h.run();
    assert!(h.get_by_label("Connect").accesskit_node().is_disabled());

    let mut h = page(Some(state(Vec::new())));
    h.run();
    assert!(
        h.get_by_label("Connect").accesskit_node().is_disabled(),
        "nothing typed"
    );
    h.state_mut().draft.address = " 192.168.1.30 ".into();
    h.run();
    h.get_by_label("Connect").click();
    h.run();
    assert_eq!(h.state().did, Some(Did::Connect("192.168.1.30".into())));
}

#[test]
fn renaming_this_machine_is_offered_once_the_name_differs() {
    let mut h = page(Some(state(Vec::new())));
    h.run();
    assert!(h.get_by_label("Rename").accesskit_node().is_disabled());
    h.state_mut().draft.name = Some("office".into());
    h.run();
    h.get_by_label("Rename").click();
    h.run();
    assert_eq!(h.state().did, Some(Did::Rename("office".into())));
}

#[test]
fn before_the_agent_answers_the_page_says_so_and_offers_nothing() {
    let mut h = page(None);
    h.run();
    assert!(h.query_by_label("Multi-PC mode").is_none());
    assert!(h.query_by_label("Asking the agent…").is_some());
}

fn screens() -> Vec<MapMonitor<'static>> {
    vec![
        MapMonitor {
            id: "DP-1",
            name: "Dell",
            mm_outside: Rect::new(0.0, 0.0, 640.0, 380.0),
            mm_content: Rect::new(20.0, 20.0, 600.0, 340.0),
            logo: None,
            wallpaper: None,
            details: &[],
            probe: None,
        },
        MapMonitor {
            id: "eDP-1@a1b2c3d4",
            name: "Laptop panel",
            mm_outside: Rect::new(700.0, 0.0, 360.0, 220.0),
            mm_content: Rect::new(710.0, 10.0, 340.0, 200.0),
            logo: None,
            wallpaper: None,
            details: &[],
            probe: None,
        },
    ]
}

#[test]
fn a_remote_monitor_on_the_map_carries_its_machine_name() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(|ui| {
            let monitors = screens();
            let fit = map::fit(map::extent(&monitors), ui.max_rect());
            let hosts = [
                None,
                Some(Host {
                    machine: "laptop",
                    remote: true,
                    shared: false,
                    problem: None,
                }),
            ];
            map::draw_hosted(ui, &monitors, &hosts, &fit, None, true);
        });
    h.run();
    let badge = h
        .get_by_label("laptop")
        .accesskit_node()
        .bounding_box()
        .unwrap();
    let dell = h
        .get_by_label("Dell")
        .accesskit_node()
        .bounding_box()
        .unwrap();
    let name = h
        .get_by_label("Laptop panel")
        .accesskit_node()
        .bounding_box()
        .unwrap();
    // Right of the local screen, and under the remote one's name, which is printed on
    // its top bezel: in the remote screen's lit part.
    assert!(
        badge.min_x() > dell.max_x() && badge.min_y() >= name.max_y(),
        "the badge is not on the remote monitor: {badge:?}, its name at {name:?}"
    );
    // The remote monitor is still a monitor: clickable and draggable like the others.
    assert!(h.query_by_label("Laptop panel").is_some());
}

#[test]
fn a_map_without_other_machines_has_no_badge() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(|ui| {
            let monitors = screens();
            let fit = map::fit(map::extent(&monitors), ui.max_rect());
            map::draw_movable(ui, &monitors, &fit, None, true);
        });
    h.run();
    assert!(h.query_by_label("laptop").is_none());
    assert!(h.query_by_label("Dell").is_some());
}

#[test]
fn a_shared_panel_badge_says_it_is_shared() {
    let host = Host {
        machine: "desk",
        remote: false,
        shared: true,
        problem: None,
    };
    assert_eq!(host.badge(), "desk \u{b7} shared");
    let failed = Host {
        problem: Some("DDC/CI: no answer"),
        ..host
    };
    assert_eq!(failed.badge(), "desk \u{b7} shared \u{b7} switch failed");
}

/// The tint is told apart from the local bezel, and from the selection, on the app's
/// own theme and on egui's dark and light ones, while the name printed on the bezel
/// reads about as well as it does on a local one.
#[test]
fn the_remote_bezel_is_told_apart_on_both_themes() {
    fn luminance(c: egui::Color32) -> f32 {
        let lin = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }
    fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    let app = {
        let ctx = egui::Context::default();
        lbm_ui::theme::install(&ctx);
        ctx.style_of(egui::Theme::Dark).visuals.clone()
    };
    for visuals in [app, egui::Visuals::dark(), egui::Visuals::light()] {
        for selected in [false, true] {
            let local = frame::bezel_fill(&visuals, selected);
            let remote = frame::remote_bezel_fill(&visuals, selected);
            let distance = (i32::from(local.r()) - i32::from(remote.r())).abs()
                + (i32::from(local.g()) - i32::from(remote.g())).abs()
                + (i32::from(local.b()) - i32::from(remote.b())).abs();
            assert!(
                distance >= 30,
                "too close to the local bezel: {local:?} {remote:?}"
            );
            let text = visuals.text_color();
            assert!(
                contrast(remote, text) >= 0.75 * contrast(local, text),
                "the name reads worse on {remote:?} than on {local:?}"
            );
        }
        assert_ne!(
            frame::remote_bezel_fill(&visuals, false),
            frame::bezel_fill(&visuals, true),
            "a remote monitor would look selected"
        );
    }
}

fn sources() -> Vec<Source> {
    vec![
        Source {
            id: "DP-1".into(),
            machine: "this machine".into(),
            attached: true,
        },
        Source {
            id: "HDMI-1@a1b2c3d4".into(),
            machine: "laptop".into(),
            attached: true,
        },
    ]
}

#[test]
fn a_shared_panel_says_which_machine_it_shows_and_switches_to_the_other() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, chosen: &mut Option<String>| {
                if let Some(source) = machines::source_switch(ui, &sources(), Some("DP-1"), true) {
                    *chosen = Some(source);
                }
            },
            None,
        );
    h.run();
    assert!(h.query_by_label("Showing this machine").is_some());
    assert!(
        h.query_by_label("Show this machine").is_none(),
        "the source already shown is not offered"
    );
    h.get_by_label("Show laptop").click();
    h.run();
    assert_eq!(h.state().as_deref(), Some("HDMI-1@a1b2c3d4"));
}

#[test]
fn a_shared_panel_cannot_be_switched_while_the_layout_cannot_change() {
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui(|ui| {
            machines::source_switch(ui, &sources(), Some("DP-1"), false);
        });
    h.run();
    assert!(h.get_by_label("Show laptop").accesskit_node().is_disabled());
}

#[test]
fn the_pointer_lock_shortcut_is_recorded_and_taken_away_by_asking_the_agent() {
    let mut h = page(Some(state(Vec::new())));
    h.run();
    h.get_by_label("Pointer lock shortcut: None").click();
    h.run();
    // A bare letter on the way to the combination is not the answer.
    h.key_press(egui::Key::L);
    h.run();
    assert_eq!(h.state().did, None);
    h.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::ALT, egui::Key::L);
    h.run();
    assert_eq!(h.state().did, Some(Did::LockShortcut("Ctrl+Alt+L".into())));

    let mut set = state(Vec::new());
    set.pointer_lock_shortcut = "Ctrl+Alt+L".into();
    let mut h = page(Some(set));
    h.run();
    assert!(h.query_by_label("Pointer lock shortcut: Ctrl+Alt+L").is_some());
    h.get_by_label("No shortcut").click();
    h.run();
    assert_eq!(h.state().did, Some(Did::LockShortcut(String::new())));
}
