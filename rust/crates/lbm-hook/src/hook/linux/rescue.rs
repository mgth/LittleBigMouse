//! The panic shortcut on Linux, through the desktop's own global shortcuts (#526).
//!
//! A cursor trapped on a screen it cannot leave puts every recovery behind a click
//! that cannot be made. Windows has had a way in from the keyboard since #526; Linux
//! has had none, and a hook that survives its agent (D5) has no other way to be
//! stopped at all.
//!
//! Two deliberate choices, the same ones the Windows listener made for its own
//! reasons:
//!
//! * **The portal, not the keyboards.** The daemon grabs mice; reading keys would
//!   mean opening the keyboard devices too — a keylogging surface a mouse utility
//!   has no business needing, and the reason this was left undone until the portal
//!   could do it. `org.freedesktop.portal.GlobalShortcuts` gives the combination to
//!   the compositor, which owns it, arbitrates collisions, shows it in the desktop's
//!   own settings, and calls back. We never see a keystroke.
//!
//! * **Held, not tapped.** A shortcut that stops the mouse engine should ask to be
//!   meant. The portal says when the combination goes down and when it comes back
//!   up, so the hold is the time between the two.
//!
//! What the desktop binds is its decision, not ours: the trigger travels as a
//! *preference*. Whatever it settles on is logged, because a rescue nobody can name
//! is a rescue nobody can use — and on Plasma what it settles on can be *nothing*,
//! the shortcut appearing in the system settings with no key until the user gives it
//! one. Asking is the whole point of not reading the keyboard; it also means the
//! request can stay open for as long as the user takes to answer.

use std::sync::atomic::Ordering;
use std::sync::LazyLock;
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use futures::StreamExt;
use tokio::sync::watch;

use crate::shared::Shared;

/// Our id for the one shortcut we bind. Also what the desktop lists it under.
const ID: &str = "rescue";

/// What the desktop shows beside the binding.
const DESCRIPTION: &str = "Little Big Mouse: free a trapped cursor";

// How long the combination must stay down: one hold for the portal and for the
// grabbed stream of a master whose pointer is on another machine.
use crate::hook::crossing::keys::HOLD;

/// The pause before asking again a portal that could not be reached, doubled up to
/// [`LAST_RETRY`]: one that arrives late is usually only a session still starting, and
/// one that never comes should not cost anything.
const FIRST_RETRY: Duration = Duration::from_secs(5);
const LAST_RETRY: Duration = Duration::from_secs(5 * 60);

/// Bumped whenever the wanted shortcut changes, so a listener that is already up
/// knows its session is describing the wrong combination. A channel rather than a
/// counter, so that the listener can wait for the change instead of looking for it.
static GENERATION: LazyLock<watch::Sender<u64>> = LazyLock::new(|| watch::Sender::new(0));

/// Start the listener on a thread of its own — as on Windows, the rescue must not
/// depend on the loop it is rescuing the user from.
pub fn spawn(shared: &'static Shared, on_fire: fn(&'static Shared)) {
    std::thread::Builder::new()
        .name("lbm-rescue".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("[LittleBigMouse.Hook] rescue: no tokio runtime: {error}");
                    return;
                }
            };
            runtime.block_on(watch(shared, on_fire));
        })
        .map(|_| ())
        .unwrap_or_else(|error| {
            eprintln!("[LittleBigMouse.Hook] rescue: no listener thread: {error}");
        });
}

/// Tell the listener the wanted shortcut changed; it rebinds when it next can.
///
/// The portal takes the bindings of a session once, so a different combination
/// means a different session: there is nothing to update in place.
pub fn shortcut_changed() {
    GENERATION.send_modify(|generation| *generation += 1);
}

