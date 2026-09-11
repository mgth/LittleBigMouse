//! `WindowsLayoutMappingTests.cs`: a Win32 monitor snapshot mapped onto the model —
//! the physical size inference (the EDID / GDI / DPI fallback chain), the orientation
//! inference and the per-source field copy — one test per C# test. Then the builder
//! the C# suite does not reach: ids, clones, specialized monitors, the layout id.

mod common;

use common::assert_equal_precision;
use lbm_layout::geo::{Point, Size, Vector};
use lbm_layout::model::{DpiAwareness, Layout, LayoutOptions};
use lbm_layout::windows::{
    add_or_update_monitor_device, create_display_source, create_monitor_model,
    create_physical_monitor, infer_orientation, is_aspect_consistent, ordinal_ignore_case_compare,
    physical_size_in_mm, pnp_name_cleanup, populate, WindowsAdapter, WindowsConnection,
    WindowsDeviceCaps, WindowsDisplayMode, WindowsEdid, WindowsMonitor,
};

/// The C# test's `Snapshot(...)` arguments, with its defaults.
struct Snapshot {
    edid: Option<WindowsEdid>,
    pels: Size,
    orientation: i32,
    gdi_size_mm: Size,
    resolution: Size,
    log_pixels: Size,
    primary: bool,
    device_string: &'static str,
    monitor_device_string: &'static str,
    attached_to_desktop: bool,
}

impl Snapshot {
    fn new(
        edid: Option<WindowsEdid>,
        pels: Size,
        orientation: i32,
        gdi_size_mm: Size,
        resolution: Size,
        log_pixels: Size,
    ) -> Self {
        Self {
            edid,
            pels,
            orientation,
            gdi_size_mm,
            resolution,
            log_pixels,
            primary: false,
            device_string: "NVIDIA GeForce RTX 3080",
            monitor_device_string: "Generic PnP Monitor",
            attached_to_desktop: true,
        }
    }

    /// A monitor the way the enumeration assembles it, its active connection on
    /// `\\.\DISPLAY1`. Only the fields the mapping reads are set; the DPIs stay at
    /// C#'s `default(Vector)`.
    fn build(self) -> WindowsMonitor {
        let mode = WindowsDisplayMode {
            position: Point::new(0.0, 0.0),
            pels: self.pels,
            display_orientation: self.orientation,
            display_frequency: 60,
        };
        let adapter = WindowsAdapter {
            device_name: r"\\.\DISPLAY1".to_owned(),
            device_string: self.device_string.to_owned(),
            primary: self.primary,
            effective_dpi: Vector::default(),
            angular_dpi: Vector::default(),
            raw_dpi: Vector::default(),
            current_mode: Some(mode),
            capabilities: WindowsDeviceCaps {
                size: self.gdi_size_mm,
                resolution: self.resolution,
                log_pixels: self.log_pixels,
            },
        };
        WindowsMonitor {
            id: "MON1".to_owned(),
            pnp_code: if self.edid.is_some() {
                "SAM1234"
            } else {
                "GEN"
            }
            .to_owned(),
            source_id: "SRC1".to_owned(),
            interface_path: String::new(),
            monitor_number: String::new(),
            is_specialized: false,
            edid: self.edid,
            active_connection: Some(WindowsConnection {
                device_name: r"\\.\DISPLAY1\Monitor0".to_owned(),
                device_string: self.monitor_device_string.to_owned(),
                attached_to_desktop: self.attached_to_desktop,
                adapter,
            }),
        }
    }
}

/// The C# test's `Edid(w, h, model)`: SAM, product 1234, serial "S/N: S1", DVI.
fn edid(physical_width: f64, physical_height: f64) -> WindowsEdid {
    edid_model(physical_width, physical_height, "S24D300")
}

fn edid_model(physical_width: f64, physical_height: f64, model: &str) -> WindowsEdid {
    WindowsEdid {
        manufacturer_code: Some("SAM".to_owned()),
        model: Some(model.to_owned()),
        serial_number: Some("S/N: S1".to_owned()),
        video_interface: Some("Dvi".to_owned()),
        physical_width,
        physical_height,
    }
}

