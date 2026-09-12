//! `MonitorBorderPolicyTests.cs`: the "Border values" rules — which size a
//! monitor's geometry roots at, and how a monitor comes to own its bezel borders.
//!
//! C#'s `MonitorBorderPolicy` is an object of its own, observed through streams.
//! In Rust its three rules belong to the layout's edits: `Layout::edit_model`
//! runs the mirror, `Layout::set_monitor_borders` detects ownership, and
//! `Layout::effective_physical_size` picks the size. So each test builds a layout
//! holding the policy's model size and one monitor of it, and observes
//! - `EffectiveSize` as `Layout::effective_physical_size`, a value computed on
//!   demand: which instance the stream publishes, and how many times, has no Rust
//!   counterpart;
//! - `BordersChanged` as what `PhysicalMonitor` makes of it: the monitor, hence
//!   the layout, marked unsaved (`Layout::saved`), watched from a saved state.
//!
//! Not ported, nothing observable being left of them in Rust:
//! - `LateSubscribersReceiveTheCurrentSizeRatherThanWaitingForTheNextSwitch`
//!   pins `Replay(1)`: a geometry chain subscribing after the stream connected
//!   must not miss the current size. Rust reads the size when it needs it.
//! - `DisposingStopsTheMirrorAndTheEditReports`: a Rust monitor has no
//!   subscription to dispose of; the mirror is the layout editing the monitors it
//!   holds.

mod common;

use common::design_options;
use lbm_layout::geo::Thickness;
use lbm_layout::model::{
    DisplaySize, Layout, LayoutOptions, Monitor, MonitorModel, PER_MODEL, PER_MONITOR,
};

const MODEL: &str = "MODEL";
const MONITOR: &str = "MONITOR";

/// `ModelSize()`: 600 x 340 mm, borders 10, 11, 12, 13 (left, top, right, bottom).
fn model_size(code: &str) -> MonitorModel {
    let mut model = MonitorModel::new(code);
    let size = &mut model.physical_size;
    size.set_width(600.0);
    size.set_height(340.0);
    size.set_left_border(10.0);
    size.set_top_border(11.0);
    size.set_right_border(12.0);
    size.set_bottom_border(13.0);
    model
}

/// `Build(mode)`: the model size, "Border values" set to `mode`, and the policy
/// — here, a monitor of that model, whose borders start as a copy of its size's.
fn build(mode: &str) -> Layout {
    let mut layout = Layout::new(LayoutOptions {
        border_values: mode.to_owned(),
        ..design_options()
    });
    let model = layout.get_or_add_model(MODEL, model_size).clone();
    layout.add_or_update_monitor(Monitor::new(MONITOR, &model));
    layout
}

fn monitor(layout: &Layout) -> &Monitor {
    layout.monitor(MONITOR).unwrap()
}

/// `policy.Borders`.
fn borders(layout: &Layout) -> Thickness {
    monitor(layout).borders()
}

/// `policy.Customized`.
fn customized(layout: &Layout) -> bool {
    monitor(layout).borders_customized()
}

/// What `policy.EffectiveSize` last published.
fn effective(layout: &Layout) -> DisplaySize {
    layout.effective_physical_size(monitor(layout))
}

/// `model`, the shared size.
fn model(layout: &Layout) -> DisplaySize {
    layout.model(MODEL).unwrap().physical_size.as_display_size()
}

/// `options.SetBorderValues(value)`.
fn set_border_values(layout: &mut Layout, value: &str) {
    layout.edit_options(|o| o.border_values = value.to_owned());
}

/// `policy.Borders.<Side> = value`: one side written, the others as they are.
fn set_border(layout: &mut Layout, edit: impl FnOnce(&mut Thickness)) {
    let mut b = borders(layout);
    edit(&mut b);
    layout.set_monitor_borders(MONITOR, b);
}

/// `model.LeftBorder = value`.
fn set_model_left_border(layout: &mut Layout, value: f64) {
    layout.edit_model(MODEL, |size, _| {
        size.set_left_border(value);
    });
}

