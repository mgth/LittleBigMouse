//! The link compiler and the XML it produces, ported from the C# tests
//! `BorderSectionLinkTests` and `WireContractGoldenTests.CurrentLayout*`.

use lbm_layout::geo::Rect;
use lbm_layout::model::{BorderResistance, BorderSection};
use lbm_layout::zoning::{Zone, ZoneLink, ZonesLayout};

// Two 1920x1080 monitors of equal DPI, side by side. Left's right edge runs
// 0..270 mm and maps 1:1 onto Right's left edge.
const EDGE_HEIGHT_MM: f64 = 270.0;

fn zone(
    layout: &ZonesLayout,
    borders: BorderResistance,
    device: &str,
    name: &str,
    px: Rect,
    mm: Rect,
) -> Zone {
    Zone::new(
        layout.zones.len(),
        borders,
        Some(device.to_owned()),
        Some(name.to_owned()),
        px,
        mm,
        None,
    )
}

fn two_monitors(left_borders: BorderResistance) -> ZonesLayout {
    let mut layout = ZonesLayout::default();
    let left = zone(
        &layout,
        left_borders,
        "LEFT",
        "Left",
        Rect::new(-1920.0, 0.0, 1920.0, 1080.0),
        Rect::new(-480.0, 0.0, 480.0, EDGE_HEIGHT_MM),
    );
    layout.zones.push(left);
    let right = zone(
        &layout,
        BorderResistance::default(),
        "RIGHT",
        "Right",
        Rect::new(0.0, 0.0, 1920.0, 1080.0),
        Rect::new(0.0, 0.0, 480.0, EDGE_HEIGHT_MM),
    );
    layout.zones.push(right);
    layout
}

fn right_links_of_left(mut layout: ZonesLayout) -> Vec<ZoneLink> {
    layout.init();
    let left = layout
        .zones
        .iter()
        .position(|z| z.name.as_deref() == Some("Left"))
        .unwrap();
    layout.zones[left].right_links.clone()
}

/// Links that actually cross into the neighbour, in edge order.
fn crossing(links: Vec<ZoneLink>) -> Vec<ZoneLink> {
    links.into_iter().filter(|l| l.target.is_some()).collect()
}

fn section(
    from: f64,
    to: f64,
    mv: f64,
    move_block: bool,
    drag: f64,
    drag_block: bool,
) -> BorderSection {
    BorderSection::new(from, to, mv, move_block, drag, drag_block)
}

#[test]
fn no_sections_leaves_the_edge_free() {
    let links = crossing(right_links_of_left(two_monitors(
        BorderResistance::default(),
    )));
    assert_eq!(links.len(), 1);
    let c = &links[0];
    assert_eq!(c.border_resistance, 0.0);
    assert_eq!(c.drag_resistance, 0.0);
    assert!(!c.move_block);
    assert!(!c.drag_block);
}

#[test]
fn a_section_spanning_the_edge_is_the_former_per_edge_resistance() {
    let mut borders = BorderResistance::default();
    borders
        .right
        .sections
        .push(section(0.0, EDGE_HEIGHT_MM, 7.0, false, 9.0, false));
    let links = crossing(right_links_of_left(two_monitors(borders)));
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].border_resistance, 7.0);
    assert_eq!(links[0].drag_resistance, 9.0);
    assert!(!links[0].move_block);
}

#[test]
fn a_section_splits_the_edge_and_carries_its_own_resistances() {
    let mut borders = BorderResistance::default();
    borders
        .right
        .sections
        .push(section(0.0, EDGE_HEIGHT_MM / 2.0, 0.0, true, 50.0, false));
    let c = crossing(right_links_of_left(two_monitors(borders)));
    assert_eq!(c.len(), 2);
    assert!(c[0].move_block);
    assert_eq!(c[0].drag_resistance, 50.0);
    assert_eq!(c[0].from, 0.0);
    assert_eq!(c[0].to, EDGE_HEIGHT_MM / 2.0);
    // The cut lands on the matching pixel row, which is what the daemon indexes.
    assert_eq!(c[0].source_to_pixel, 540);
    assert!(!c[1].move_block);
    assert_eq!(c[1].border_resistance, 0.0);
    assert_eq!(c[1].drag_resistance, 0.0);
    assert_eq!(c[1].source_from_pixel, 540);
}

#[test]
fn adjacent_sections_with_different_settings_are_not_merged() {
    let mut borders = BorderResistance::default();
    borders
        .right
        .sections
        .push(section(0.0, 90.0, 10.0, false, 0.0, false));
    borders
        .right
        .sections
        .push(section(90.0, 180.0, 20.0, false, 0.0, false));
    let c = crossing(right_links_of_left(two_monitors(borders)));
    assert_eq!(c.len(), 3);
    assert_eq!(c[0].border_resistance, 10.0);
    assert_eq!(c[1].border_resistance, 20.0);
    assert_eq!(c[2].border_resistance, 0.0);
}

