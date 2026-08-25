//! Golden tests of the UI↔daemon wire contract, over the shared corpus in
//! `wire-contract/goldens`.
//!
//! The C# side's `WireContractGoldenTests` reads the SAME files from the source tree,
//! so a payload the UI produces is provably the payload this daemon parses — neither
//! end compares against its own private copy. `wire_contract.rs` covers the transport
//! (framing, duplex, reconnection); this file covers the payloads that ride it.
//!
//! Authority differs by direction:
//!
//! * `ui-to-daemon/` — C# is the producer of record. Its `ZoneSerializer` derives the
//!   XML names by reflection from C# member names, so those goldens are regenerated on
//!   the C# side and only ever PARSED here. A parse that changes meaning is a
//!   regression in this crate, not in the golden.
//! * `daemon-to-ui/` — this crate is the producer of record. Those goldens are
//!   regenerated here with `LBM_UPDATE_GOLDEN=1` and only ever parsed on the C# side.
//!
//! See `wire-contract/README.md` for the procedure when a message changes.

use std::path::PathBuf;

use littlebigmouse_hook::engine::probe;
use littlebigmouse_hook::ipc::protocol;
use littlebigmouse_hook::priority::Priority;
use littlebigmouse_hook::zones::zone_link::{MODE_DRAG, MODE_MOVE};
use littlebigmouse_hook::zones::{Algorithm, ZonesLayout};

fn goldens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate has a parent directory")
        .join("wire-contract/goldens")
}

fn read(relative: &str) -> String {
    let path = goldens().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing golden {}: {e}", path.display()))
        .trim_end_matches('\n')
        .to_string()
}

fn parse(relative: &str) -> ZonesLayout {
    ZonesLayout::from_xml(&read(relative))
        .unwrap_or_else(|| panic!("golden {relative} must parse as a ZonesLayout"))
}

/// Compare against a golden this crate OWNS (daemon→UI). Honours `LBM_UPDATE_GOLDEN`.
fn assert_owned_golden(relative: &str, actual: &str) {
    let path = goldens().join(relative);
    if std::env::var("LBM_UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{}\n", actual.trim_end_matches('\n'))).unwrap();
    }
    assert_eq!(
        read(relative),
        actual.trim_end_matches('\n'),
        "golden {relative} no longer matches what the daemon emits"
    );
}

/// The probe report for the current layout golden, recomputed from the real engine.
fn current_probe_report() -> String {
    probe::probe_xml(&read("ui-to-daemon/layout-v5.6-current.xml"))
        .expect("the current layout golden must probe")
}

fn zone_named<'a>(layout: &'a ZonesLayout, name: &str) -> &'a littlebigmouse_hook::zones::Zone {
    let zid = *layout
        .zones
        .iter()
        .find(|&&z| layout.arena[z].name == name)
        .unwrap_or_else(|| panic!("no zone named {name:?}"));
    &layout.arena[zid]
}

// =========================================================================
// UI→daemon — versions
//
// The per-version shapes are not invented: they are what `ZonesLayout.Serialize`,
// `Zone.Serialize` and `ZoneLink.Serialize` emitted at each tag (`git show
// v5.2.3:...`). A user who has not upgraded the daemon and the UI in lockstep sends
// exactly these.
// =========================================================================

