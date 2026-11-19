use littlebigmouse_hook::zones::ZonesLayout;

#[test]
fn touch_separation_is_opt_in_on_the_wire() {
    for (attribute, expected) in [
        ("", false),
        ("TouchMouseIndependent=\"False\"", false),
        ("TouchMouseIndependent=\"True\"", true),
    ] {
        let layout = ZonesLayout::from_xml(&format!("<ZonesLayout {attribute}/>")).unwrap();
        assert_eq!(layout.touch_mouse_independent, expected);
        let pen_attribute = attribute.replace("TouchMouseIndependent", "StylusMouseIndependent");
        let pen_layout = littlebigmouse_hook::zones::ZonesLayout::from_xml(&format!(
            "<ZonesLayout {pen_attribute} />"
        ))
        .unwrap();
        assert_eq!(pen_layout.stylus_mouse_independent, expected);
        let focus_attribute = attribute.replace("TouchMouseIndependent", "RestoreKeyboardFocus");
        let layout = ZonesLayout::from_xml(&format!("<ZonesLayout {focus_attribute}/>")).unwrap();
        assert_eq!(layout.restore_keyboard_focus, expected);
    }
}

#[test]
fn old_layout_defaults_and_invalid_delay() {
    let old = ZonesLayout::from_xml("<ZonesLayout/>").unwrap();
    assert_eq!(old.focus_restore_delay, 120);
    assert!(!old.focus_restore_on_mouse_move);
    assert!(old.touch_all_displays);
    assert_eq!(old.touch_override_modifier, "None");
    for (value, expected) in [
        ("-1", 0),
        ("0", 0),
        ("275", 275),
        ("9000", 5000),
        ("oops", 120),
    ] {
        let layout =
            ZonesLayout::from_xml(&format!("<ZonesLayout FocusRestoreDelay=\"{value}\"/>"))
                .unwrap();
        assert_eq!(layout.focus_restore_delay, expected);
    }
}

#[test]
fn selected_display_and_app_rules_reach_runtime_together() {
    use littlebigmouse_hook::hook::touch_policy::{Modifier, TouchPolicy};
    let layout = ZonesLayout::from_xml(
        r#"<ZonesLayout FocusRestoreDelay="320"
        FocusRestoreOnMouseMove="True" TouchAllDisplays="False"
        TouchDisplayIds="panel" TouchDisplayBounds="-1920,0,1920,1080"
        TouchOverrideModifier="Shift" FocusKeepApps="editor.exe"
        FocusRestoreApps="control.exe;editor.exe"/>"#,
    )
    .unwrap();
    let p = TouchPolicy::from_layout(&layout);
    assert_eq!(p.delay_ms, 320);
    assert!(p.on_mouse_move);
    assert_eq!(p.modifier, Modifier::Shift);
    assert!(p.includes(-1, 100));
    assert!(!p.includes(1, 100));
    assert!(p.restores_app(Some(r"C:\Apps\CONTROL.EXE")));
    assert!(!p.restores_app(Some("editor.exe")));
}