fn size(w: f64, h: f64) -> Size {
    Size::new(w, h)
}

/// A monitor with no connection at all: `ActiveConnection` is null.
fn unconnected(edid: Option<WindowsEdid>) -> WindowsMonitor {
    WindowsMonitor {
        id: "MON1".to_owned(),
        pnp_code: String::new(),
        source_id: "SRC1".to_owned(),
        interface_path: String::new(),
        monitor_number: String::new(),
        is_specialized: false,
        edid,
        active_connection: None,
    }
}

fn mode(pels: Size, orientation: i32) -> WindowsDisplayMode {
    WindowsDisplayMode {
        position: Point::new(0.0, 0.0),
        pels,
        display_orientation: orientation,
        display_frequency: 0,
    }
}

// ---- Physical size inference -------------------------------------------------

/// C#: `WindowsLayoutMappingTests.LandscapeWithConsistentGdi_UsesGdiSize`
#[test]
fn landscape_with_consistent_gdi_uses_gdi_size() {
    // 16:9 panel, GDI mm already consistent with the pixel aspect: taken as-is.
    let monitor = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(2560.0, 1440.0),
        0,
        size(598.0, 336.0),
        size(2560.0, 1440.0),
        size(96.0, 96.0),
    )
    .build();

    assert_eq!(physical_size_in_mm(&monitor), (598.0, 336.0));
}

/// C#: `WindowsLayoutMappingTests.RotatedGdi_IsNormalizedBackToIntrinsicOrientation`
#[test]
fn rotated_gdi_is_normalized_back_to_intrinsic_orientation() {
    // The driver reports the GDI size transposed (portrait) while the panel is 16:9:
    // swapped back to the intrinsic (landscape) orientation.
    let monitor = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(1440.0, 2560.0),
        1,
        size(336.0, 598.0),
        size(1440.0, 2560.0),
        size(96.0, 96.0),
    )
    .build();

    assert_eq!(physical_size_in_mm(&monitor), (598.0, 336.0));
}

/// C#: `WindowsLayoutMappingTests.SquareGdiPlaceholderWithEdid_FallsBackToEdidSize`
#[test]
fn square_gdi_placeholder_with_edid_falls_back_to_edid_size() {
    // A bogus 1000 x 1000 GDI size fails the aspect test both ways; the EDID wins.
    let monitor = Snapshot::new(
        Some(edid(600.0, 340.0)),
        size(1920.0, 1080.0),
        0,
        size(1000.0, 1000.0),
        size(1920.0, 1080.0),
        size(96.0, 96.0),
    )
    .build();

    assert_eq!(physical_size_in_mm(&monitor), (600.0, 340.0));
}

/// C#: `WindowsLayoutMappingTests.SquareGdiPlaceholderNoEdid_EstimatesFromResolutionAndDpi`
#[test]
fn square_gdi_placeholder_no_edid_estimates_from_resolution_and_dpi() {
    // 1920 px / 96 dpi * 25.4 = 508 mm, 1080 / 96 * 25.4 = 285.75 mm.
    let monitor = Snapshot::new(
        None,
        size(1920.0, 1080.0),
        0,
        size(1000.0, 1000.0),
        size(1920.0, 1080.0),
        size(96.0, 96.0),
    )
    .build();

    let (w, h) = physical_size_in_mm(&monitor);
    assert_equal_precision(1920.0 / 96.0 * 25.4, w, 3);
    assert_equal_precision(1080.0 / 96.0 * 25.4, h, 3);
}

/// C#: `WindowsLayoutMappingTests.NoCurrentMode_WithEdid_UsesEdidSize`
#[test]
fn no_current_mode_with_edid_uses_edid_size() {
    // No connection, so no active one, so no current mode: the EDID decides.
    let monitor = unconnected(Some(edid(510.0, 287.0)));

    assert_eq!(physical_size_in_mm(&monitor), (510.0, 287.0));
}

/// C#: `WindowsLayoutMappingTests.NoCurrentMode_NoEdid_ReturnsZero`
#[test]
fn no_current_mode_no_edid_returns_zero() {
    assert_eq!(physical_size_in_mm(&unconnected(None)), (0.0, 0.0));
}