/// v5.2.3 predates `Virtual`, `RescueShortcut`, the freelook options, `Zone.DeviceId`
/// and the whole move/drag split. Every one of those absences must land on the value
/// that reproduces the old behaviour — not on a type default that happens to be handy.
#[test]
fn layout_from_v5_2_3_keeps_its_old_behaviour() {
    let layout = parse("ui-to-daemon/layout-v5.2.3.xml");

    assert_eq!(layout.zones.len(), 2);
    assert_eq!(layout.main_zones.len(), 2);
    assert_eq!(layout.algorithm, Algorithm::Strait);
    assert_eq!(layout.priority, Priority::Normal);
    assert_eq!(layout.priority_unhooked, Priority::Above);
    assert_eq!(layout.max_travel_distance_squared, 200.0 * 200.0);

    // Absent because the version could not send them.
    assert!(
        !layout.virtual_layout,
        "a pre-Virtual layout is never virtual"
    );
    assert_eq!(layout.rescue_shortcut, "", "no shortcut travelled with it");
    assert_eq!(zone_named(&layout, "Left & Main").device_id, "");

    // Freelook: no interval means "re-check every event", the pre-throttle behaviour.
    // Enabled defaults to true, because the off switch did not exist to be off.
    assert_eq!(layout.freelook_check_interval_ms, 0);
    assert!(layout.freelook_enabled);

    // The move/drag split: one BorderResistance governed BOTH modes, so the missing
    // DragResistance must fall back to it rather than to 0 (which would silently
    // unblock every dragged crossing).
    let link = &zone_named(&layout, "Left & Main").right[0];
    assert_eq!(link.border_resistance[MODE_MOVE], 12.5);
    assert_eq!(link.border_resistance[MODE_DRAG], 12.5);
    assert_eq!(
        link.border_resistance_px[MODE_MOVE],
        link.border_resistance_px[MODE_DRAG]
    );
}

/// v5.5.2 added `Virtual`, the freelook options and `Zone.DeviceId`, but still predates
/// the move/drag split.
#[test]
fn layout_from_v5_5_2_carries_device_ids_and_freelook_but_no_drag_split() {
    let layout = parse("ui-to-daemon/layout-v5.5.2.xml");

    assert_eq!(layout.algorithm, Algorithm::CornerCrossing);
    assert_eq!(layout.priority, Priority::High);
    assert_eq!(layout.priority_unhooked, Priority::Idle);
    assert!(layout.adjust_pointer);
    assert!(!layout.adjust_speed);
    assert!(layout.loop_x);
    assert!(!layout.loop_y);
    assert!(!layout.virtual_layout);
    assert_eq!(layout.freelook_check_interval_ms, 100);
    assert!(layout.freelook_enabled);
    assert_eq!(layout.max_travel_distance_squared, 150.0 * 150.0);

    assert_eq!(zone_named(&layout, "Left & Main").device_id, "DISPLAY1");
    assert_eq!(zone_named(&layout, "Right").device_id, "DISPLAY2");

    // Still one resistance for both modes.
    let link = &zone_named(&layout, "Left & Main").right[0];
    assert_eq!(link.border_resistance[MODE_MOVE], 12.5);
    assert_eq!(link.border_resistance[MODE_DRAG], 12.5);
}

/// The current golden, produced by the C# serializer itself (regenerate on that side).
#[test]
fn layout_from_the_current_ui_parses_with_the_move_drag_split() {
    let layout = parse("ui-to-daemon/layout-v5.6-current.xml");

    assert_eq!(layout.algorithm, Algorithm::CornerCrossing);
    assert_eq!(layout.priority, Priority::High);
    assert_eq!(layout.priority_unhooked, Priority::Idle);
    assert_eq!(layout.rescue_shortcut, "Ctrl+Alt+Shift+M");
    assert!(layout.adjust_pointer);
    assert_eq!(layout.max_travel_distance_squared, 150.0 * 150.0);

    // Attribute escaping survives the trip: the UI escapes & and " in EDID names.
    let left = zone_named(&layout, "Left & \"Main\"");
    assert_eq!(left.device_id, "DISPLAY1");

    // Two sections on the right edge became two crossing links with distinct
    // resistances, plus the wall runs above and below.
    let crossing: Vec<_> = left.right.iter().filter(|l| l.target.is_some()).collect();
    assert_eq!(crossing.len(), 2, "one link per section");

    // First section: a plain resistance, heavier for drags than for moves.
    assert_eq!(crossing[0].border_resistance[MODE_MOVE], 12.5);
    assert_eq!(crossing[0].border_resistance[MODE_DRAG], 30.0);

    // Second section: DragBlock="True" is an undrainable resistance, and it must not
    // touch the move mode.
    assert_eq!(crossing[1].border_resistance[MODE_MOVE], 12.5);
    assert_eq!(crossing[1].border_resistance[MODE_DRAG], f64::INFINITY);
    assert_eq!(crossing[1].border_resistance_px[MODE_DRAG], i64::MAX);
    assert!(crossing[1].border_resistance_px[MODE_MOVE] < i64::MAX);

    // The catch-all runs beyond the shared edge are walls (TargetId="-1").
    assert!(left.right.iter().any(|l| l.target.is_none()));
}

