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
/// is the rest sent again, once. `false` when inputs are still missing.
pub fn send_following(
    count: usize,
    mut send: impl FnMut(usize) -> usize,
    reattach: impl FnOnce() -> bool,
) -> bool {
    let sent = send(0).min(count);
    if sent == count {
        return true;
    }
    if !reattach() {
        return false;
    }
    sent + send(sent) >= count
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
        assert!(send_following(
            3,
            |_| 3,
            || {
                reattached.set(true);
                true
            }
        ));
        assert!(!reattached.get());
    }

    #[test]
    fn a_short_send_on_a_new_desktop_sends_the_rest_once() {
        let mut starts = Vec::new();
        let all = send_following(
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
        assert!(all);
        assert_eq!(starts, [0, 2]);
    }

    #[test]
    fn a_short_send_on_the_same_desktop_is_not_sent_again() {
        let mut calls = 0;
        assert!(!send_following(
            4,
            |_| {
                calls += 1;
                0
            },
            || false
        ));
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_second_short_send_is_reported_and_not_retried_further() {
        let mut calls = 0;
        assert!(!send_following(
            4,
            |_| {
                calls += 1;
                1
            },
            || true
        ));
        assert_eq!(calls, 2);
    }
}