#[test]
fn a_negative_resistance_is_refused_rather_than_carried_to_the_daemon() {
    // The C# test also pins the property-change notification the editor needs;
    // the value rule is what the model carries.
    let mut s = section(0.0, 0.0, 5.0, false, 5.0, false);
    s.set_move_resistance(-5.0);
    assert_eq!(s.move_resistance(), 0.0);
    s.set_move_resistance(-1.0);
    assert_eq!(s.move_resistance(), 0.0);
}

#[test]
fn adjacent_sections_with_identical_settings_still_merge() {
    let mut borders = BorderResistance::default();
    borders
        .right
        .sections
        .push(section(0.0, 90.0, 10.0, false, 0.0, false));
    borders
        .right
        .sections
        .push(section(90.0, 180.0, 10.0, false, 0.0, false));
    let c = crossing(right_links_of_left(two_monitors(borders)));
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].border_resistance, 10.0);
    assert_eq!(c[0].to, 180.0);
}

#[test]
fn sections_are_relative_to_the_edge_start_corner() {
    let mut borders = BorderResistance::default();
    borders
        .right
        .sections
        .push(section(0.0, 135.0, 0.0, true, 0.0, false));
    let mut layout = ZonesLayout::default();
    let left = zone(
        &layout,
        borders,
        "LEFT",
        "Left",
        Rect::new(-1920.0, 0.0, 1920.0, 1080.0),
        Rect::new(-480.0, 1000.0, 480.0, EDGE_HEIGHT_MM),
    );
    layout.zones.push(left);
    let right = zone(
        &layout,
        BorderResistance::default(),
        "RIGHT",
        "Right",
        Rect::new(0.0, 0.0, 1920.0, 1080.0),
        Rect::new(0.0, 1000.0, 480.0, EDGE_HEIGHT_MM),
    );
    layout.zones.push(right);
    let c = crossing(right_links_of_left(layout));
    assert_eq!(c.len(), 2);
    assert!(c[0].move_block);
    assert_eq!(c[0].from, 1000.0);
    assert_eq!(c[0].to, 1135.0);
    assert!(!c[1].move_block);
}

#[test]
fn serialized_link_keeps_the_historical_attribute_names() {
    let mut borders = BorderResistance::default();
    borders
        .right
        .sections
        .push(section(0.0, EDGE_HEIGHT_MM, 3.0, false, 4.0, true));
    let mut layout = two_monitors(borders);
    layout.init();
    let xml = layout.serialize();
    assert!(xml.contains(r#"BorderResistance="3""#), "{xml}");
    assert!(xml.contains(r#"DragResistance="4""#));
    assert!(xml.contains(r#"MoveBlock="False""#));
    assert!(xml.contains(r#"DragBlock="True""#));
}

/// `WireContractGoldenTests.CurrentLayout`: the layout whose serialization C#
/// records as `wire-contract/goldens/ui-to-daemon/layout-v5.6-current.xml`.
fn current_layout() -> ZonesLayout {
    let mut left_borders = BorderResistance::default();
    left_borders
        .right
        .sections
        .push(section(0.0, 135.0, 12.5, false, 30.0, false));
    left_borders
        .right
        .sections
        .push(section(135.0, 270.0, 12.5, false, 0.0, true));
    let mut layout = ZonesLayout {
        adjust_pointer: true,
        adjust_speed: false,
        loop_x: false,
        loop_y: false,
        virtual_layout: false,
        rescue_shortcut: "Ctrl+Alt+Shift+M".to_owned(),
        priority: Some("High".to_owned()),
        priority_unhooked: Some("Idle".to_owned()),
        algorithm: "Cross".to_owned(),
        max_travel_distance: 150.0,
        freelook_check_interval: 100.0,
        freelook_enabled: true,
        ..ZonesLayout::default()
    };
    let left = zone(
        &layout,
        left_borders,
        "DISPLAY1",
        "Left & \"Main\"",
        Rect::new(-1920.0, 0.0, 1920.0, 1080.0),
        Rect::new(-480.0, 0.0, 480.0, EDGE_HEIGHT_MM),
    );
    layout.zones.push(left);
    let right = zone(
        &layout,
        BorderResistance::default(),
        "DISPLAY2",
        "Right",
        Rect::new(0.0, 0.0, 1920.0, 1080.0),
        Rect::new(0.0, 0.0, 480.0, EDGE_HEIGHT_MM),
    );
    layout.zones.push(right);
    layout.init();
    layout
}

#[test]
fn current_layout_serializes_byte_for_byte_like_csharp() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("crate lives at rust/crates/lbm-layout");
    let golden = std::fs::read_to_string(
        root.join("wire-contract/goldens/ui-to-daemon/layout-v5.6-current.xml"),
    )
    .expect("golden")
    .replace("\r\n", "\n");
    assert_eq!(current_layout().serialize() + "\n", golden);
}