/// C#: `WindowsLayoutMappingTests.ZeroSizedEdid_IsTreatedAsAbsent`
#[test]
fn zero_sized_edid_is_treated_as_absent() {
    // An EDID with no size is no reference: estimated from resolution / DPI.
    let monitor = Snapshot::new(
        Some(edid(0.0, 0.0)),
        size(1920.0, 1080.0),
        0,
        size(1000.0, 1000.0),
        size(1920.0, 1080.0),
        size(96.0, 96.0),
    )
    .build();

    let (w, h) = physical_size_in_mm(&monitor);
    assert_equal_precision(1920.0 / 96.0 * 25.4, w, 3);
    assert_equal_precision(1080.0 / 96.0 * 25.4, h, 3);
}

// ---- Orientation inference ---------------------------------------------------

/// C#: `WindowsLayoutMappingTests.ExplicitDevmodeOrientation_IsReturnedAsIs` (the four
/// `InlineData`)
#[test]
fn explicit_devmode_orientation_is_returned_as_is() {
    for orientation in 0..4 {
        let m = mode(size(2560.0, 1440.0), orientation);
        assert_eq!(
            infer_orientation(&m, Some(&edid(597.0, 336.0))),
            orientation
        );
    }
}

/// C#: `WindowsLayoutMappingTests.NoEdid_DefaultOrientation_StaysZero`
#[test]
fn no_edid_default_orientation_stays_zero() {
    assert_eq!(infer_orientation(&mode(size(1440.0, 2560.0), 0), None), 0);
}

/// C#: `WindowsLayoutMappingTests.DriverRotatedBelowWindows_PixelPortraitLandscapeEdid_InfersRotation`
#[test]
fn driver_rotated_below_windows_pixel_portrait_landscape_edid_infers_rotation() {
    // DEVMODE says 0 but the pixels are portrait and the panel landscape (#507).
    let m = mode(size(1440.0, 2560.0), 0);
    assert_eq!(infer_orientation(&m, Some(&edid(597.0, 336.0))), 1);
}

/// C#: `WindowsLayoutMappingTests.PixelAndPanelAgree_NoInferredRotation`
#[test]
fn pixel_and_panel_agree_no_inferred_rotation() {
    let m = mode(size(2560.0, 1440.0), 0);
    assert_eq!(infer_orientation(&m, Some(&edid(597.0, 336.0))), 0);
}

/// C#: `WindowsLayoutMappingTests.SquarePixels_DecideNothing`
#[test]
fn square_pixels_decide_nothing() {
    let m = mode(size(1080.0, 1080.0), 0);
    assert_eq!(infer_orientation(&m, Some(&edid(597.0, 336.0))), 0);
}

/// C#: `WindowsLayoutMappingTests.SquareEdid_DecidesNothing`
#[test]
fn square_edid_decides_nothing() {
    let m = mode(size(1440.0, 2560.0), 0);
    assert_eq!(infer_orientation(&m, Some(&edid(500.0, 500.0))), 0);
}

// ---- Aspect consistency ------------------------------------------------------

/// C#: `WindowsLayoutMappingTests.AspectConsistency_MatchingAspectIsTrue`
#[test]
fn aspect_consistency_matching_aspect_is_true() {
    assert!(is_aspect_consistent(598.0, 336.0, 2560.0, 1440.0));
}

/// C#: `WindowsLayoutMappingTests.AspectConsistency_SquarePlaceholderAgainstWideResolutionIsFalse`
#[test]
fn aspect_consistency_square_placeholder_against_wide_resolution_is_false() {
    assert!(!is_aspect_consistent(1000.0, 1000.0, 1920.0, 1080.0));
}

/// C#: `WindowsLayoutMappingTests.AspectConsistency_NonPositiveInputsAreFalse` (the four
/// `InlineData`)
#[test]
fn aspect_consistency_non_positive_inputs_are_false() {
    for (w, h, pw, ph) in [
        (0.0, 336.0, 2560.0, 1440.0),
        (598.0, 0.0, 2560.0, 1440.0),
        (598.0, 336.0, 0.0, 1440.0),
        (598.0, 336.0, 2560.0, 0.0),
    ] {
        assert!(!is_aspect_consistent(w, h, pw, ph), "{w} {h} {pw} {ph}");
    }
}

