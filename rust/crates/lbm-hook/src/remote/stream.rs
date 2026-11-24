//! The injector of `lbm-inject` (docs/multi-pc-service-windows.md): a stream of input
//! batches on a blocking reader, through the same [`Session`] as the hook's input
//! endpoint, into an [`Injector`].
//!
//! The reader is the one thing `lbm-inject` listens to: the pipe the Windows service
//! hands it at launch. There is no other input, no file and no configuration, so that
//! what runs with the service's rights does nothing but inject.
//!
//! Whatever ends the stream — its clean end, a batch cut short, one that does not
//! decode — ends the session the way the hook's endpoint does: everything still
//! pressed is released, then the pointer is unparked.

use std::io::{self, Read};

use lbm_ipc::input::{decode, read_batch_blocking};

use super::{deliver, Injector, Session};

/// Reads batches from `reader` and injects them until the stream ends. `Ok` for a
/// clean end, the error otherwise; the releases happen in both cases.
///
/// For an injector that never holds input back ([`Injector::due`] is always `None`),
/// as `SendInput` is: a blocking read has no deadline to wake up at.
pub fn pump(reader: &mut impl Read, injector: &mut dyn Injector) -> io::Result<()> {
    let mut session = Session::default();
    let ended = loop {
        let batch = match read_batch_blocking(reader) {
            Ok(Some(batch)) => batch,
            Ok(None) => break Ok(()),
            Err(error) => break Err(error),
        };
        match decode(&batch) {
            Ok(events) => deliver(injector, session.apply(&events)),
            Err(error) => break Err(error),
        }
    };
    deliver(injector, session.release_all());
    injector.unpark();
    ended
}

/// Puts `count` inputs into the system with `send`, following the input desktop.
///
/// `send(start)` puts in the inputs from `start` on and says how many went in.
/// Windows takes input only on the desktop the calling thread is attached to, and
/// that stops being the input desktop when the secure desktop comes up (a UAC prompt,
/// the lock screen) or goes away. Fewer in than asked is that moment: `reattach`
/// moves the thread to the input desktop and says whether it changed, and only then
/// is the rest sent again, once. Says how many went in: the first ones, in order.
pub fn send_following(
    count: usize,
    mut send: impl FnMut(usize) -> usize,
    reattach: impl FnOnce() -> bool,
) -> usize {
    let sent = send(0).min(count);
    if sent == count || !reattach() {
        return sent;
    }
    (sent + send(sent)).min(count)
}