/// The pixel/millimetre model is duplicated on both sides (C# `Zone.Init` builds
/// matrices, this crate builds `physical_inside` + a DPI). They must agree on the SAME
/// serialized bounds, or the cursor lands somewhere the UI never drew.
#[test]
fn geometry_derived_from_the_golden_agrees_with_what_the_ui_computed() {
    let layout = parse("ui-to-daemon/layout-v5.6-current.xml");
    let right = zone_named(&layout, "Right");

    // 1920 px over 480 mm => 101.6 dpi on both axes.
    assert!(
        (right.dpi - 101.6).abs() < 0.05,
        "dpi was {}, expected ~101.6",
        right.dpi
    );

    // Pixel -> physical -> pixel round-trips.
    let px = littlebigmouse_hook::geometry::Point::new(960, 540);
    let back = right.to_pixels(right.to_physical(px));
    assert_eq!((back.x(), back.y()), (px.x(), px.y()));

    // The zone owns its own pixels and not its neighbour's.
    assert!(right.contains_pixel(littlebigmouse_hook::geometry::Point::new(0, 0)));
    assert!(!right.contains_pixel(littlebigmouse_hook::geometry::Point::new(-1, 0)));
    assert!(!right.contains_pixel(littlebigmouse_hook::geometry::Point::new(1920, 0)));
}

// =========================================================================
// UI→daemon — unknown values and forward compatibility
// =========================================================================

/// A layout from a UI newer than this daemon: unknown attributes, unknown child
/// elements. The daemon must read what it knows and ignore the rest, never fail the
/// whole load — a rejected layout means a cursor stuck with no configuration at all.
#[test]
fn unknown_fields_from_a_newer_ui_are_ignored_not_fatal() {
    let layout = parse("ui-to-daemon/layout-future-unknown-fields.xml");

    assert_eq!(layout.zones.len(), 2);
    assert_eq!(layout.main_zones.len(), 2);
    assert_eq!(layout.algorithm, Algorithm::Strait);
    assert_eq!(layout.rescue_shortcut, "Ctrl+Alt+Shift+M");
    assert_eq!(layout.max_travel_distance_squared, 200.0 * 200.0);

    // The known links parsed despite an unknown attribute sitting among them.
    let left = zone_named(&layout, "Left & Main");
    let crossing: Vec<_> = left.right.iter().filter(|l| l.target.is_some()).collect();
    assert_eq!(crossing.len(), 1);
    assert_eq!(crossing[0].border_resistance[MODE_MOVE], 12.5);
    assert_eq!(crossing[0].border_resistance[MODE_DRAG], 30.0);
}

/// Enum values the daemon does not know must fall back to the documented default, not
/// to whatever `parse()` happens to return. An unknown priority that landed on Realtime
/// would be a machine-wide hazard.
#[test]
fn unknown_enum_values_fall_back_to_the_documented_defaults() {
    let layout = parse("ui-to-daemon/layout-unknown-enum-values.xml");

    assert_eq!(
        layout.priority,
        Priority::Normal,
        "unknown priority -> Normal"
    );
    assert_eq!(
        layout.priority_unhooked,
        Priority::Normal,
        "empty priority -> Normal"
    );
    assert_eq!(
        layout.algorithm,
        Algorithm::Strait,
        "unknown algorithm -> the cheap, safe one"
    );
    assert_eq!(layout.zones.len(), 1);
}