// ---- Source field mapping ----------------------------------------------------

/// C#: `WindowsLayoutMappingTests.SourceMapping_CopiesIdentityPrimaryAndPixelRect`
#[test]
fn source_mapping_copies_identity_primary_and_pixel_rect() {
    let mut snapshot = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(2560.0, 1440.0),
        0,
        size(598.0, 336.0),
        size(2560.0, 1440.0),
        size(96.0, 96.0),
    );
    snapshot.primary = true;
    let mut monitor = snapshot.build();
    monitor.monitor_number = "1".to_owned();

    let source = create_display_source(&monitor);

    assert!(source.primary);
    assert!(source.attached_to_desktop);
    assert_eq!(source.display_frequency, 60);
    assert_eq!(source.orientation, 0);
    assert_eq!(source.in_pixel.width, 2560.0);
    assert_eq!(source.in_pixel.height, 1440.0);
    assert_eq!(source.source_number.as_deref(), Some("1"));
    assert_eq!(
        source.device_name.as_deref(),
        Some(r"\\.\DISPLAY1\Monitor0")
    );
    assert_eq!(source.display_name.as_deref(), Some(r"\\.\DISPLAY1"));
}

/// C#: `WindowsLayoutMappingTests.SourceMapping_InfersRotationForDriverRotatedPanel`
#[test]
fn source_mapping_infers_rotation_for_driver_rotated_panel() {
    let monitor = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(1440.0, 2560.0),
        0, // DEVMODE default, driver rotated below Windows
        size(336.0, 598.0),
        size(1440.0, 2560.0),
        size(96.0, 96.0),
    )
    .build();

    assert_eq!(create_display_source(&monitor).orientation, 1);
}

/// C#: `WindowsLayoutMappingTests.SourceMapping_NoCurrentMode_ClearsFrequencyAndZeroesRect`
#[test]
fn source_mapping_no_current_mode_clears_frequency_and_zeroes_rect() {
    let monitor = unconnected(Some(edid(597.0, 336.0)));

    let source = create_display_source(&monitor);

    assert_eq!(source.display_frequency, 0);
    // Beyond the C# assertion: without a connection C# returns right after the
    // interface path, leaving the rectangle at its empty default.
    assert_eq!((source.in_pixel.width, source.in_pixel.height), (0.0, 0.0));
    assert_eq!(source.source_number, None);
}

// ---- Model mapping -----------------------------------------------------------

/// C#: `WindowsLayoutMappingTests.ModelMapping_KeepsIntrinsicSizeForDriverRotatedPortrait`
#[test]
fn model_mapping_keeps_intrinsic_size_for_driver_rotated_portrait() {
    // Portrait pixels, landscape EDID panel: the shared model keeps the intrinsic
    // (landscape) size (#507).
    let monitor = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(1440.0, 2560.0),
        1,
        size(336.0, 598.0),
        size(1440.0, 2560.0),
        size(96.0, 96.0),
    )
    .build();

    let model = create_monitor_model(&monitor, "SAM1234");

    assert_eq!(model.physical_size.width(), 598.0);
    assert_eq!(model.physical_size.height(), 336.0);
}

/// C#: `WindowsLayoutMappingTests.ModelMapping_GenericPnpNameWithEdidModel_UsesEdidModel`
#[test]
fn model_mapping_generic_pnp_name_with_edid_model_uses_edid_model() {
    let mut snapshot = Snapshot::new(
        Some(edid_model(597.0, 336.0, "MyPanel")),
        size(2560.0, 1440.0),
        0,
        size(598.0, 336.0),
        size(2560.0, 1440.0),
        size(96.0, 96.0),
    );
    snapshot.monitor_device_string = "Generic PnP Monitor";
    let monitor = snapshot.build();

    let model = create_monitor_model(&monitor, "SAM1234");

    assert_eq!(model.pnp_device_name.as_deref(), Some("MyPanel"));
}