/// Bind, listen, and rebind whenever the shortcut changes — for as long as the
/// process lives. A portal that is not there is not an error: this desktop has no
/// global shortcuts to offer, said once.
async fn watch(shared: &'static Shared, on_fire: fn(&'static Shared)) {
    let mut registered = false;
    let mut announced_absence = false;
    let mut pause = FIRST_RETRY;
    loop {
        if !registered {
            registered = register().await;
        }
        let generation = *GENERATION.borrow();
        let wanted = shared
            .rescue_shortcut
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();

        // The desktop may be asking the user to allow the binding, and they may take
        // as long as they like. Give up only if the wanted shortcut changed under us:
        // then the answer would be to the wrong question.
        let bound = tokio::select! {
            bound = bind(&wanted) => bound,
            () = superseded(generation) => continue,
        };

        match bound {
            Ok(session) => {
                announced_absence = false;
                pause = FIRST_RETRY;
                listen(session, &wanted, generation, shared, on_fire).await;
            }
            Err(error) => {
                if !announced_absence {
                    eprintln!(
                        "[LittleBigMouse.Hook] rescue: no global shortcut ({error}); \
                         the cursor can still be freed by stopping the daemon"
                    );
                    announced_absence = true;
                }
                pause = after_failure(&error, shared, &wanted, generation, pause).await;
            }
        }
    }
}

/// The application id the portal knows this process by.
///
/// The portal finds the id of a process that is not sandboxed in the name of its
/// systemd unit, which the desktop derives from the `.desktop` entry that started it:
/// `app-littlebigmouse\x2dagent@autostart.service` for the agent's autostart entry, and
/// this process with it. `littlebigmouse-agent` is no valid id, so the portal sees
/// none, and the global shortcuts refuse a process with none ("An app id is
/// required"). Said outright instead, as the portal's host registry allows.
const APP_ID: &str = "io.github.mgth.LittleBigMouse";

/// Tells the portal who is asking, before anything is asked on the connection. One too
/// old to know the registry, or that already found an id of its own, keeps whatever it
/// had. Whether that is settled: `false` only while the portal is not there yet, to be
/// asked again before the next binding.
async fn register() -> bool {
    let Ok(app_id) = ashpd::AppID::try_from(APP_ID) else {
        return true;
    };
    match ashpd::register_host_app(app_id).await {
        Ok(()) => {
            eprintln!("[LittleBigMouse.Hook] rescue: known to the portal as {APP_ID}");
            true
        }
        Err(error) => {
            let absent = not_there(&error);
            if !absent {
                eprintln!("[LittleBigMouse.Hook] rescue: not registered with the portal ({error})");
            }
            !absent
        }
    }
}

/// Whether the portal is not on the bus (yet), rather than answering.
fn not_there(error: &ashpd::Error) -> bool {
    match error {
        ashpd::Error::Zbus(zbus::Error::MethodError(name, _, _)) => matches!(
            name.as_str(),
            "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
        ),
        ashpd::Error::Zbus(zbus::Error::InputOutput(_)) => true,
        _ => false,
    }
}

/// Waits for the moment to ask again after `error`, and returns the next pause.
async fn after_failure(
    error: &ashpd::Error,
    shared: &Shared,
    wanted: &str,
    generation: u64,
    pause: Duration,
) -> Duration {
    if refused(error) {
        // The portal answered, and its answer depends on who is asking: asking again
        // would only get the same one. Said every time, not once: saving a shortcut
        // clears the warning, and without this the window would then show a rescue
        // that no key can trigger.
        shared.report_rescue_refused(wanted);
        superseded(generation).await;
        return pause;
    }
    // A portal that arrives late is the ordinary case; retry, quietly from here on.
    tokio::select! {
        _ = tokio::time::sleep(pause) => {}
        () = superseded(generation) => {}
    }
    next_retry(pause)
}

/// Whether the portal said no, rather than not being there to ask. It does for a process
/// it can attach no application id to ("An app id is required").
fn refused(error: &ashpd::Error) -> bool {
    matches!(
        error,
        ashpd::Error::Portal(ashpd::PortalError::NotAllowed(_))
    )
}

fn next_retry(pause: Duration) -> Duration {
    (pause * 2).min(LAST_RETRY)
}

/// Resolves once the wanted shortcut is no longer the one `generation` describes.
async fn superseded(generation: u64) {
    differs_from(GENERATION.subscribe(), generation).await;
}

