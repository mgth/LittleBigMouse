//! A layout holding a remote machine's screens, through the store (docs/multi-pc.md):
//! which machine a source is on, the input value that selects it, and which source a
//! shared panel shows.

mod common;

use common::{design_options, temp_excluded_file, test_persistence, FakeStore};
use lbm_layout::geo::Point;
use lbm_layout::linux::{add_monitor, LinuxEdid, LinuxMonitor};
use lbm_layout::model::Layout;
use lbm_layout::remote::{merge_remote, remote_source_id, RemoteMonitor};
use lbm_store::LayoutDocument;
use serde_json::Value;

const LAPTOP: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const SHARED: &str = "DEL4065_ABC123";

fn dell() -> LinuxMonitor {
    LinuxMonitor {
        connector_name: "DP-1".to_owned(),
        logical_x: 0.0,
        logical_y: 0.0,
        logical_width: 2560.0,
        logical_height: 1440.0,
        pixel_width: 2560,
        pixel_height: 1440,
        scale: 1.0,
        width_mm: 597.0,
        height_mm: 336.0,
        primary: true,
        enabled: true,
        orientation: 0,
        frequency: 60,
        edid: Some(LinuxEdid {
            manufacturer_code: Some("DEL".to_owned()),
            product_code: Some("4065".to_owned()),
            serial: Some("00000000".to_owned()),
            serial_number: Some("ABC123".to_owned()),
            model: Some("DELL U2720Q".to_owned()),
            physical_width: 597.0,
            physical_height: 336.0,
            video_interface: Some("DisplayPort".to_owned()),
        }),
    }
}

fn laptop(dell_attached: bool) -> Vec<RemoteMonitor> {
    let display = |id: &str, key: Option<&str>, x: i32| RemoteMonitor {
        id: id.to_owned(),
        connector: id.to_owned(),
        panel_key: key.map(str::to_owned),
        x,
        width: 1920,
        height: 1080,
        scale: 1.0,
        attached: true,
        width_mm: 344.0,
        height_mm: 194.0,
        ..RemoteMonitor::default()
    };
    let mut dell = display("HDMI-1", Some(SHARED), 0);
    dell.attached = dell_attached;
    vec![display("eDP-1", None, -1920), dell]
}

/// What the agent builds at each display change: this machine's screens, then the
/// laptop's merged in.
fn rebuilt(dell_attached_on_laptop: bool) -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(&mut layout, &dell());
    merge_remote(
        &mut layout,
        LAPTOP,
        "laptop",
        &laptop(dell_attached_on_laptop),
    );
    layout.id = layout.compute_id();
    layout
}

fn source_json<'a>(json: &'a Value, monitor: &str, source: &str) -> &'a Value {
    &json["Layout"]["Monitors"][monitor]["Sources"][source]
}

#[test]
fn the_document_says_which_machine_a_source_is_on_and_how_to_select_it() {
    let mut layout = rebuilt(true);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    layout.set_active_source(SHARED, Some(remote_dell.clone()));
    layout.set_source_input(SHARED, Some(0x0F));
    layout.set_source_input(&remote_dell, Some(0x11));

    let json = serde_json::to_value(LayoutDocument::of(&layout)).unwrap();

    let local = source_json(&json, SHARED, SHARED);
    assert_eq!(local.get("Host"), None);
    assert_eq!(local["InputSource"], 15);
    let remote = source_json(&json, SHARED, &remote_dell);
    assert_eq!(remote["Host"], LAPTOP);
    assert_eq!(remote["InputSource"], 17);
    assert_eq!(
        json["Layout"]["Monitors"][SHARED]["ActiveSource"],
        remote_dell.as_str()
    );
    let panel = remote_source_id(LAPTOP, "eDP-1");
    assert_eq!(source_json(&json, &panel, &panel)["Host"], LAPTOP);
}