/// C#: `WindowsLayoutMappingTests.PhysicalMonitorMapping_CopiesDeviceIdAndSerial`
#[test]
fn physical_monitor_mapping_copies_device_id_and_serial() {
    let monitor = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(2560.0, 1440.0),
        0,
        size(598.0, 336.0),
        size(2560.0, 1440.0),
        size(96.0, 96.0),
    )
    .build();

    let model = create_monitor_model(&monitor, "SAM1234");
    let physical = create_physical_monitor(&monitor, "SRC1", &model);

    assert_eq!(physical.device_id.as_deref(), Some("MON1"));
    assert_eq!(physical.serial_number.as_deref(), Some("S/N: S1"));
}

/// C#: `WindowsLayoutMappingTests.PhysicalMonitorMapping_NoEdid_SerialFallsBackToNA`
#[test]
fn physical_monitor_mapping_no_edid_serial_falls_back_to_na() {
    let monitor = Snapshot::new(
        None,
        size(1920.0, 1080.0),
        0,
        size(509.0, 286.0),
        size(1920.0, 1080.0),
        size(96.0, 96.0),
    )
    .build();

    let model = create_monitor_model(&monitor, "GEN");
    let physical = create_physical_monitor(&monitor, "SRC1", &model);

    assert_eq!(physical.serial_number.as_deref(), Some("N/A"));
}

// ---- Beyond the C# suite: the rest of the mapping ------------------------------

/// A monitor of the enumeration: an EDID'd Samsung on `\\.\DISPLAY<n>`, 2560 x 1440 at
/// `x`, 144 dpi effective.
fn monitor_at(n: u32, source_id: &str, x: f64, primary: bool) -> WindowsMonitor {
    let mut snapshot = Snapshot::new(
        Some(edid(597.0, 336.0)),
        size(2560.0, 1440.0),
        0,
        size(598.0, 336.0),
        size(2560.0, 1440.0),
        size(144.0, 144.0),
    );
    snapshot.primary = primary;
    let mut monitor = snapshot.build();
    monitor.id = format!(r"MONITOR\SAM1234\{{4d36e96e-e325-11ce-bfc1-08002be10318}}\000{n}");
    monitor.source_id = source_id.to_owned();
    monitor.monitor_number = n.to_string();
    monitor.interface_path = format!(r"\\?\DISPLAY#SAM1234#{n}");
    let connection = monitor.active_connection.as_mut().unwrap();
    connection.device_name = format!(r"\\.\DISPLAY{n}\Monitor0");
    connection.adapter.device_name = format!(r"\\.\DISPLAY{n}");
    connection.adapter.effective_dpi = Vector::new(144.0, 144.0);
    connection.adapter.current_mode.as_mut().unwrap().position = Point::new(x, 0.0);
    monitor
}

#[test]
fn a_monitor_becomes_a_model_a_monitor_and_its_active_source() {
    let mut layout = Layout::new(LayoutOptions::default());
    let monitor = monitor_at(1, "SAM1234S/N: S1_00_0000_00", 0.0, true);
    add_or_update_monitor_device(&mut layout, &monitor);

    let model = layout.model("SAM1234").unwrap();
    assert_eq!(model.pnp_device_name.as_deref(), Some("S24D300"));
    assert_eq!(model.logo.as_deref(), Some("icon/Pnp/SAM?icon/Pnp/LBM"));
    assert_eq!(
        (model.physical_size.width(), model.physical_size.height()),
        (598.0, 336.0)
    );

    let physical = layout.monitor("SAM1234S/N: S1_00_0000_00").unwrap();
    assert_eq!(physical.device_id.as_deref(), Some(monitor.id.as_str()));
    assert_eq!(physical.sources, vec![monitor.source_id.clone()]);
    assert_eq!(
        physical.active_source.as_deref(),
        Some(monitor.source_id.as_str())
    );

    let source = layout.source(&monitor.source_id).unwrap();
    assert_eq!(source.device_id, monitor.id);
    assert_eq!(source.monitor, monitor.source_id);
    let s = &source.source;
    assert_eq!(s.interface_path.as_deref(), Some(r"\\?\DISPLAY#SAM1234#1"));
    assert_eq!(s.source_name.as_deref(), Some(r"Dvi:\\.\DISPLAY1\Monitor0"));
    assert_eq!(s.interface_name.as_deref(), Some("nvidia geforce rtx 3080"));
    assert_eq!(s.interface_logo.as_deref(), Some("icon/pnp/nvidia"));
    assert_eq!((s.effective_dpi.x, s.effective_dpi.y), (144.0, 144.0));
    assert_eq!(
        layout.primary_source().map(|p| p.device_id.as_str()),
        Some(monitor.id.as_str())
    );
}

