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

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures::StreamExt;

use crate::shared::Shared;

/// Our id for the one shortcut we bind. Also what the desktop lists it under.
const ID: &str = "rescue";

/// What the desktop shows beside the binding.
const DESCRIPTION: &str = "Little Big Mouse: free a trapped cursor";

/// How long the combination must stay down, as on Windows: long enough not to fire
/// on a fumble, short enough to feel like a rescue.
const HOLD: Duration = Duration::from_millis(900);

/// Bumped whenever the wanted shortcut changes, so a listener that is already up
/// knows its session is describing the wrong combination.
static GENERATION: AtomicU64 = AtomicU64::new(0);

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
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Bind, listen, and rebind whenever the shortcut changes — for as long as the
/// process lives. A portal that is not there is not an error: this desktop has no
/// global shortcuts to offer, said once.
async fn watch(shared: &'static Shared, on_fire: fn(&'static Shared)) {
    let mut announced_absence = false;
    loop {
        let generation = GENERATION.load(Ordering::SeqCst);
        let wanted = shared
            .rescue_shortcut
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();

        // The desktop may be asking the user to allow the binding, and they may take
        // as long as they like. Give up only if the wanted shortcut changed under us:
        // then the answer would be to the wrong question.
        let bound = tokio::select! {
            bound = bind(shared, &wanted) => bound,
            () = superseded(generation) => continue,
        };

        match bound {
            Ok(session) => {
                announced_absence = false;
                listen(session, generation, shared, on_fire).await;
            }
            Err(error) => {
                if !announced_absence {
                    eprintln!(
                        "[LittleBigMouse.Hook] rescue: no global shortcut ({error}); \
                         the cursor can still be freed by stopping the daemon"
                    );
                    announced_absence = true;
                }
                // A portal that arrives late (a session still starting) is the
                // ordinary case; retry, quietly from here on.
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Resolves once the wanted shortcut is no longer the one `generation` describes.
async fn superseded(generation: u64) {
    while GENERATION.load(Ordering::SeqCst) == generation {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// One session, holding one binding.
struct Binding {
    shortcuts: GlobalShortcuts,
    session: ashpd::desktop::Session<GlobalShortcuts>,
}

/// Ask the desktop for the combination `wanted` names. `shared` is only told when
/// the answer is "registered, but bound to nothing".
async fn bind(shared: &'static Shared, wanted: &str) -> Result<Binding, ashpd::Error> {
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
    for shortcut in bound.shortcuts() {
        match shortcut.trigger_description() {
            // Registered, described, and bound to nothing. Said the same way Windows
            // says "another application owns this combination": a rescue that
            // silently does not exist is worse than none, because the user finds out
            // at the moment they need it.
            "" | "none" => {
                eprintln!(
                    "[LittleBigMouse.Hook] rescue: registered with no key — give it one in \
                     the desktop's shortcut settings, under {DESCRIPTION:?}"
                );
                shared.broadcast(&crate::ipc::protocol::shortcut_unavailable(wanted));
            }
            trigger => eprintln!("[LittleBigMouse.Hook] rescue: bound to {trigger}"),
        }
    }

    Ok(Binding { shortcuts, session })
}

/// Follow one binding until the shortcut changes under it or the portal goes away.
async fn listen(
    binding: Binding,
    generation: u64,
    shared: &'static Shared,
    on_fire: fn(&'static Shared),
) {
    let (Ok(mut activated), Ok(mut deactivated)) = (
        binding.shortcuts.receive_activated().await,
        binding.shortcuts.receive_deactivated().await,
    ) else {
        eprintln!("[LittleBigMouse.Hook] rescue: the portal stopped answering");
        let _ = binding.session.close().await;
        return;
    };

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
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                if GENERATION.load(Ordering::SeqCst) != generation {
                    break;
                }
            }
        }
    }

    let _ = binding.session.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let shared: &'static Shared = crate::shared::SHARED.get_or_init(Shared::new);
        let binding = bind(shared, crate::shortcut::DEFAULT)
            .await
            .expect("the portal bound the shortcut");

        binding.session.close().await.expect("the session closes");
    }
}
