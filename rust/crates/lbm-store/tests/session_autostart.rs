//! The session autostart a frontend asks for, and what the option then says.
//!
//! The option is what the frontends show, so it must be what the session will do: a
//! task that could not be registered must not be reported as registered.

mod common;

use std::cell::{Cell, RefCell};
use std::io;

use common::{design_options, temp_excluded_file, FakeStore};
use lbm_layout::model::Layout;
use lbm_store::{LayoutPersistence, PersistencePlatform};

/// A session autostart that may refuse, and remembers what it was asked.
#[derive(Default)]
struct FakeSession {
    scheduled: Cell<bool>,
    /// Fails every request with this message.
    fails: RefCell<Option<String>>,
    /// Says yes and does nothing (a registration that silently did not take).
    ignores: Cell<bool>,
    asked: RefCell<Vec<(bool, bool)>>,
}

impl PersistencePlatform for FakeSession {
    fn is_elevated(&self) -> bool {
        false
    }

    fn is_autostart_scheduled(&self, _layout: &Layout) -> bool {
        self.scheduled.get()
    }

    fn set_autostart(&self, _layout: &Layout, enabled: bool, elevated: bool) -> io::Result<()> {
        self.asked.borrow_mut().push((enabled, elevated));
        if let Some(message) = self.fails.borrow().clone() {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, message));
        }
        if !self.ignores.get() {
            self.scheduled.set(enabled);
        }
        Ok(())
    }
}

fn persistence(
    session: FakeSession,
) -> (tempfile::TempDir, LayoutPersistence<FakeStore, FakeSession>) {
    let (dir, excluded) = temp_excluded_file("lbm-session-autostart-tests");
    let persistence =
        LayoutPersistence::with_excluded_list_file(FakeStore::default(), session, move || {
            excluded.clone()
        });
    (dir, persistence)
}

fn layout(load_at_startup: bool) -> Layout {
    let mut layout = Layout::new(design_options());
    layout.id = "TESTMON1".to_owned();
    layout.options.load_at_startup = load_at_startup;
    layout
}

#[test]
fn a_registration_that_works_is_recorded() {
    let (_dir, persistence) = persistence(FakeSession::default());
    let mut layout = layout(false);

    assert_eq!(persistence.set_load_at_startup(&mut layout, true), Ok(()));

    assert!(layout.options.load_at_startup);
    assert!(persistence.platform().scheduled.get());
}

#[test]
fn a_registration_that_fails_is_refused_and_not_recorded() {
    let session = FakeSession::default();
    *session.fails.borrow_mut() = Some("Access is denied.".to_owned());
    let (_dir, persistence) = persistence(session);
    let mut layout = layout(false);

    let refused = persistence
        .set_load_at_startup(&mut layout, true)
        .unwrap_err();

    assert!(refused.contains("Access is denied."), "{refused}");
    assert!(
        !layout.options.load_at_startup,
        "no task, so the option says no"
    );
}

#[test]
fn a_registration_that_silently_did_not_take_is_refused_too() {
    let session = FakeSession::default();
    session.ignores.set(true);
    let (_dir, persistence) = persistence(session);
    let mut layout = layout(false);

    let refused = persistence
        .set_load_at_startup(&mut layout, true)
        .unwrap_err();

    assert!(refused.contains("could not be set up"), "{refused}");
    assert!(!layout.options.load_at_startup);
}

#[test]
fn a_removal_that_fails_leaves_the_option_on() {
    // The other way round: the task is still there, so the session still starts the
    // app, and the option must keep saying so.
    let session = FakeSession::default();
    session.scheduled.set(true);
    *session.fails.borrow_mut() = Some("Access is denied.".to_owned());
    let (_dir, persistence) = persistence(session);
    let mut layout = layout(true);

    assert!(persistence.set_load_at_startup(&mut layout, false).is_err());

    assert!(layout.options.load_at_startup);
}

#[test]
fn the_registration_asks_for_the_elevation_the_options_hold() {
    let (_dir, persistence) = persistence(FakeSession::default());
    let mut layout = layout(false);
    layout.options.start_elevated = true;

    persistence.set_load_at_startup(&mut layout, true).unwrap();

    assert_eq!(*persistence.platform().asked.borrow(), vec![(true, true)]);
}