#[test]
fn monitors_of_one_make_share_the_first_ones_model() {
    let mut layout = Layout::new(LayoutOptions::default());
    let first = monitor_at(1, "A", 0.0, true);
    let mut second = monitor_at(2, "B", 2560.0, false);
    // A different GDI size and name for the same PnP code: the model keeps the first's.
    let adapter = &mut second.active_connection.as_mut().unwrap().adapter;
    adapter.capabilities.size = size(600.0, 340.0);
    second.active_connection.as_mut().unwrap().device_string = "Other name".to_owned();
    add_or_update_monitor_device(&mut layout, &first);
    add_or_update_monitor_device(&mut layout, &second);

    assert_eq!(layout.models().len(), 1);
    assert_eq!(layout.models()[0].physical_size.width(), 598.0);
    assert_eq!(
        layout.models()[0].pnp_device_name.as_deref(),
        Some("S24D300")
    );
    assert_eq!(layout.monitors().len(), 2);
}

/// Two monitors the enumeration left with one source id (C#'s duplicate scan only
/// compares neighbours): the second is a second source of the first monitor, and its
/// display source takes the id's place among the layout's sources.
#[test]
fn a_taken_source_id_adds_a_source_to_that_monitor() {
    let mut layout = Layout::new(LayoutOptions::default());
    let first = monitor_at(1, "SAME", 0.0, true);
    let second = monitor_at(2, "SAME", 2560.0, false);
    add_or_update_monitor_device(&mut layout, &first);
    add_or_update_monitor_device(&mut layout, &second);

    assert_eq!(layout.monitors().len(), 1);
    let monitor = layout.monitor("SAME").unwrap();
    assert_eq!(monitor.sources, vec!["SAME".to_owned(), "SAME".to_owned()]);
    assert_eq!(monitor.device_id.as_deref(), Some(first.id.as_str()));
    let sources: Vec<_> = layout.sources().collect();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].device_id, second.id);
    assert_eq!(layout.compute_id(), "SAME");
}

#[test]
fn a_source_already_there_is_refreshed_in_place() {
    let mut layout = Layout::new(LayoutOptions::default());
    let mut monitor = monitor_at(1, "A", 0.0, true);
    add_or_update_monitor_device(&mut layout, &monitor);
    monitor.monitor_number = "7".to_owned();
    monitor.source_id = "ignored: the device id matches".to_owned();
    add_or_update_monitor_device(&mut layout, &monitor);

    assert_eq!(layout.monitors().len(), 1);
    let sources: Vec<_> = layout.sources().collect();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source.id, "A");
    assert_eq!(sources[0].source.source_number.as_deref(), Some("7"));
}

#[test]
fn a_detached_monitor_keeps_its_interface_path_and_an_empty_source() {
    let mut layout = Layout::new(LayoutOptions::default());
    let mut monitor = unconnected(None);
    monitor.interface_path = r"\\?\DISPLAY#GEN#1".to_owned();
    monitor.pnp_code = "GEN".to_owned();
    add_or_update_monitor_device(&mut layout, &monitor);

    let model = layout.model("GEN").unwrap();
    // No connection: an empty name, the LBM logo, no size.
    assert_eq!(model.pnp_device_name.as_deref(), Some(""));
    assert_eq!(model.logo.as_deref(), Some("icon/Pnp/LBM"));
    assert_eq!(model.physical_size.width(), 0.0);
    let source = &layout.source("SRC1").unwrap().source;
    assert_eq!(source.interface_path.as_deref(), Some(r"\\?\DISPLAY#GEN#1"));
    assert_eq!(source.device_name, None);
    assert!(!source.attached_to_desktop);
}