/// C#: `MonitorBorderPolicyTests.BordersAreSeededFromTheModelSoPerMonitorStartsMatchingPerModel`.
#[test]
fn borders_are_seeded_from_the_model_so_per_monitor_starts_matching_per_model() {
    let layout = build(PER_MODEL);
    let model = model(&layout);

    assert_eq!(model.left_border, borders(&layout).left);
    assert_eq!(model.top_border, borders(&layout).top);
    assert_eq!(model.right_border, borders(&layout).right);
    assert_eq!(model.bottom_border, borders(&layout).bottom);
    assert!(!customized(&layout));
}

/// C#: `MonitorBorderPolicyTests.EffectiveSizeIsTheModelSizeUntilTheModeSwitchesToPerMonitor`.
#[test]
fn effective_size_is_the_model_size_until_the_mode_switches_to_per_monitor() {
    let mut layout = build(PER_MODEL);

    // `Assert.Same(model, Assert.Single(sizes))`.
    assert_eq!(effective(&layout), model(&layout));

    set_border_values(&mut layout, PER_MONITOR);
    // The second size: the model's dimensions under this monitor's own borders.
    // Those still mirror the model's, so it reads like the model size — C#'s
    // `NotSame(model, sizes[1])` is an identity no value can show.
    assert_eq!(
        effective(&layout),
        model(&layout).with_borders(borders(&layout))
    );

    set_border_values(&mut layout, PER_MODEL);
    assert_eq!(effective(&layout), model(&layout));

    // Back to PerMonitor, C# must reuse the very same override instance: the
    // geometry chain carries layout-computed positions on it. In Rust the position
    // lives on the monitor, and the size is again the monitor's borders over the
    // model's dimensions.
    set_border_values(&mut layout, PER_MONITOR);
    assert_eq!(
        effective(&layout),
        model(&layout).with_borders(borders(&layout))
    );
}

/// C#: `MonitorBorderPolicyTests.RepeatedModeWritesOfTheSameValueDoNotRepublishTheSize`.
#[test]
fn repeated_mode_writes_of_the_same_value_do_not_republish_the_size() {
    let mut layout = build(PER_MODEL);
    layout.mark_saved();
    let before = effective(&layout);

    set_border_values(&mut layout, PER_MODEL);
    set_border_values(&mut layout, PER_MODEL);

    // `Assert.Single(sizes)`. A real switch republishes every monitor's geometry on
    // the other size and leaves the layout unsaved; writing the value it already
    // has does neither.
    assert!(layout.saved());
    assert_eq!(effective(&layout), before);
}

/// C#: `MonitorBorderPolicyTests.ThePerMonitorSizeKeepsTheModelDimensionsButSubstitutesTheMonitorBorders`.
#[test]
fn the_per_monitor_size_keeps_the_model_dimensions_but_substitutes_the_monitor_borders() {
    let mut layout = build(PER_MONITOR);

    set_border(&mut layout, |b| b.left = 40.0);

    // C# also asserts the stream published no second size (the override is
    // edited in place); a Rust size is read on demand, there is no stream.
    let effective = effective(&layout);
    let model = model(&layout);
    assert_eq!(model.width, effective.width);
    assert_eq!(model.height, effective.height);
    assert_eq!(40.0, effective.left_border);
    assert_eq!(model.top_border, effective.top_border);
}

/// C#: `MonitorBorderPolicyTests.BordersMirrorTheModelWhileTheMonitorDoesNotOwnThem`.
#[test]
fn borders_mirror_the_model_while_the_monitor_does_not_own_them() {
    let mut layout = build(PER_MODEL);

    set_model_left_border(&mut layout, 25.0);

    assert_eq!(25.0, borders(&layout).left);
    assert!(!customized(&layout));
}

/// C#: `MonitorBorderPolicyTests.TheMirrorStopsOnceTheMonitorOwnsItsBorders`.
#[test]
fn the_mirror_stops_once_the_monitor_owns_its_borders() {
    let mut layout = build(PER_MODEL);

    layout.set_borders_customized(MONITOR, true);
    set_model_left_border(&mut layout, 25.0);

    assert_eq!(10.0, borders(&layout).left);
}

