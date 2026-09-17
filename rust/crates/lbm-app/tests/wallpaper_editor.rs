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

fn image_editor() -> Editor {
    use lbm_store::wallpaper_settings::ScreenWallpaperSettings;
    let mut e = ready();
    for id in ["A", "B"] {
        e.draft.as_mut().unwrap().per_screen.insert(
            id.into(),
            ScreenWallpaperSettings {
                image_path: Some(format!("/images/{id}.png")),
                ..Default::default()
            },
        );
    }
    e
}
#[test]
fn a_choice_updates_only_its_original_image_and_leaves_apply_explicit() {
    for panorama in [false, true] {
        let mut e = image_editor();
        if panorama {
            e.draft.as_mut().unwrap().mode = WallpaperMode::Span;
        }
        let choice = e.choose_file(Some("A")).unwrap();
        assert_eq!(
            choice.current_path(),
            if panorama {
                None
            } else {
                Some("/images/A.png")
            }
        );
        assert!(e.file_chosen(choice, Some("A"), Ok(Some("/images/été 100%.png".into()))));
        let settings = e.draft.as_ref().unwrap();
        assert_eq!(
            settings.per_screen["B"].image_path.as_deref(),
            Some("/images/B.png")
        );
        assert_eq!(
            if panorama {
                settings.span_image_path.as_deref()
            } else {
                settings.per_screen["A"].image_path.as_deref()
            },
            Some("/images/été 100%.png")
        );
        assert!(e.dirty());
        assert!(!e.busy());
    }
}
#[test]
fn cancelled_failed_and_obsolete_choices_never_overwrite_the_draft() {
    for cause in 0..8 {
        let mut e = image_editor();
        let choice = e.choose_file(Some("A")).unwrap();
        let mut selected = Some("A");
        match cause {
            0 => e.disconnect(),
            1 => e.observes(&Key {
                layout: "changed".into(),
                revision: 2,
            }),
            2 => e.reset(),
            3 => {
                e.read(
                    44,
                    Key {
                        layout: "A".into(),
                        revision: 1,
                    },
                );
            }
            4 => {
                e.draft
                    .as_mut()
                    .unwrap()
                    .per_screen
                    .get_mut("A")
                    .unwrap()
                    .image_path = Some("typed-after-opening.png".into())
            }
            5 => selected = Some("B"),
            6 => e.draft.as_mut().unwrap().mode = WallpaperMode::Span,
            _ => {
                e.apply(44);
            }
        }
        let before = e.draft.clone();
        assert!(!e.file_chosen(choice, selected, Ok(Some("late.png".into()))));
        assert_eq!(e.draft, before);
    }
    let mut e = image_editor();
    let before = e.draft.clone();
    let choice = e.choose_file(Some("A")).unwrap();
    assert!(!e.file_chosen(choice, Some("A"), Ok(None)));
    assert_eq!(e.draft, before);
    assert!(e.error.is_none());
    let choice = e.choose_file(Some("A")).unwrap();
    assert!(!e.file_chosen(choice, Some("A"), Err("portal unavailable".into())));
    assert_eq!(e.draft, before);
    assert_eq!(e.error.as_deref(), Some("portal unavailable"));
}
#[test]
fn browsing_requires_an_image_destination_but_not_native_wallpaper_support() {
    let mut e = ready();
    e.supported = false;
    assert!(e.choose_file(None).is_none());
    assert!(e.choose_file(Some("missing")).is_none());
    e.draft.as_mut().unwrap().mode = WallpaperMode::Span;
    assert!(e.choose_file(None).is_some());
    e.draft.as_mut().unwrap().mode = WallpaperMode::PerScreen;
    e.draft.as_mut().unwrap().per_screen.insert(
        "A".into(),
        lbm_store::wallpaper_settings::ScreenWallpaperSettings {
            kind: lbm_store::wallpaper_settings::ScreenWallpaperKind::Color,
            ..Default::default()
        },
    );
    assert!(e.choose_file(Some("A")).is_none());
}

#[test]
fn reselecting_the_same_repaired_file_refreshes_preview_without_making_it_dirty() {
    let mut e = image_editor();
    let settings = e.draft.clone().unwrap();
    e.read(
        20,
        Key {
            layout: "A".into(),
            revision: 1,
        },
    );
    e.answered(20, &Ok(json!({"Settings":settings,"Supported":true})));
    e.error = Some("image was missing".into());
    let choice = e.choose_file(Some("A")).unwrap();
    assert!(e.file_chosen(choice, Some("A"), Ok(Some("/images/A.png".into()))));
    assert!(!e.dirty());
    assert!(e.error.is_none());
}