#[test]
fn populate_leaves_specialized_monitors_out_and_anchors_on_the_primary() {
    let mut layout = Layout::new(LayoutOptions::default());
    let left = monitor_at(1, "L", -2560.0, false);
    let primary = monitor_at(2, "P", 0.0, true);
    let mut headset = monitor_at(3, "VR", 2560.0, false);
    headset.is_specialized = true;
    let mut loaded = false;
    populate(
        &mut layout,
        DpiAwareness::SystemAware,
        &[left, primary, headset],
        |_| {
            loaded = true;
            Ok::<(), ()>(())
        },
    )
    .unwrap();

    assert!(loaded);
    assert_eq!(layout.dpi_awareness, DpiAwareness::SystemAware);
    assert_eq!(layout.id, "L+P");
    assert!(layout.monitor("VR").is_none());
    let p = layout
        .depth_projection(layout.monitor("P").unwrap())
        .unwrap();
    let l = layout
        .depth_projection(layout.monitor("L").unwrap())
        .unwrap();
    assert_eq!((p.x, p.y), (0.0, 0.0));
    assert!(
        l.x < 0.0,
        "the left monitor is placed left of the primary: {}",
        l.x
    );
}

#[test]
fn populate_without_monitors_is_an_empty_layout() {
    let mut layout = Layout::new(LayoutOptions::default());
    populate(&mut layout, DpiAwareness::PerMonitorAware, &[], |_| {
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(layout.id, "");
    assert!(layout.monitors().is_empty());
}

#[test]
fn a_failed_load_stops_populate() {
    let mut layout = Layout::new(LayoutOptions::default());
    let monitor = monitor_at(1, "A", 0.0, true);
    let result = populate(
        &mut layout,
        DpiAwareness::PerMonitorAware,
        &[monitor],
        |_| Err("store"),
    );
    assert_eq!(result, Err("store"));
    assert_eq!(layout.id, "A");
}

#[test]
fn the_logo_follows_the_adapter_then_the_edid() {
    let with = |gpu: &'static str, model: &str| {
        let mut snapshot = Snapshot::new(
            Some(edid_model(597.0, 336.0, model)),
            size(2560.0, 1440.0),
            0,
            size(598.0, 336.0),
            size(2560.0, 1440.0),
            size(96.0, 96.0),
        );
        snapshot.device_string = gpu;
        create_monitor_model(&snapshot.build(), "X").logo.unwrap()
    };
    assert_eq!(
        with("spacedesk Graphics Adapter", "x"),
        "icon/Pnp/Spacedesk"
    );
    assert_eq!(
        with("Microsoft Remote Display Adapter", "x"),
        "icon/Pnp/Microsoft"
    );
    assert_eq!(with("NVIDIA", "Aorus FI27Q"), "icon/Pnp/Aorus");
    assert_eq!(with("NVIDIA", "aorus"), "icon/Pnp/SAM?icon/Pnp/LBM");
}

#[test]
fn pnp_names_lose_drivers_and_the_first_parenthesized_part() {
    assert_eq!(pnp_name_cleanup("Dell U2415 Drivers (DP)"), "Dell U2415");
    assert_eq!(
        pnp_name_cleanup("Generic PnP Monitor"),
        "Generic PnP Monitor"
    );
    // Every copy of the first group goes; a later, different one stays.
    assert_eq!(pnp_name_cleanup("A (x) B (x) C (y)"), "A  B  C (y)");
    // The group runs to the first ")" after the first "(".
    assert_eq!(pnp_name_cleanup("A (B (C) D)"), "A  D)");
    assert_eq!(pnp_name_cleanup("A (unclosed"), "A (unclosed");
    assert_eq!(pnp_name_cleanup(" ) ( "), ") (");
}

#[test]
fn ordinal_ignore_case_orders_display_names() {
    use std::cmp::Ordering;
    assert_eq!(
        ordinal_ignore_case_compare(r"\\.\display2", r"\\.\DISPLAY10"),
        Ordering::Greater
    );
    assert_eq!(
        ordinal_ignore_case_compare(r"\\.\Display1", r"\\.\DISPLAY1"),
        Ordering::Equal
    );
}
