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
    }
}

#[test]
fn an_old_layout_gets_the_defaults() {
    let old = ZonesLayout::from_xml("<ZonesLayout/>").unwrap();
    assert!(old.touch_all_displays);
    assert_eq!(old.touch_override_modifier, "None");
}

#[test]
fn selected_display_and_override_reach_the_runtime() {
    use littlebigmouse_hook::hook::touch_policy::{Modifier, TouchPolicy};
    let layout = ZonesLayout::from_xml(
        r#"<ZonesLayout TouchAllDisplays="False" TouchDisplayIds="panel"
        TouchDisplayBounds="-1920,0,1920,1080" TouchOverrideModifier="Shift"/>"#,
    )
    .unwrap();
    let p = TouchPolicy::from_layout(&layout);
    assert_eq!(p.modifier, Modifier::Shift);
    assert!(p.includes(-1, 100));
    assert!(!p.includes(1, 100));
}
