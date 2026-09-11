//! The real enumeration, on Windows only: it must go through whatever the machine has —
//! a CI runner may have one basic display or none at all — and what it finds must map.

#![cfg(windows)]

use lbm_display::windows::{
    current_display_signature, discover, display_json, thread_dpi_awareness,
};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_layout::windows::populate;

#[test]
fn the_real_enumeration_goes_through_and_maps() {
    let tree = discover().expect("the enumeration fails only where C# throws");
    let input = tree.layout_input();
    assert_eq!(input.len(), tree.monitors().count());
    for (device, monitor) in tree.monitors().zip(&input) {
        assert!(!monitor.source_id.is_empty());
        assert!(!monitor.monitor_number.is_empty());
        assert!(display_json(&tree, device).is_object());
    }

    let mut layout = Layout::new(LayoutOptions::default());
    populate(&mut layout, thread_dpi_awareness(), &input, |_| {
        Ok::<(), ()>(())
    })
    .unwrap();
    assert!(layout.monitors().len() <= input.len());

    // The cheap signature reads the same system.
    let _ = current_display_signature();
}