#[test]
fn a_rebuilt_layout_gets_back_the_choice_the_inputs_and_the_remote_placement() {
    let mut saved = rebuilt(true);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    let panel = remote_source_id(LAPTOP, "eDP-1");
    saved.set_active_source(SHARED, Some(remote_dell.clone()));
    saved.set_source_input(SHARED, Some(0x0F));
    saved.set_location(&panel, Point::new(-400.0, 25.0));
    saved.set_placed(&panel, true);
    let text = serde_json::to_string(&LayoutDocument::of(&saved)).unwrap();

    let mut layout = rebuilt(true);
    let document: LayoutDocument = serde_json::from_str(&text).unwrap();
    document.apply(&mut layout);

    let shared = layout.monitor(SHARED).unwrap();
    assert_eq!(shared.active_source.as_deref(), Some(remote_dell.as_str()));
    assert_eq!(
        layout.source(SHARED).unwrap().source.input_source,
        Some(0x0F)
    );
    let restored = layout.monitor(&panel).unwrap();
    assert_eq!(restored.location(), Point::new(-400.0, 25.0));
    assert!(restored.placed);
}

#[test]
fn a_stored_choice_of_a_source_gone_dark_gives_way_to_the_one_still_showing() {
    let mut saved = rebuilt(true);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    saved.set_active_source(SHARED, Some(remote_dell));
    let text = serde_json::to_string(&LayoutDocument::of(&saved)).unwrap();

    let mut layout = rebuilt(false);
    let document: LayoutDocument = serde_json::from_str(&text).unwrap();
    document.apply(&mut layout);

    assert_eq!(
        layout.monitor(SHARED).unwrap().active_source.as_deref(),
        Some(SHARED)
    );
}

#[test]
fn a_detached_source_is_kept_in_the_store_for_the_input_it_remembers() {
    let mut layout = rebuilt(false);
    let remote_dell = remote_source_id(LAPTOP, "HDMI-1");
    layout.set_source_input(&remote_dell, Some(0x11));
    let json = serde_json::to_value(LayoutDocument::of(&layout)).unwrap();
    assert_eq!(source_json(&json, SHARED, &remote_dell)["InputSource"], 17);

    let mut rebuilt = rebuilt(false);
    let document: LayoutDocument = serde_json::from_value(json).unwrap();
    document.apply(&mut rebuilt);
    assert_eq!(
        rebuilt.source(&remote_dell).unwrap().source.input_source,
        Some(0x11)
    );
}

// A panel a hardware KVM moves between this machine and the laptop: one profile file,
// whichever machine shows it.

const SAMSUNG: &str = "SAME035_H1AK500000";
const SAMSUNG_THERE: &str = r"MONITOR\SAME035\{4d36e96e-e325-11ce-bfc1-08002be10318}\0010";

fn samsung_here() -> LinuxMonitor {
    LinuxMonitor {
        connector_name: "HDMI-A-1".to_owned(),
        logical_x: 2560.0,
        logical_width: 3840.0,
        logical_height: 2160.0,
        pixel_width: 3840,
        pixel_height: 2160,
        width_mm: 698.0,
        height_mm: 393.0,
        primary: false,
        edid: Some(LinuxEdid {
            manufacturer_code: Some("SAM".to_owned()),
            product_code: Some("E035".to_owned()),
            serial: Some("00000000".to_owned()),
            serial_number: Some("H1AK500000".to_owned()),
            model: Some("Odyssey G80SD".to_owned()),
            physical_width: 698.0,
            physical_height: 393.0,
            video_interface: Some("HDMI".to_owned()),
        }),
        ..dell()
    }
}

fn samsung_there() -> RemoteMonitor {
    RemoteMonitor {
        id: SAMSUNG_THERE.to_owned(),
        connector: r"\\.\DISPLAY1".to_owned(),
        panel_key: Some(SAMSUNG.to_owned()),
        pnp_code: Some("SAME035".to_owned()),
        width: 3840,
        height: 2160,
        scale: 1.0,
        attached: true,
        width_mm: 698.0,
        height_mm: 393.0,
        ..RemoteMonitor::default()
    }
}

/// Where the KVM points: this machine, the laptop, or both machines keep a link.
#[derive(Clone, Copy)]
enum Kvm {
    Here,
    There,
    Both,
}