/// C#: `MonitorBorderPolicyTests.AMirrorWriteIsNeverAUserEditEvenWhileAlreadyInPerMonitorMode`.
#[test]
fn a_mirror_write_is_never_a_user_edit_even_while_already_in_per_monitor_mode() {
    // The Load path applies model borders with the mode already on PerMonitor.
    // Were the copy taken for a user edit, it would mark the monitor customized
    // and cut the mirror mid-write, freezing the remaining sides at their seeded
    // values.
    let mut layout = build(PER_MONITOR);

    set_model_left_border(&mut layout, 25.0);
    layout.edit_model(MODEL, |size, _| {
        size.set_bottom_border(26.0);
    });

    assert!(!customized(&layout));
    assert_eq!(25.0, borders(&layout).left);
    assert_eq!(26.0, borders(&layout).bottom);
}

/// C#: `MonitorBorderPolicyTests.EditingABorderInPerMonitorModeMakesTheMonitorOwnThem`.
#[test]
fn editing_a_border_in_per_monitor_mode_makes_the_monitor_own_them() {
    let mut layout = build(PER_MONITOR);

    set_border(&mut layout, |b| b.left = 40.0);
    assert!(customized(&layout));

    // Ownership taken: the model no longer drives this monitor.
    set_model_left_border(&mut layout, 25.0);
    assert_eq!(40.0, borders(&layout).left);
}

/// C#: `MonitorBorderPolicyTests.EditingABorderInPerModelModeDoesNotMakeTheMonitorOwnThem`.
#[test]
fn editing_a_border_in_per_model_mode_does_not_make_the_monitor_own_them() {
    // PerModel edits land on the shared model, not here; a write reaching the
    // monitor's borders in that mode is the store or a stray caller, and must not
    // silently take ownership.
    let mut layout = build(PER_MODEL);

    set_border(&mut layout, |b| b.left = 40.0);

    assert!(!customized(&layout));
}

/// C#: `MonitorBorderPolicyTests.CustomizedRaisesAChangeNotificationWhenTheEditIsDetected`.
#[test]
fn customized_raises_a_change_notification_when_the_edit_is_detected() {
    // What the notification announces, and a bound editor re-reads: the flag
    // changed with the edit.
    let mut layout = build(PER_MONITOR);
    assert!(!customized(&layout));

    set_border(&mut layout, |b| b.left = 40.0);

    assert!(customized(&layout));
}

/// C#: `MonitorBorderPolicyTests.BordersChangedReportsEveryEditPastTheSeedingIncludingMirrorWrites`.
#[test]
fn borders_changed_reports_every_edit_past_the_seeding_including_mirror_writes() {
    let mut layout = build(PER_MODEL);
    layout.mark_saved();

    // C# first asserts that the seeding was not reported. The Rust seeding is
    // `Monitor::new` copying the model's borders before the monitor joins a
    // layout: nothing is there to report it, and nothing to observe.

    // A mirror write is not a user edit, but it does change what a save would
    // write out. (The model's own border change marks the layout unsaved as well:
    // one flag cannot tell the two reports apart.)
    set_model_left_border(&mut layout, 25.0);
    assert!(!layout.saved());

    // The mode switch leaves the layout unsaved on its own account: watch the
    // second edit from a saved state again.
    set_border_values(&mut layout, PER_MONITOR);
    layout.mark_saved();
    set_border(&mut layout, |b| b.top = 30.0);
    assert!(!layout.saved());
}

/// C#: `MonitorBorderPolicyTests.WritingTheSameBorderValueReportsNothing`.
#[test]
fn writing_the_same_border_value_reports_nothing() {
    let mut layout = build(PER_MODEL);
    layout.mark_saved();

    let left = borders(&layout).left;
    set_border(&mut layout, |b| b.left = left);

    assert!(layout.saved());
}
