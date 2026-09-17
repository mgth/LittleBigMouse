use lbm_app::wallpaper_editor::{Editor, Key};
use lbm_store::wallpaper_settings::{LayoutWallpaperSettings, WallpaperMode};
use serde_json::json;
fn ready() -> Editor {
    let mut e = Editor::default();
    e.read(
        10,
        Key {
            layout: "A".into(),
            revision: 1,
        },
    );
    assert!(e.answered(
        10,
        &Ok(json!({"Settings":LayoutWallpaperSettings::default(),"Supported":true}))
    ));
    e
}
#[test]
fn only_the_sent_draft_is_earned_and_a_desktop_failure_is_not_a_save_failure() {
    let mut e = ready();
    e.draft.as_mut().unwrap().mode = WallpaperMode::Span;
    assert!(e.apply(11).is_some());
    e.draft.as_mut().unwrap().span_image_path = Some("newer.png".into());
    assert!(e.answered(
        11,
        &Ok(json!({"Saved":true,"Applied":false,"Error":"shell refused"}))
    ));
    assert!(e.dirty());
    assert!(e.error.as_ref().unwrap().contains("Settings saved"));
    e.reset();
    assert!(!e.dirty());
    assert_eq!(e.draft.as_ref().unwrap().span_image_path, None);
    assert_eq!(e.draft.as_ref().unwrap().mode, WallpaperMode::Span);
}
#[test]
fn a_failed_save_keeps_edits_and_busy_disables_duplicate_requests() {
    let mut e = ready();
    e.draft.as_mut().unwrap().mode = WallpaperMode::Span;
    assert!(e.apply(11).is_some());
    assert!(e.apply(12).is_none());
    assert!(!e.answered(12, &Ok(json!({"Saved":true,"Applied":true}))));
    e.answered(11, &Err("disk full".into()));
    assert!(e.dirty());
    e.reset();
    assert_eq!(e.draft.unwrap().mode, WallpaperMode::PerScreen);
}
#[test]
fn disconnect_and_a_new_revision_retire_answers_without_losing_edits() {
    for disconnect in [false, true] {
        let mut e = ready();
        e.draft.as_mut().unwrap().mode = WallpaperMode::Span;
        e.apply(11);
        if disconnect {
            e.disconnect();
        } else {
            e.observes(&Key {
                layout: "A".into(),
                revision: 2,
            });
        }
        assert!(e.stale);
        assert!(!e.busy());
        assert!(e.dirty());
        assert!(e.apply(12).is_none());
        assert!(!e.answered(11, &Ok(json!({"Saved":true,"Applied":true}))));
        let generation = e.preview_generation;
        e.observes(&Key {
            layout: "B".into(),
            revision: 3,
        });
        assert_eq!(generation, e.preview_generation);
    }
}
#[test]
fn reloading_is_explicit_and_does_not_accept_an_older_reply() {
    let mut e = ready();
    e.draft.as_mut().unwrap().mode = WallpaperMode::Span;
    e.read(
        20,
        Key {
            layout: "B".into(),
            revision: 2,
        },
    );
    assert!(!e.answered(
        10,
        &Ok(json!({"Settings":LayoutWallpaperSettings::default(),"Supported":true}))
    ));
    assert!(e.answered(20, &Err("corrupt settings".into())));
    assert!(e.dirty());
    assert!(!e.supported);
    assert!(e.apply(21).is_none());
}