fn desk(kvm: Kvm) -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(&mut layout, &dell());
    if matches!(kvm, Kvm::Here | Kvm::Both) {
        add_monitor(&mut layout, &samsung_here());
    }
    let remote = match kvm {
        Kvm::Here => vec![],
        Kvm::There | Kvm::Both => vec![samsung_there()],
    };
    merge_remote(&mut layout, LAPTOP, "laptop", &remote);
    layout.id = layout.compute_id();
    layout
}

fn samsung_there_source() -> String {
    remote_source_id(LAPTOP, SAMSUNG_THERE)
}

type Persistence = lbm_store::LayoutPersistence<FakeStore, common::TestPlatform>;

fn persistence() -> (tempfile::TempDir, FakeStore, Persistence) {
    let (dir, excluded) = temp_excluded_file("lbm-kvm");
    let store = FakeStore::default();
    let persistence = test_persistence(store.clone(), &excluded);
    (dir, store, persistence)
}

/// Loads the profile of `kvm`'s desk, places the Samsung, switches the profile on, and
/// saves.
fn save_placed(persistence: &mut Persistence, kvm: Kvm, at: Point) -> Layout {
    let mut layout = desk(kvm);
    persistence.load(&mut layout).unwrap();
    layout.set_location(SAMSUNG, at);
    layout.set_placed(SAMSUNG, true);
    layout.edit_options(|o| o.enabled = true);
    persistence.save(&mut layout).unwrap();
    layout
}

fn loaded(persistence: &mut Persistence, kvm: Kvm) -> Layout {
    let mut layout = desk(kvm);
    persistence.load(&mut layout).unwrap();
    layout
}

