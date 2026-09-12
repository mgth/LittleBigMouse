//! The desktop the compositor draws, as against the extent of the layout.
//!
//! They are the same rectangle right up until a monitor is excluded from the layout —
//! and then they are not, which is the whole reason the desktop has to be asked for
//! rather than inferred. What consumes it is the Linux hook's absolute pointing
//! device, whose range a compositor maps onto the whole desktop: declare the layout's
//! extent there and every position is scaled by the ratio between the two.

mod common;

use common::{add_with_source, design_options, model};
use lbm_layout::geo::Rect;
use lbm_layout::model::{DisplaySize, DisplaySource, Layout, Monitor, PhysicalSource};
use lbm_layout::zoning::compute_zones;

fn add_monitor(layout: &mut Layout, id: &str, x: f64, width: f64, primary: bool) {
    let model = model(layout, &format!("PNP_{id}"), 500.0, 280.0, Some(0.0));
    let monitor = Monitor::new(id, &model);
    let mut source = DisplaySource::new(format!("{id}-src"));
    source.attached_to_desktop = true;
    source.primary = primary;
    source.in_pixel = DisplaySize::from_rect(Rect::new(x, 0.0, width, 1080.0));
    let physical_source = PhysicalSource::new(format!("{id}-dev"), id, source);
    add_with_source(layout, monitor, physical_source, true);
}

fn two_screens() -> Layout {
    let mut layout = Layout::new(design_options());
    add_monitor(&mut layout, "LEFT", 0.0, 1920.0, true);
    add_monitor(&mut layout, "RIGHT", 1920.0, 1920.0, false);
    layout
}

/// The ordinary case: nothing excluded, so the two agree and nothing changes for
/// anyone who was inferring one from the other.
#[test]
fn with_nothing_excluded_the_desktop_is_the_layouts_own_extent() {
    let layout = two_screens();

    let desktop = layout
        .desktop_pixel_bounds()
        .expect("two screens are attached");
    let zones = compute_zones(&layout);
    let extent = zones_extent(&zones);

    assert_eq!(desktop, Rect::new(0.0, 0.0, 3840.0, 1080.0));
    assert_eq!(extent, Some(desktop));
}

/// And the case that makes the question worth asking. Excluding a monitor says the
/// cursor should not go there — not that the screen stopped existing: the compositor
/// still draws it, and the desktop is still 3840 wide.
#[test]
fn an_excluded_monitor_leaves_the_desktop_alone_and_shrinks_the_layout() {
    let mut layout = two_screens();
    layout.set_excluded("RIGHT", true);

    let desktop = layout
        .desktop_pixel_bounds()
        .expect("both are still attached");
    let extent = zones_extent(&compute_zones(&layout));

    assert_eq!(
        desktop,
        Rect::new(0.0, 0.0, 3840.0, 1080.0),
        "the compositor still draws the excluded screen"
    );
    assert_eq!(
        extent,
        Some(Rect::new(0.0, 0.0, 1920.0, 1080.0)),
        "the cursor is meant to stay off it"
    );
}

/// What the hook used to infer: the union of the zones' pixel rectangles.
fn zones_extent(zones: &lbm_layout::zoning::ZonesLayout) -> Option<Rect> {
    let mut it = zones.zones.iter().map(|z| z.pixels_bounds);
    let first = it.next()?;
    let (mut l, mut t, mut r, mut b) = (first.left(), first.top(), first.right(), first.bottom());
    for z in it {
        l = l.min(z.left());
        t = t.min(z.top());
        r = r.max(z.right());
        b = b.max(z.bottom());
    }
    Some(Rect::new(l, t, r - l, b - t))
}
