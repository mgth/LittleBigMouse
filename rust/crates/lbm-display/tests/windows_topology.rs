//! CCD policy and sequencing, exercised without touching any real display.
use lbm_display::windows::topology::*;
fn id(adapter: u64, id: u32) -> Id {
    Id { adapter, id }
}
fn path(source: u32, target: u32, active: bool) -> Path {
    Path {
        id: PathId {
            source: id(1, source),
            target: id(1, target),
        },
        connector: format!(r"\\?\DISPLAY#PANEL{target}#serial#{{guid}}"),
        name: format!("Panel {target}"),
        available: true,
        active,
    }
}
fn source(id_: u32, x: i32, y: i32) -> Source {
    Source {
        id: id(1, id_),
        x,
        y,
        width: 1920,
        height: 1080,
        pixel_format: 4,
    }
}
fn desktop() -> Configuration {
    Configuration {
        paths: vec![
            path(0, 10, true),
            path(1, 11, true),
            path(0, 12, false),
            path(2, 12, false),
            path(2, 13, false),
            path(3, 13, false),
        ],
        sources: vec![source(0, 0, 0), source(1, -1920, 120)],
        details: "rotation=portrait;refresh=60000/1001".into(),
    }
}
#[test]
fn inventory_deduplicates_targets_but_keeps_all_detached_monitors() {
    let c = desktop();
    let outputs = c.outputs().unwrap();
    assert_eq!(outputs.len(), 4);
    assert!(outputs[0].primary && outputs[0].enabled);
    assert!(!outputs[1].primary && outputs[1].enabled);
    assert!(!outputs[2].enabled && !outputs[3].enabled);
    assert_eq!(outputs[3].connector, c.paths[4].connector);
}
#[test]
fn attach_uses_the_selected_target_and_a_free_source_not_the_first_binding() {
    let c = desktop();
    let p = c
        .plan(&c.paths[2].connector, Change::Attach)
        .unwrap()
        .unwrap();
    assert_eq!(p.activate, Some(c.paths[3].id));
    assert_eq!(p.paths, vec![c.paths[0].id, c.paths[1].id, c.paths[3].id]);
    assert!(p.allow_changes);
    assert!(p.positions.is_empty());
    let p = c
        .plan(&c.paths[5].connector, Change::Attach)
        .unwrap()
        .unwrap();
    assert_eq!(p.activate, Some(c.paths[4].id));
}
#[test]
fn an_attach_without_a_free_source_never_replaces_or_clones_an_active_monitor() {
    let mut c = desktop();
    c.paths.retain(|p| p.id.source.id < 2);
    assert!(c
        .plan(&c.paths[2].connector, Change::Attach)
        .unwrap_err()
        .contains("unused"));
}
#[test]
fn source_and_target_ids_include_the_adapter() {
    let mut c = desktop();
    c.paths[3].id.source = id(2, 0);
    assert_eq!(
        c.plan(&c.paths[3].connector, Change::Attach)
            .unwrap()
            .unwrap()
            .activate,
        Some(c.paths[3].id)
    );
}
#[test]
fn primary_translates_each_source_once_and_keeps_the_selected_path_first() {
    let c = desktop();
    let p = c
        .plan(&c.paths[1].connector, Change::Primary)
        .unwrap()
        .unwrap();
    assert_eq!(p.paths, vec![c.paths[1].id, c.paths[0].id]);
    assert!(!p.allow_changes);
    assert_eq!(p.positions, vec![(id(1, 0), 1920, -120), (id(1, 1), 0, 0)]);
    assert_eq!(c.sources[0].width, 1920);
    assert_eq!(c.sources[1].y, 120);
}
#[test]
fn detaching_the_primary_selects_a_remaining_origin_and_preserves_other_paths() {
    let c = desktop();
    let p = c
        .plan(&c.paths[0].connector, Change::Detach)
        .unwrap()
        .unwrap();
    assert_eq!(p.paths, vec![c.paths[1].id]);
    assert_eq!(p.positions, vec![(id(1, 1), 0, 0)]);
}
#[test]
fn cloned_sources_count_monitors_not_paths_or_sources_for_the_last_screen_guard() {
    let mut c = desktop();
    c.paths = vec![path(0, 10, true), path(0, 11, true)];
    c.sources.truncate(1);
    let p = c
        .plan(&c.paths[0].connector, Change::Detach)
        .unwrap()
        .unwrap();
    assert_eq!(p.paths, vec![c.paths[1].id]);
    c.paths.remove(0);
    assert!(c
        .plan(&c.paths[0].connector, Change::Detach)
        .unwrap_err()
        .contains("last enabled"));
}
#[test]
fn detached_primary_unknown_identity_and_ambiguous_targets_are_rejected() {
    let mut c = desktop();
    assert!(c.plan(&c.paths[2].connector, Change::Primary).is_err());
    assert!(c.plan("", Change::Attach).is_err());
    c.paths[2].connector = c.paths[0].connector.to_uppercase();
    assert!(c.outputs().is_err());
    let mut c = desktop();
    c.paths.push(c.paths[0].clone());
    assert!(c.outputs().is_err());
}
#[test]
fn missing_modes_and_coordinate_overflow_are_errors_not_partial_plans() {
    let mut c = desktop();
    c.sources.remove(0);
    assert!(c.plan(&c.paths[1].connector, Change::Primary).is_err());
    let mut c = desktop();
    c.sources[0].x = i32::MAX;
    c.sources[1].x = i32::MIN;
    assert!(c
        .plan(&c.paths[1].connector, Change::Primary)
        .unwrap_err()
        .contains("overflow"));
}
#[test]
fn already_satisfied_requests_are_noops_and_identity_matching_is_case_insensitive() {
    let c = desktop();
    for (i, change) in [
        (0, Change::Primary),
        (1, Change::Attach),
        (2, Change::Detach),
    ] {
        assert!(c
            .plan(&c.paths[i].connector.to_uppercase(), change)
            .unwrap()
            .is_none());
    }
}
#[test]
fn inventory_signature_covers_detached_identity_bindings_positions_and_modes() {
    let c = desktop();
    let signature = c.signature();
    for change in 0..5 {
        let mut edited = c.clone();
        match change {
            0 => edited.paths[3].connector.push('x'),
            1 => edited.paths[3].id.source.id = 9,
            2 => edited.sources[1].x -= 1,
            3 => edited.sources[1].width = 2560,
            _ => edited.details.push('x'),
        }
        assert_ne!(edited.signature(), signature);
    }
}