fn stored_sources(store: &FakeStore, layout_id: &str) -> Vec<String> {
    store.layouts.borrow()[layout_id].monitors[SAMSUNG]
        .sources
        .as_ref()
        .map(|s| s.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn a_panel_the_kvm_moves_keeps_one_profile() {
    assert_eq!(desk(Kvm::Here).id, "DEL4065_ABC123+SAME035_H1AK500000");
    assert_eq!(desk(Kvm::There).id, desk(Kvm::Here).id);
    assert_eq!(desk(Kvm::Both).id, desk(Kvm::Here).id);
}

#[test]
fn placed_while_shown_here_then_loaded_while_shown_there_keeps_its_place_and_shows_the_laptop() {
    let (_dir, store, mut persistence) = persistence();
    let here = save_placed(&mut persistence, Kvm::Here, Point::new(650.0, -40.0));
    assert_eq!(stored_sources(&store, &here.id), vec![SAMSUNG.to_owned()]);
    assert_eq!(
        store.layouts.borrow()[&here.id].monitors[SAMSUNG]
            .active_source
            .as_deref(),
        Some(SAMSUNG),
        "the file names a source the next load will not have"
    );

    let there = loaded(&mut persistence, Kvm::There);
    let samsung = there.monitor(SAMSUNG).unwrap();
    assert_eq!(samsung.location(), Point::new(650.0, -40.0));
    assert!(samsung.placed);
    assert_eq!(
        samsung.active_source.as_deref(),
        Some(samsung_there_source().as_str())
    );
    assert!(there.options.enabled, "the profile is the one switched on");
    let zones = lbm_layout::zoning::compute_zones(&there);
    assert!(zones.zones.iter().any(|z| {
        z.device_id.as_deref() == Some(samsung_there_source().as_str())
            && z.host.as_deref() == Some(LAPTOP)
    }));
}

#[test]
fn placed_while_shown_there_then_loaded_while_shown_here_keeps_its_place_and_shows_this_machine() {
    let (_dir, store, mut persistence) = persistence();
    let there = save_placed(&mut persistence, Kvm::There, Point::new(-700.0, 15.0));
    assert_eq!(
        stored_sources(&store, &there.id),
        vec![samsung_there_source()]
    );

    let here = loaded(&mut persistence, Kvm::Here);
    let samsung = here.monitor(SAMSUNG).unwrap();
    assert_eq!(samsung.location(), Point::new(-700.0, 15.0));
    assert!(samsung.placed);
    assert_eq!(samsung.active_source.as_deref(), Some(SAMSUNG));
    assert!(here.options.enabled);
}

#[test]
fn a_panel_both_machines_keep_a_link_to_stays_one_monitor_with_two_sources() {
    let (_dir, store, mut persistence) = persistence();
    let mut both = desk(Kvm::Both);
    persistence.load(&mut both).unwrap();
    assert_eq!(
        both.monitor(SAMSUNG).unwrap().sources,
        vec![SAMSUNG.to_owned(), samsung_there_source()]
    );
    assert_eq!(
        both.monitor(SAMSUNG).unwrap().active_source.as_deref(),
        Some(SAMSUNG),
        "attached on both, no choice yet: this machine's"
    );
    both.set_active_source(SAMSUNG, Some(samsung_there_source()));
    both.set_location(SAMSUNG, Point::new(650.0, -40.0));
    persistence.save(&mut both).unwrap();
    assert_eq!(
        stored_sources(&store, &both.id),
        vec![SAMSUNG.to_owned(), samsung_there_source()]
    );

    let again = loaded(&mut persistence, Kvm::Both);
    let samsung = again.monitor(SAMSUNG).unwrap();
    assert_eq!(samsung.location(), Point::new(650.0, -40.0));
    assert_eq!(
        samsung.active_source.as_deref(),
        Some(samsung_there_source().as_str())
    );
}

#[test]
fn alternating_saves_keep_what_each_machines_source_learnt() {
    let (_dir, store, mut persistence) = persistence();
    // Both linked once: each source learns the input that selects it.
    let mut both = desk(Kvm::Both);
    persistence.load(&mut both).unwrap();
    both.set_source_input(SAMSUNG, Some(0x0F));
    both.set_source_input(&samsung_there_source(), Some(0x11));
    persistence.save(&mut both).unwrap();

    // Then each machine alone, saving each time.
    let mut here = loaded(&mut persistence, Kvm::Here);
    persistence.save(&mut here).unwrap();
    let mut there = loaded(&mut persistence, Kvm::There);
    persistence.save(&mut there).unwrap();
    let mut here = loaded(&mut persistence, Kvm::Here);
    persistence.save(&mut here).unwrap();

    assert_eq!(
        here.source(SAMSUNG).unwrap().source.input_source,
        Some(0x0F)
    );
    assert_eq!(
        stored_sources(&store, &here.id),
        vec![SAMSUNG.to_owned(), samsung_there_source()]
    );
    let both = loaded(&mut persistence, Kvm::Both);
    assert_eq!(
        both.source(&samsung_there_source())
            .unwrap()
            .source
            .input_source,
        Some(0x11)
    );
    assert_eq!(
        both.source(SAMSUNG).unwrap().source.input_source,
        Some(0x0F)
    );
}

#[test]
fn a_source_the_layout_holds_is_never_taken_back_from_the_file() {
    let (_dir, store, mut persistence) = persistence();
    let mut both = desk(Kvm::Both);
    persistence.load(&mut both).unwrap();
    both.set_source_input(&samsung_there_source(), Some(0x11));
    persistence.save(&mut both).unwrap();

    // The laptop's source forgets its input: the file must say so, not resurrect it.
    let mut both = loaded(&mut persistence, Kvm::Both);
    both.set_source_input(&samsung_there_source(), None);
    persistence.save(&mut both).unwrap();
    let layouts = store.layouts.borrow();
    let stored = &layouts[&both.id].monitors[SAMSUNG]
        .sources
        .as_ref()
        .unwrap()[&samsung_there_source()];
    assert_eq!(stored.input_source, None);
}

#[test]
fn a_layout_without_remotes_stores_neither_host_nor_input() {
    let mut layout = Layout::new(design_options());
    add_monitor(&mut layout, &dell());
    let text = serde_json::to_string(&LayoutDocument::of(&layout)).unwrap();
    assert!(!text.contains("\"Host\""));
    assert!(!text.contains("InputSource"));
}
