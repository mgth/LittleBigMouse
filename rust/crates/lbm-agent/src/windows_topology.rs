//! Bridge from an edited layout to stable CCD target positions. No platform I/O here.
use lbm_display::windows::topology::placement::Wanted;
use lbm_layout::model::Layout;
use lbm_store::LayoutDocument;

pub(crate) fn prepare(
    layout: &Layout,
    document: &LayoutDocument,
    adjust_scale: bool,
) -> Result<(Layout, Vec<Wanted>), String> {
    if layout.is_virtual() {
        return Err("A foreign layout cannot change Windows displays.".into());
    }
    if adjust_scale {
        return Err(
            "Scale adjustment is not supported on Windows; disable it before applying.".into(),
        );
    }
    let mut edited = layout.clone();
    document.apply(&mut edited);
    // Stop may have happened after the frontend read its document. Applying positions
    // must not resurrect that old Enabled flag when the saved layout is rebuilt.
    edited.options.enabled = layout.options.enabled;
    let wanted = edited
        .compute_pixel_locations_from_physical(false)
        .into_iter()
        .map(|(id, placement)| {
            let source = &layout
                .source(&id)
                .ok_or("A display source disappeared.")?
                .source;
            Ok(Wanted {
                connector: source
                    .interface_path
                    .clone()
                    .filter(|s| !s.is_empty())
                    .ok_or("A Windows monitor has no stable interface path.")?,
                previous: source.in_pixel.bounds(),
                x: placement.pixel_bounds.left(),
                y: placement.pixel_bounds.top(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if wanted.is_empty() {
        return Err("No attached Windows screen to place.".into());
    }
    Ok((edited, wanted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_layout::{
        geo::Point,
        linux::{add_monitor, LinuxMonitor},
        model::LayoutOptions,
    };

    fn layout() -> Layout {
        let mut layout = Layout::new(LayoutOptions::default());
        for (connector, x, primary) in [("PANEL1", 0., true), ("PANEL2", 1920., false)] {
            add_monitor(
                &mut layout,
                &LinuxMonitor {
                    connector_name: connector.into(),
                    logical_x: x,
                    primary,
                    ..LinuxMonitor::fallback()
                },
            );
        }
        layout.set_locations_from_system_configuration(true);
        layout
    }

    #[test]
    fn physical_edits_use_the_solver_but_keep_original_detection_bounds_and_identity() {
        let original = layout();
        let before = serde_json::to_string(&LayoutDocument::of(&original)).unwrap();
        let mut edited = original.clone();
        edited.set_location("PANEL2", Point::new(-600., 0.));
        let (_, wanted) = prepare(&original, &LayoutDocument::of(&edited), false).unwrap();
        let second = wanted.iter().find(|w| w.connector == "PANEL2").unwrap();
        assert_eq!(second.previous.left(), 1920.);
        assert!(second.x < 0.);
        assert_eq!(wanted.len(), 2);
        assert_eq!(
            serde_json::to_string(&LayoutDocument::of(&original)).unwrap(),
            before
        );
        let expected = edited.compute_pixel_locations_from_physical(false);
        for (id, placement) in expected {
            let connector = original
                .source(&id)
                .unwrap()
                .source
                .interface_path
                .as_ref()
                .unwrap();
            let actual = wanted.iter().find(|w| &w.connector == connector).unwrap();
            assert_eq!(
                (actual.x, actual.y),
                (placement.pixel_bounds.left(), placement.pixel_bounds.top())
            );
        }
    }

    #[test]
    fn unsupported_scaling_and_empty_layouts_are_rejected_before_platform_access() {
        let layout = layout();
        assert!(prepare(&layout, &LayoutDocument::of(&layout), true)
            .unwrap_err()
            .contains("Scale"));
        let empty = Layout::new(LayoutOptions::default());
        assert!(prepare(&empty, &LayoutDocument::of(&empty), false).is_err());
    }

    #[test]
    fn applying_an_old_document_keeps_the_engine_stopped_and_refuses_foreign_layouts() {
        let mut layout = layout();
        layout.options.enabled = true;
        let old = LayoutDocument::of(&layout);
        layout.options.enabled = false;
        let (edited, _) = prepare(&layout, &old, false).unwrap();
        assert!(!edited.options.enabled);
        layout.source_kind = lbm_layout::model::LayoutSource::VirtualImport;
        assert!(prepare(&layout, &old, false)
            .unwrap_err()
            .contains("foreign"));
    }
}