/// Resolves once `current` holds anything but `generation`, at once if it already does:
/// a change made before the wait began is not missed.
async fn differs_from(mut current: watch::Receiver<u64>, generation: u64) {
    // An error would mean the sender is gone, and it is a static.
    let _ = current.wait_for(|value| *value != generation).await;
}

/// One session, holding one binding.
struct Binding {
    shortcuts: GlobalShortcuts,
    session: ashpd::desktop::Session<GlobalShortcuts>,
    /// What the desktop bound the rescue to, if it listed it.
    trigger: Option<String>,
}

/// Ask the desktop for the combination `wanted` names. What it answered is said once
/// the binding is listened to: a key bound in a session nobody hears is no rescue.
async fn bind(wanted: &str) -> Result<Binding, ashpd::Error> {
    let shortcuts = GlobalShortcuts::new().await?;
    let session = shortcuts.create_session(Default::default()).await?;

    let mut shortcut = NewShortcut::new(ID, DESCRIPTION);
    // A preference, not an instruction: the desktop decides, and a key it has no
    // portable name for (the OEM positions) travels as no preference at all.
    if let Some(trigger) = crate::shortcut::portal_trigger(wanted) {
        shortcut = shortcut.preferred_trigger(trigger.as_str());
    } else {
        eprintln!(
            "[LittleBigMouse.Hook] rescue: {wanted} has no portable spelling; \
             the desktop will pick the binding"
        );
    }

    let bound = shortcuts
        .bind_shortcuts(&session, &[shortcut], None, Default::default())
        .await?
        .response()?;
    // The whole answer: a desktop can accept the request and bind nothing, and a rescue
    // missing from it is a rescue no key will trigger.
    let trigger = rescue_trigger(described(bound.shortcuts())).map(str::to_owned);

    Ok(Binding {
        shortcuts,
        session,
        trigger,
    })
}

fn described(shortcuts: &[Shortcut]) -> impl Iterator<Item = (&str, &str)> {
    shortcuts.iter().map(|s| (s.id(), s.trigger_description()))
}

/// What the desktop says of our shortcut, among the `(id, trigger)` it lists.
fn rescue_trigger<'a>(mut shortcuts: impl Iterator<Item = (&'a str, &'a str)>) -> Option<&'a str> {
    shortcuts.find_map(|(id, trigger)| (id == ID).then_some(trigger))
}

/// Reports what the desktop says the shortcut is bound to: `None` when it does not list
/// it at all.
fn note_trigger(shared: &Shared, wanted: &str, trigger: Option<&str>) {
    match trigger {
        // Registered and bound to nothing, or not registered. Said the same way Windows
        // says "another application owns this combination": a rescue that silently does
        // not exist is worse than none, because the user finds out at the moment they
        // need it.
        None | Some("" | "none") => {
            eprintln!(
                "[LittleBigMouse.Hook] rescue: no key bound — give it one in the desktop's \
                 shortcut settings, under {DESCRIPTION:?}"
            );
            shared
                .rescue_bound_by_desktop
                .store(false, Ordering::SeqCst);
            shared.report_rescue_refused(wanted);
        }
        Some(trigger) => {
            eprintln!("[LittleBigMouse.Hook] rescue: bound to {trigger}");
            shared.rescue_bound_by_desktop.store(true, Ordering::SeqCst);
            shared.report_rescue_bound(wanted);
        }
    }
}

/// The desktop's settings changed some shortcuts. Unlike the answer to a binding this
/// lists only those, so a list without ours says nothing about ours.
fn note_change<'a>(
    shared: &Shared,
    wanted: &str,
    changed: impl Iterator<Item = (&'a str, &'a str)>,
) {
    if let Some(trigger) = rescue_trigger(changed) {
        note_trigger(shared, wanted, Some(trigger));
    }
}