/// Keeps, out of what did not go in (`unsent`, the tail of a batch), the releases.
///
/// The [`Session`] stops tracking a key the moment it hands its release over, so a
/// release the system refused — the secure desktop came up between the press and
/// the release — would leave the key down for good, and the end of the stream would
/// not know to lift it. Such releases are kept and put in again ahead of the next
/// batch, and when the injector parks or leaves. Presses and motions are not kept:
/// input put in late is input nobody typed.
pub fn keep_releases<T: Copy>(kept: &mut Vec<T>, unsent: &[T], is_release: impl Fn(&T) -> bool) {
    kept.extend(unsent.iter().copied().filter(|input| is_release(input)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::Injection;
    use lbm_ipc::input::{encode, InputEvent};

    #[derive(Default)]
    struct Recorder(Vec<String>);

    impl Injector for Recorder {
        fn inject(&mut self, batch: &[Injection]) {
            for injection in batch {
                self.0.push(format!("{injection:?}"));
            }
        }
        fn park(&mut self) {
            self.0.push("park".into());
        }
        fn unpark(&mut self) {
            self.0.push("unpark".into());
        }
    }

    fn framed(batches: &[Vec<InputEvent>]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for events in batches {
            let batch = encode(events);
            bytes.extend_from_slice(&(batch.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&batch);
        }
        bytes
    }

    const KEY_A: u16 = 30;

    #[test]
    fn a_stream_is_injected_and_its_end_releases_what_it_pressed() {
        let bytes = framed(&[
            vec![InputEvent::Enter { x: 10, y: 20 }],
            vec![InputEvent::Key {
                code: KEY_A,
                pressed: true,
            }],
        ]);
        let mut recorder = Recorder::default();
        pump(&mut &bytes[..], &mut recorder).unwrap();
        assert_eq!(
            recorder.0,
            [
                "unpark",
                "MoveTo { x: 10, y: 20 }",
                "Key { code: 30, pressed: true }",
                "Key { code: 30, pressed: false }",
                "park",
                "unpark",
            ]
        );
    }

    #[test]
    fn nothing_is_injected_before_an_enter() {
        let bytes = framed(&[vec![
            InputEvent::Motion { x: 1, y: 1 },
            InputEvent::Key {
                code: KEY_A,
                pressed: true,
            },
        ]]);
        let mut recorder = Recorder::default();
        pump(&mut &bytes[..], &mut recorder).unwrap();
        assert_eq!(recorder.0, ["unpark"]);
    }

    #[test]
    fn a_batch_cut_short_ends_the_stream_with_its_releases() {
        let mut bytes = framed(&[
            vec![InputEvent::Enter { x: 0, y: 0 }],
            vec![InputEvent::Key {
                code: KEY_A,
                pressed: true,
            }],
        ]);
        bytes.extend_from_slice(&[9, 0, 0, 0, 1]);
        let mut recorder = Recorder::default();
        assert!(pump(&mut &bytes[..], &mut recorder).is_err());
        assert!(recorder.0.ends_with(&[
            "Key { code: 30, pressed: false }".into(),
            "park".into(),
            "unpark".into()
        ]));
    }

    #[test]
    fn a_batch_that_does_not_decode_ends_the_stream_with_its_releases() {
        let mut bytes = framed(&[
            vec![InputEvent::Enter { x: 0, y: 0 }],
            vec![InputEvent::Key {
                code: KEY_A,
                pressed: true,
            }],
        ]);
        bytes.extend_from_slice(&[1, 0, 0, 0, 0xff]);
        let mut recorder = Recorder::default();
        assert!(pump(&mut &bytes[..], &mut recorder).is_err());
        assert!(recorder.0.ends_with(&[
            "Key { code: 30, pressed: false }".into(),
            "park".into(),
            "unpark".into()
        ]));
    }

    #[test]
    fn everything_in_at_once_never_reattaches() {
        let reattached = std::cell::Cell::new(false);
        let sent = send_following(
            3,
            |_| 3,
            || {
                reattached.set(true);
                true
            },
        );
        assert_eq!(sent, 3);
        assert!(!reattached.get());
    }

    #[test]
    fn a_short_send_on_a_new_desktop_sends_the_rest_once() {
        let mut starts = Vec::new();
        let sent = send_following(
            5,
            |start| {
                starts.push(start);
                if start == 0 {
                    2
                } else {
                    5 - start
                }
            },
            || true,
        );
        assert_eq!(sent, 5);
        assert_eq!(starts, [0, 2]);
    }

    #[test]
    fn a_short_send_on_the_same_desktop_is_not_sent_again() {
        let mut calls = 0;
        let sent = send_following(
            4,
            |_| {
                calls += 1;
                0
            },
            || false,
        );
        assert_eq!(sent, 0);
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_second_short_send_is_reported_and_not_retried_further() {
        let mut calls = 0;
        let sent = send_following(
            4,
            |_| {
                calls += 1;
                1
            },
            || true,
        );
        assert_eq!(sent, 2);
        assert_eq!(calls, 2);
    }

    /// What a Windows injector does around `SendInput`, with the inputs as
    /// `(code, pressed)` and `accepts` saying how many of the call go in.
    fn put(kept: &mut Vec<(u16, bool)>, batch: &[(u16, bool)], accepts: usize) -> Vec<(u16, bool)> {
        let mut all = std::mem::take(kept);
        all.extend_from_slice(batch);
        let sent = accepts.min(all.len());
        keep_releases(kept, &all[sent..], |&(_, pressed)| !pressed);
        all[..sent].to_vec()
    }

    #[test]
    fn a_modifier_whose_release_alone_is_refused_is_released_with_the_next_batch() {
        const CTRL: u16 = 29;
        let mut kept = Vec::new();
        assert_eq!(put(&mut kept, &[(CTRL, true)], 1), [(CTRL, true)]);
        // The secure desktop came up: the release does not go in.
        assert!(put(&mut kept, &[(CTRL, false)], 0).is_empty());
        assert_eq!(kept, [(CTRL, false)]);
        // Back on a desktop that takes input: the release goes first.
        assert_eq!(
            put(&mut kept, &[(KEY_A, true)], 2),
            [(CTRL, false), (KEY_A, true)]
        );
        assert!(kept.is_empty());
    }

    #[test]
    fn only_releases_are_kept_from_what_did_not_go_in() {
        let mut kept = Vec::new();
        put(
            &mut kept,
            &[(1, true), (2, false), (3, true), (4, false)],
            1,
        );
        assert_eq!(kept, [(2, false), (4, false)]);
    }

    #[test]
    fn a_release_refused_twice_is_kept_once() {
        let mut kept = Vec::new();
        put(&mut kept, &[(1, false)], 0);
        put(&mut kept, &[], 0);
        assert_eq!(kept, [(1, false)]);
    }
}
