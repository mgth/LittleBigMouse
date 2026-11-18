//! App preferences must survive stale layout windows and unrelated setting edits.
use lbm_agent::{
    api::Request,
    discovery::Discovery,
    reconcile::World,
    world::{AgentWorld, SystemWorld},
};
use lbm_layout::model::{Layout, LayoutOptions};
use lbm_store::{JsonLayoutStore, LayoutPersistence, LayoutStore, PersistencePlatform};
use serde_json::{json, Value};

struct Platform;
impl PersistencePlatform for Platform {
    fn is_elevated(&self) -> bool {
        false
    }
}
fn world(dir: &std::path::Path) -> SystemWorld<JsonLayoutStore, Platform> {
    let excluded = dir.join("Excluded.txt");
    let mut world = SystemWorld::new(
        Discovery::Fallback,
        LayoutPersistence::with_excluded_list_file(
            JsonLayoutStore::new(dir.to_owned()),
            Platform,
            move || excluded.clone(),
        ),
    );
    world.rebuild_layout();
    world
}
fn request((method, extra): (&str, Value)) -> Request {
    let mut frame = json!({"Id":1,"Method":method});
    frame
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value::<lbm_agent::api::RequestFrame>(frame)
        .unwrap()
        .request
}
fn save_options(world: &mut impl AgentWorld, before: &LayoutOptions, after: &LayoutOptions) {
    let Request::SaveOptions {
        options,
        excluded,
        load_at_startup,
    } = request(lbm_app::settings::save_options(before, after, None))
    else {
        unreachable!()
    };
    world
        .save_options(options.as_ref(), excluded.as_deref(), load_at_startup)
        .unwrap();
}
fn enabled(dir: &std::path::Path, id: &str) -> bool {
    JsonLayoutStore::new(dir.to_owned())
        .read(id, &[])
        .unwrap()
        .global_options
        .unwrap()
        .vcp_control
        .unwrap()
}

#[test]
fn display_lab_survives_stale_layout_saves_unrelated_settings_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut agent = world(dir.path());
    let mut stale = Layout::new(LayoutOptions::default());
    lbm_layout::linux::populate(&mut stale, &[], |_| Ok::<(), std::io::Error>(())).unwrap();
    assert!(!stale.options.vcp_control);
    let defaults = LayoutOptions::default();
    let mut current = defaults.clone();
    current.vcp_control = true;
    save_options(&mut agent, &defaults, &current);
    assert!(enabled(dir.path(), &stale.id));

    // Another window had VCP control disabled when it opened, then moved a monitor.
    let monitor = stale.monitors()[0].id.clone();
    stale.set_location(&monitor, lbm_layout::geo::Point::new(123., 45.));
    let Request::SaveLayout {
        layout_id,
        document,
    } = request(lbm_app::settings::save_layout(&stale))
    else {
        unreachable!()
    };
    agent.edit(&layout_id, &document).unwrap();
    agent.save_layout().unwrap();
    assert!(
        enabled(dir.path(), &stale.id),
        "a layout save overwrote VCP control"
    );

    // The same stale window changes a different preference.
    let mut unrelated = defaults.clone();
    unrelated.debug_tools = true;
    save_options(&mut agent, &defaults, &unrelated);
    assert!(
        enabled(dir.path(), &stale.id),
        "an unrelated preference overwrote VCP control"
    );
    drop(agent);
    let mut restarted = world(dir.path());
    restarted.save_layout().unwrap();
    assert!(
        enabled(dir.path(), &stale.id),
        "VCP control was lost on restart"
    );

    // Intentional disabling must still persist, rather than being forced on.
    current.debug_tools = true;
    let mut disabled = current.clone();
    disabled.vcp_control = false;
    save_options(&mut restarted, &current, &disabled);
    drop(restarted);
    world(dir.path()).save_layout().unwrap();
    assert!(!enabled(dir.path(), &stale.id));
}

#[test]
fn preferences_are_autosaved_and_do_not_turn_layout_save_or_undo_on() {
    let mut layout = Layout::new(LayoutOptions::default());
    let saved = lbm_app::saved::Reference::of(&layout);
    layout.edit_options(|o| o.vcp_control = true);
    assert!(saved.holds(&layout));
    layout.edit_options(|o| o.loop_x = true);
    assert!(
        !saved.holds(&layout),
        "layout options still belong to Save and Undo"
    );
}

#[test]
fn unanswered_preferences_accumulate_and_only_the_latest_answer_clears_them() {
    use lbm_app::settings::{save_options, PendingOptions};
    let base = LayoutOptions::default();
    let mut minimized = base.clone();
    minimized.start_minimized = !base.start_minimized;
    let mut both = minimized.clone();
    both.load_at_startup = !base.load_at_startup;

    let mut pending = PendingOptions::default();
    assert!(pending.is_empty());
    pending.add(save_options(&base, &minimized, None).1);
    pending.add(save_options(&minimized, &both, Some(&["game.exe".to_owned()])).1);
    let (method, request) = pending.request().unwrap();
    assert_eq!(method, "SaveOptions");
    assert_eq!(
        request["Options"].as_object().unwrap().len(),
        1,
        "only what changed may be replayed: {request}"
    );
    assert_eq!(request["Options"]["StartMinimized"], !base.start_minimized);
    assert_eq!(request["LoadAtStartup"], !base.load_at_startup);
    assert_eq!(pending.excluded().unwrap(), ["game.exe"]);

    // Options read again from the store get the unanswered changes back.
    let mut reloaded = base.clone();
    pending.apply(&mut reloaded);
    assert_eq!(reloaded.start_minimized, both.start_minimized);
    assert_eq!(reloaded.load_at_startup, both.load_at_startup);

    pending.sent(7);
    pending.answered(6);
    assert!(!pending.is_empty(), "an older answer says nothing");
    pending.disconnected();
    pending.answered(7);
    assert!(!pending.is_empty(), "that connection's ids are gone");
    pending.sent(3);
    pending.answered(3);
    assert!(pending.is_empty());
}