/// Follow one binding until the shortcut changes under it or the portal goes away.
async fn listen(
    binding: Binding,
    wanted: &str,
    generation: u64,
    shared: &'static Shared,
    on_fire: fn(&'static Shared),
) {
    let streams = (
        binding.shortcuts.receive_activated().await,
        binding.shortcuts.receive_deactivated().await,
        binding.shortcuts.receive_shortcuts_changed().await,
    );
    let (Ok(mut activated), Ok(mut deactivated), Ok(mut rebound)) = streams else {
        no_longer_heard(shared, wanted);
        let _ = binding.session.close().await;
        return;
    };
    note_trigger(shared, wanted, binding.trigger.as_deref());

    let mut lost = true;

    loop {
        tokio::select! {
            down = activated.next() => {
                if down.is_none() {
                    break;
                }
                // Held, not tapped: the combination has to outlast HOLD. Letting go
                // early is the user changing their mind, and costs nothing.
                let held = tokio::select! {
                    _ = tokio::time::sleep(HOLD) => true,
                    up = deactivated.next() => {
                        if up.is_none() {
                            break;
                        }
                        false
                    }
                };
                if held {
                    on_fire(shared);
                }
            }
            // A release with no press of ours: someone else's, or one we already
            // acted on. Nothing to do but keep the stream drained.
            up = deactivated.next() => {
                if up.is_none() {
                    break;
                }
            }
            // The user gave it a key, or took it away, in the desktop's settings: what
            // the warning says has to follow.
            changed = rebound.next() => {
                let Some(changed) = changed else {
                    break;
                };
                note_change(shared, wanted, described(changed.shortcuts()));
            }
            () = superseded(generation) => {
                lost = false;
                break;
            }
        }
    }

    if lost {
        no_longer_heard(shared, wanted);
    } else {
        // Bound again at once, for the shortcut now wanted.
        shared
            .rescue_bound_by_desktop
            .store(false, Ordering::SeqCst);
    }
    let _ = binding.session.close().await;
}

/// The portal's signals cannot be heard, or no longer are: whatever the desktop bound,
/// nothing here would act on it. The keys this daemon reads stop leaving the rescue to
/// it, and the rescue is said as refused until a binding is heard again.
fn no_longer_heard(shared: &Shared, wanted: &str) {
    eprintln!("[LittleBigMouse.Hook] rescue: the portal stopped answering");
    shared
        .rescue_bound_by_desktop
        .store(false, Ordering::SeqCst);
    shared.report_rescue_refused(wanted);
}

#[cfg(test)]
mod tests {
    use super::*;
    use lbm_ipc::protocol::Event;

    const PATIENCE: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn a_key_given_later_withdraws_the_warning() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        let shortcut = "Ctrl+Alt+F9".to_string();

        note_trigger(shared, &shortcut, Some("none"));
        assert_eq!(
            agent.next("the warning").await,
            Event::ShortcutUnavailable {
                shortcut: shortcut.clone(),
            }
        );

        note_change(shared, &shortcut, [(ID, "Ctrl+Alt+F9")].into_iter());
        assert_eq!(
            agent.next("its withdrawal").await,
            Event::ShortcutAvailable {
                shortcut: shortcut.clone(),
            }
        );
        assert_eq!(*shared.rescue_unavailable.lock().unwrap(), None);
    }

    /// A binding can succeed and bind nothing: the list comes back empty, or without us.
    #[tokio::test]
    async fn a_binding_that_leaves_the_rescue_out_is_reported() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        let shortcut = "Ctrl+Alt+F9".to_string();
        let unavailable = Event::ShortcutUnavailable {
            shortcut: shortcut.clone(),
        };

        for listed in [vec![], vec![("something-else", "Ctrl+A")]] {
            shared.report_rescue_available(&shortcut);
            assert_eq!(
                agent.next("the earlier success").await,
                Event::ShortcutAvailable {
                    shortcut: shortcut.clone(),
                }
            );

            note_trigger(shared, &shortcut, rescue_trigger(listed.into_iter()));
            assert_eq!(agent.next("the missing rescue").await, unavailable);
        }
    }

    /// `ShortcutsChanged` names what changed, which may be nothing of ours.
    #[tokio::test]
    async fn a_change_that_does_not_name_the_rescue_says_nothing() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;

        note_change(shared, "Ctrl+Alt+F9", std::iter::empty());
        note_change(shared, "Ctrl+Alt+F9", [("other", "none")].into_iter());

        agent.stays_quiet().await;
    }

    #[tokio::test]
    async fn a_refusal_is_told_to_the_window() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        let generation = *GENERATION.borrow();

        let waiting = tokio::spawn(async move {
            let refusal = ashpd::PortalError::NotAllowed("An app id is required".into());
            let error = ashpd::Error::Portal(refusal);
            after_failure(&error, shared, "Ctrl+Alt+F9", generation, FIRST_RETRY).await
        });

        assert_eq!(
            agent.next("the refusal").await,
            Event::ShortcutUnavailable {
                shortcut: "Ctrl+Alt+F9".to_string(),
            }
        );
        waiting.abort();
    }

    /// The evdev backend reads the keyboards and watches them for the rescue: the
    /// portal's refusal leaves it armed.
    #[tokio::test]
    async fn a_refusal_with_the_keyboards_read_leaves_the_rescue_armed() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        shared.set_keyboards_read(true);
        let mut agent = crate::testing::Listening::to(shared).await;
        let generation = *GENERATION.borrow();

        let waiting = tokio::spawn(async move {
            let refusal = ashpd::PortalError::NotAllowed("An app id is required".into());
            let error = ashpd::Error::Portal(refusal);
            after_failure(&error, shared, "Ctrl+Alt+F9", generation, FIRST_RETRY).await
        });

        assert_eq!(
            agent.next("the refusal").await,
            Event::ShortcutAvailable {
                shortcut: "Ctrl+Alt+F9".to_string(),
            }
        );
        waiting.abort();
    }

    /// The portal said no first, and the keyboards are read after: the warning is
    /// withdrawn then.
    #[tokio::test]
    async fn keyboards_read_after_a_refusal_withdraw_the_warning() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        shared.report_rescue_refused("Ctrl+Alt+F9");
        assert_eq!(
            agent.next("the refusal").await,
            Event::ShortcutUnavailable {
                shortcut: "Ctrl+Alt+F9".to_string(),
            }
        );
        shared.set_keyboards_read(true);
        assert_eq!(
            agent.next("the keyboards").await,
            Event::ShortcutAvailable {
                shortcut: "Ctrl+Alt+F9".to_string(),
            }
        );
    }

    /// The keyboards come and go: a rescue the desktop refused follows them, one it
    /// bound does not.
    #[tokio::test]
    async fn the_last_keyboard_gone_says_a_refused_rescue_again() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        let available = Event::ShortcutAvailable {
            shortcut: "Ctrl+Alt+F9".to_string(),
        };
        let unavailable = Event::ShortcutUnavailable {
            shortcut: "Ctrl+Alt+F9".to_string(),
        };

        shared.set_keyboards_read(true);
        shared.report_rescue_refused("Ctrl+Alt+F9");
        assert_eq!(agent.next("the refusal, read").await, available);
        shared.set_keyboards_read(true);
        shared.set_keyboards_read(false);
        assert_eq!(agent.next("the keyboards gone").await, unavailable);
        shared.set_keyboards_read(true);
        assert_eq!(agent.next("a keyboard plugged in").await, available);

        note_trigger(shared, "Ctrl+Alt+F9", Some("Ctrl+Alt+F9"));
        assert_eq!(agent.next("the desktop's binding").await, available);
        shared.set_keyboards_read(false);
        shared.report_rescue_available("marker");
        assert_eq!(
            agent.next("nothing but the marker").await,
            Event::ShortcutAvailable {
                shortcut: "marker".to_string(),
            }
        );
    }

    /// A binding whose signals cannot be heard is none: the keys read here take the
    /// rescue back, and it is said as refused.
    #[tokio::test]
    async fn a_binding_no_longer_heard_hands_the_rescue_back_to_the_keys() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        note_trigger(shared, "Ctrl+Alt+F9", Some("Ctrl+Alt+F9"));
        agent.next("the binding").await;
        assert!(shared.rescue_bound_by_desktop.load(Ordering::SeqCst));

        no_longer_heard(shared, "Ctrl+Alt+F9");
        assert!(!shared.rescue_bound_by_desktop.load(Ordering::SeqCst));
        assert_eq!(
            agent.next("the refusal").await,
            Event::ShortcutUnavailable {
                shortcut: "Ctrl+Alt+F9".to_string(),
            }
        );
    }

    #[test]
    fn the_app_id_is_one_the_portal_accepts() {
        assert!(ashpd::AppID::try_from(APP_ID).is_ok());
    }

    /// Absent is not refused: the portal may still come, so nothing is reported yet.
    #[tokio::test]
    async fn a_portal_out_of_reach_is_not_reported_and_is_retried_later() {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        let mut agent = crate::testing::Listening::to(shared).await;
        let generation = *GENERATION.borrow();
        let error = ashpd::Error::NoResponse;

        let next = after_failure(
            &error,
            shared,
            "Ctrl+Alt+F9",
            generation,
            Duration::from_millis(1),
        )
        .await;

        assert_eq!(next, Duration::from_millis(2));
        agent.stays_quiet().await;
    }

    #[test]
    fn only_a_refusal_ends_the_retries() {
        let refusal = ashpd::PortalError::NotAllowed("An app id is required".into());
        assert!(refused(&ashpd::Error::Portal(refusal)));

        let failure = ashpd::PortalError::Failed("not ready".into());
        assert!(!refused(&ashpd::Error::Portal(failure)));
        assert!(!refused(&ashpd::Error::NoResponse));
    }

    #[test]
    fn retries_slow_down_to_a_limit() {
        assert_eq!(next_retry(FIRST_RETRY), Duration::from_secs(10));
        assert_eq!(next_retry(Duration::from_secs(200)), LAST_RETRY);
        assert_eq!(next_retry(LAST_RETRY), LAST_RETRY);
    }

    #[tokio::test]
    async fn a_listener_waits_for_the_shortcut_to_change() {
        let (generation, current) = watch::channel(7);
        let mut waiting = tokio::spawn(differs_from(current, 7));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut waiting)
                .await
                .is_err(),
            "nothing changed yet"
        );

        generation.send_modify(|value| *value += 1);
        tokio::time::timeout(PATIENCE, waiting)
            .await
            .expect("the change ends the wait")
            .unwrap();
    }

    #[tokio::test]
    async fn a_change_made_before_the_wait_is_not_missed() {
        let (generation, current) = watch::channel(7);
        generation.send_modify(|value| *value += 1);
        tokio::time::timeout(PATIENCE, differs_from(current, 7))
            .await
            .expect("it had already changed");
    }

    /// Only that the wait ends: other tests load layouts, which change the shortcut.
    #[tokio::test]
    async fn shortcut_changed_reaches_a_listener() {
        let generation = *GENERATION.borrow();
        let waiting = tokio::spawn(superseded(generation));
        shortcut_changed();
        tokio::time::timeout(PATIENCE, waiting)
            .await
            .expect("the change ends the wait")
            .unwrap();
    }

    /// Not run by default: it asks the running desktop for a real shortcut, and what
    /// comes back is the desktop's answer, not ours. On a session with the portal:
    /// `cargo test -p lbm-hook --lib -- --ignored binds_for_real`.
    ///
    /// It is the only way to see what no unit test can: that the trigger spelling is
    /// one the portal accepts, and what the desktop made of it. The binding lives as
    /// long as the session, which this test closes.
    #[ignore = "asks the running desktop to bind a shortcut"]
    #[tokio::test]
    async fn binds_for_real_and_says_what_it_got() {
        let binding = bind(crate::shortcut::DEFAULT)
            .await
            .expect("the portal bound the shortcut");
        eprintln!("bound to {:?}", binding.trigger);

        binding.session.close().await.expect("the session closes");
    }
}