struct Fake {
    reads: std::collections::VecDeque<Result<Configuration, String>>,
    calls: Vec<&'static str>,
    validate_ok: bool,
    apply_ok: bool,
}
impl Backend for Fake {
    fn query(&mut self) -> Result<Configuration, String> {
        self.calls.push("query");
        self.reads.pop_front().unwrap()
    }
    fn validate(&mut self, _: &Plan) -> Result<(), String> {
        self.calls.push("validate");
        if self.validate_ok {
            Ok(())
        } else {
            Err("unsupported mode".into())
        }
    }
    fn apply(&mut self, _: &Plan) -> Result<(), String> {
        self.calls.push("apply");
        if self.apply_ok {
            Ok(())
        } else {
            Err("driver failure".into())
        }
    }
}
fn after_primary(c: &Configuration) -> Configuration {
    let mut after = c.clone();
    after.sources[0].x = 1920;
    after.sources[0].y = -120;
    after.sources[1].x = 0;
    after.sources[1].y = 0;
    after
}
fn fake(reads: Vec<Configuration>) -> Fake {
    Fake {
        reads: reads.into_iter().map(Ok).collect(),
        calls: vec![],
        validate_ok: true,
        apply_ok: true,
    }
}
#[test]
fn a_success_requires_validation_fresh_inventory_and_post_apply_verification() {
    let c = desktop();
    let mut backend = fake(vec![c.clone(), c.clone(), after_primary(&c)]);
    change_checked_with(
        &mut backend,
        Change::Primary,
        &c.paths[1].connector,
        &c.signature(),
    )
    .unwrap();
    assert_eq!(
        backend.calls,
        ["query", "validate", "query", "apply", "query"]
    );
}
#[test]
fn no_native_write_on_stale_inventory_validation_failure_or_validation_race() {
    let c = desktop();
    let mut backend = fake(vec![c.clone()]);
    assert!(
        change_checked_with(&mut backend, Change::Primary, &c.paths[1].connector, "old").is_err()
    );
    assert_eq!(backend.calls, ["query"]);
    let mut backend = fake(vec![c.clone()]);
    backend.validate_ok = false;
    assert!(change_checked_with(
        &mut backend,
        Change::Primary,
        &c.paths[1].connector,
        &c.signature()
    )
    .is_err());
    assert_eq!(backend.calls, ["query", "validate"]);
    let mut changed = c.clone();
    changed.sources[1].width = 3840;
    let mut backend = fake(vec![c.clone(), changed]);
    assert!(change_checked_with(
        &mut backend,
        Change::Primary,
        &c.paths[1].connector,
        &c.signature()
    )
    .is_err());
    assert_eq!(backend.calls, ["query", "validate", "query"]);
}
#[test]
fn success_return_without_the_requested_result_is_an_error() {
    let c = desktop();
    let mut backend = fake(vec![c.clone(), c.clone(), c.clone()]);
    assert!(change_checked_with(
        &mut backend,
        Change::Primary,
        &c.paths[1].connector,
        &c.signature()
    )
    .unwrap_err()
    .contains("did not confirm"));
}
#[test]
fn driver_failure_still_rereads_and_keeps_the_original_error() {
    let c = desktop();
    let mut backend = fake(vec![c.clone(), c.clone(), after_primary(&c)]);
    backend.apply_ok = false;
    assert_eq!(
        change_checked_with(
            &mut backend,
            Change::Primary,
            &c.paths[1].connector,
            &c.signature()
        )
        .unwrap_err(),
        "driver failure"
    );
    assert_eq!(backend.calls.last(), Some(&"query"));
}
#[test]
fn losing_another_active_monitor_or_moving_it_unexpectedly_is_not_success() {
    let c = desktop();
    for missing in [true, false] {
        let mut after = after_primary(&c);
        if missing {
            after.paths[0].active = false;
        } else {
            after.sources[0].x = 0;
        }
        let mut backend = fake(vec![c.clone(), c.clone(), after]);
        assert!(change_checked_with(
            &mut backend,
            Change::Primary,
            &c.paths[1].connector,
            &c.signature()
        )
        .is_err());
    }
}

#[test]
fn unexpected_activation_and_resolution_changes_are_reported() {
    let c = desktop();
    for resolution in [true, false] {
        let mut after = after_primary(&c);
        if resolution {
            after.sources[0].width = 1280;
        } else {
            after.paths[3].active = true;
            after.sources.push(source(2, 4000, 0));
        }
        let mut backend = fake(vec![c.clone(), c.clone(), after]);
        assert!(change_checked_with(
            &mut backend,
            Change::Primary,
            &c.paths[1].connector,
            &c.signature()
        )
        .is_err());
    }
}
