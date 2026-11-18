use egui::vec2;
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use lbm_ipc::remote::{Action, Brand, Command, Config, State};
use lbm_ui::remote::{Did, Panel};
struct TestState {
    panel: Panel,
    state: State,
    busy: bool,
    commands: Vec<Command>,
}
fn harness(brand: Brand, paired: bool, width: f32) -> Harness<'static, TestState> {
    let config = Config {
        ip_address: "192.168.1.20".into(),
        mac_address: "AA:BB:CC:DD:EE:FF".into(),
        name: brand.label().into(),
        paired,
        ..Default::default()
    };
    let mut state = State::default();
    match brand {
        Brand::Samsung => state.samsung = config,
        Brand::Hisense => state.hisense = config,
    };
    let mut panel = Panel::default();
    panel.brand = brand;
    panel.load(&state);
    Harness::builder()
        .with_size(vec2(width, 1000.))
        .build_ui_state(
            |ui, s| {
                lbm_ui::theme::install(ui.ctx());
                if let Some(action) = s.panel.show(ui, Some(&s.state), s.busy, None) {
                    match action {
                        Did::Command(c) => s.commands.push(c),
                        Did::Brand(b) => {
                            s.panel.brand = b;
                            s.panel.load(&s.state);
                        }
                    }
                }
            },
            TestState {
                panel,
                state,
                busy: false,
                commands: vec![],
            },
        )
}
#[test]
fn navigation_targets_the_selected_brand_without_any_ddc_state() {
    let mut h = harness(Brand::Samsung, true, 300.);
    h.run();
    h.get_by_label("Up").click();
    h.run();
    assert_eq!(
        h.state().commands.last(),
        Some(&Command {
            brand: Brand::Samsung,
            action: Action::Key {
                key: "KEY_UP".into()
            }
        })
    );
    h.get_by_label("Vol +").click();
    h.run();
    assert!(
        matches!(h.state().commands.last(),Some(Command{action:Action::Key{key},..}) if key=="KEY_VOLUP")
    );
}
#[test]
fn unpaired_hisense_offers_pin_setup_and_disables_navigation() {
    let mut h = harness(Brand::Hisense, false, 300.);
    h.run();
    assert!(h.get_by_label("OK").accesskit_node().is_disabled());
    h.get_by_label("Request VIDAA PIN").click();
    h.run();
    assert_eq!(
        h.state().commands.last(),
        Some(&Command {
            brand: Brand::Hisense,
            action: Action::RequestPin
        })
    );
}
#[test]
#[cfg(not(windows))]
fn samsung_remote_snapshot() {
    let mut h = harness(Brand::Samsung, true, 280.);
    h.run();
    h.snapshot("remote-samsung");
}
#[test]
#[cfg(not(windows))]
fn hisense_setup_snapshot() {
    let mut h = harness(Brand::Hisense, false, 280.);
    h.run();
    h.snapshot("remote-hisense-setup");
}