/// The one enum whose valid set used to be written down in three places that disagreed.
///
/// `"Cross"` is the wire value: it is what every shipped release offers in
/// `LbmOptionsViewModel.AlgorithmList`, so it is the only spelling a real configuration
/// has ever contained. `"CornerCrossing"` is the spelling that the C# doc comment and the
/// persistence fixtures carried for a long time while the parser did NOT accept it — it
/// landed on Strait, silently. It is now tolerated as an alias, so that a hand-edited
/// config or a migration that trusted the documentation does what it says.
///
/// The alias is a safety net, not a second blessed name: the UI must keep emitting
/// `"Cross"`, which is what `AlgorithmWireSpellingsAreTheOnesTheDaemonUnderstands` pins
/// on the C# side.
#[test]
fn cross_is_the_wire_value_and_corner_crossing_is_tolerated_as_an_alias() {
    let cross = read("ui-to-daemon/layout-v5.6-current.xml");
    assert!(
        cross.contains(r#"Algorithm="Cross""#),
        "the UI must emit the wire value, not the alias"
    );
    assert_eq!(
        ZonesLayout::from_xml(&cross).unwrap().algorithm,
        Algorithm::CornerCrossing
    );

    let documented = cross.replace(r#"Algorithm="Cross""#, r#"Algorithm="CornerCrossing""#);
    assert_eq!(
        ZonesLayout::from_xml(&documented).unwrap().algorithm,
        Algorithm::CornerCrossing,
        "the documented spelling must no longer land on Strait"
    );

    // The alias is exact, not a fuzzy match: everything else is still Strait, so an
    // unknown algorithm stays a silent fallback rather than becoming a guess.
    for wrong in [r#"Algorithm="cross""#, r#"Algorithm="cornercrossing""#] {
        let variant = cross.replace(r#"Algorithm="Cross""#, wrong);
        assert_eq!(
            ZonesLayout::from_xml(&variant).unwrap().algorithm,
            Algorithm::Strait,
            "matching stays case-sensitive ({wrong})"
        );
    }
}

// =========================================================================
// UI→daemon — commands
// =========================================================================

#[test]
fn command_goldens_parse_to_the_commands_they_name() {
    use littlebigmouse_hook::ipc::protocol::Command;

    assert_eq!(
        protocol::parse(&read("ui-to-daemon/command-run.xml")),
        vec![Command::Run]
    );
    assert_eq!(
        protocol::parse(&read("ui-to-daemon/command-stop.xml")),
        vec![Command::Stop]
    );
    assert_eq!(
        protocol::parse(&read("ui-to-daemon/command-quit.xml")),
        vec![Command::Quit]
    );
    assert_eq!(
        protocol::parse(&read("ui-to-daemon/command-shortcut.xml")),
        vec![Command::Shortcut("Ctrl+Alt+Shift+M".to_string())]
    );
}

/// A `Load` golden must yield a payload that parses on its own: the daemon slices the
/// `<ZonesLayout>` subtree back out of the command by source range.
#[test]
fn load_command_golden_yields_a_layout_that_parses_on_its_own() {
    use littlebigmouse_hook::ipc::protocol::Command;

    let commands = protocol::parse(&read("ui-to-daemon/command-load.xml"));
    let [Command::Load(payload)] = &commands[..] else {
        panic!("expected a single Load, got {commands:?}");
    };

    // Byte-identical to the standalone layout golden — the same document, extracted.
    assert_eq!(payload, &read("ui-to-daemon/layout-v5.6-current.xml"));

    let layout = ZonesLayout::from_xml(payload).expect("extracted payload must parse");
    assert_eq!(layout.zones.len(), 2);
    assert_eq!(layout.algorithm, Algorithm::CornerCrossing);
}

/// An unknown command is reported as Unknown rather than dropped, and never mistaken
/// for a known one — the daemon logs it instead of acting on a guess.
#[test]
fn an_unknown_command_from_a_newer_ui_is_named_not_guessed() {
    use littlebigmouse_hook::ipc::protocol::Command;

    assert_eq!(
        protocol::parse(r#"<CommandMessage Command="Hibernate" Payload=""/>"#),
        vec![Command::Unknown("Hibernate".to_string())]
    );
}

// =========================================================================
// daemon→UI — this crate is the producer of record
// =========================================================================

/// Every frame the daemon can emit, in one file, regenerated from `protocol.rs`. The C#
/// side asserts it can parse every line; a frame added here without the UI learning it
/// is a daemon state the user never sees.
#[test]
fn daemon_event_frames_match_the_golden() {
    // Computed rather than read back, so a cold `LBM_UPDATE_GOLDEN=1` regenerates the
    // whole corpus in one pass regardless of test order.
    let report = current_probe_report();

    let frames = [
        protocol::RUNNING.to_string(),
        protocol::STOPPED.to_string(),
        protocol::PAUSED.to_string(),
        protocol::DISPLAY_CHANGED.to_string(),
        protocol::SETTING_CHANGED.to_string(),
        protocol::DESKTOP_CHANGED.to_string(),
        protocol::SUSPENDED.to_string(),
        protocol::RESUMED.to_string(),
        protocol::RESCUED.to_string(),
        protocol::LOAD_FAILED.to_string(),
        protocol::loaded(2, 2, false),
        protocol::shortcut_unavailable("Ctrl+Alt+Shift+M"),
        // A path carrying XML metacharacters: the case a substring parser gets wrong.
        protocol::focus_changed(r#"C:\Games\A&B\<Stopped DisplayChanged>.exe"#),
        protocol::probed(&report),
    ];

    // Each constant is already `\n`-terminated, so the concatenation IS the file.
    assert_owned_golden("daemon-to-ui/events.txt", &frames.concat());
}

/// The probe report for the current layout golden, produced by driving the real engine
/// over it. This is the only artefact where both geometric models meet on the same
/// input: the UI sends bounds and links, the daemon answers with where the cursor
/// actually crosses.
#[test]
fn probe_report_golden_is_what_the_prober_emits_for_the_current_layout() {
    let report = current_probe_report();

    assert_owned_golden("daemon-to-ui/probe-report.xml", &report);

    // The report must describe the layout it was given, not a default.
    assert!(report.starts_with(
        r#"<ProbeReport Algorithm="Cross" LoopX="False" LoopY="False" Virtual="False">"#
    ));
    assert!(report.contains(r#"DeviceId="DISPLAY1""#));
    assert!(report.contains(r#"DeviceId="DISPLAY2""#));
}

/// Payload escaping is where the two XML layers meet: the prober escapes the zone name
/// into the report, then `probed` escapes the whole report into the event payload. The
/// C# side has to get the original text back after unwrapping both.
#[test]
fn probed_payload_survives_two_layers_of_escaping() {
    let report = current_probe_report();
    let frame = protocol::probed(&report);

    let document = roxmltree::Document::parse(frame.trim_end()).expect("frame is well-formed XML");
    let payload = document
        .descendants()
        .find(|n| n.has_tag_name("Payload"))
        .expect("Probed carries a Payload");

    // One unwrap gives back the report document, byte for byte.
    assert_eq!(payload.text().unwrap(), report);

    // A second parse gives back the escaped-once zone name.
    let inner = roxmltree::Document::parse(payload.text().unwrap()).expect("report is well-formed");
    let zone = inner
        .descendants()
        .find(|n| n.has_tag_name("Zone"))
        .expect("report has zones");
    assert_eq!(zone.attribute("Name"), Some(r#"Left & "Main""#));
}
