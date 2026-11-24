//! The crossing rules driven the way a master drives them, against a platform that
//! records what it is asked: no device, no engine, no input stream.
//!
//! Where the pointer goes is given, as the engine of a master would have located it:
//! these tests are about what follows from it. The same rules driven through the
//! evdev pipeline, engine included, are tested there.

use std::sync::atomic::Ordering;
use std::time::Instant;

use lbm_ipc::input::{decode, InputEvent as Wire};

use super::keys::HOLD;
use super::*;

const START: (i32, i32) = (1900, 500);
/// Where the pointer lands on the other machine, in its own pixels.
const LANDING: (i32, i32) = (0, 500);
const BTN_LEFT: u16 = 0x110;
const KEY_A: u16 = 30;
const KEY_M: u16 = 50;
const KEY_LEFTCTRL: u16 = 29;
const KEY_LEFTALT: u16 = 56;
const KEY_LEFTSHIFT: u16 = 42;

fn point((x, y): (i32, i32)) -> Point<i32> {
    Point::new(x, y)
}

/// What the rules asked of the platform. The frame being dispatched is a list of
/// buttons and keys, and its wheels.
struct Recording {
    /// The platform's pointer on this machine.
    here: Point<i32>,
    frame: Vec<Input>,
    wheels: (i32, i32),
    /// Each frame of this machine's pointer: where, and the buttons it carried.
    pointer: Vec<((i32, i32), Vec<Input>)>,
    keyboard: Vec<Vec<Input>>,
    /// Each time the pointer was put back.
    returned: Vec<(i32, i32)>,
    connected: bool,
    accepting: bool,
    client: u64,
    sent: Vec<Wire>,
    /// The keyboards are grabbed: asked for, and not let go since.
    grabbed: bool,
    releases: usize,
    /// Each hide: where from, and how many events had been sent by then.
    hides: Vec<((i32, i32), usize)>,
    shows: usize,
    /// What hides the cursor is in place, under this spot.
    spot: Option<Point<i32>>,
}

impl Platform for Recording {
    fn remote_client(&self) -> Option<u64> {
        self.connected.then_some(self.client)
    }

    fn send_remote(&mut self, batch: Vec<u8>) -> bool {
        if !(self.connected && self.accepting) {
            return false;
        }
        self.sent
            .extend(decode(&batch).expect("a batch that decodes"));
        true
    }

    fn grab_keyboards(&mut self) {
        self.grabbed = true;
    }

    fn release_keyboards(&mut self) {
        self.grabbed = false;
        self.releases += 1;
    }

    fn hide_cursor(&mut self, from: Point<i32>) {
        self.hides.push(((from.x(), from.y()), self.sent.len()));
    }

    fn cursor_hiding_spot(&mut self) -> Option<Point<i32>> {
        self.spot
    }

    fn show_cursor(&mut self) {
        self.shows += 1;
    }

    fn pointer(&self) -> Point<i32> {
        self.here
    }

    fn return_pointer(&mut self, home: Point<i32>) {
        self.here = home;
        self.returned.push((home.x(), home.y()));
    }

    fn emit_pointer(&mut self, at: Point<i32>) {
        let buttons = self
            .frame
            .iter()
            .copied()
            .filter(|input| matches!(input, Input::Button(..)))
            .collect();
        self.frame
            .retain(|input| !matches!(input, Input::Button(..)));
        self.wheels = (0, 0);
        self.pointer.push(((at.x(), at.y()), buttons));
    }

    fn pointer_pending(&self) -> bool {
        self.wheels != (0, 0)
            || self
                .frame
                .iter()
                .any(|input| matches!(input, Input::Button(..)))
    }

    fn emit_keyboard(&mut self) {
        let keys: Vec<Input> = self
            .frame
            .iter()
            .copied()
            .filter(|input| matches!(input, Input::Key(..)))
            .collect();
        if !keys.is_empty() {
            self.frame.retain(|input| !matches!(input, Input::Key(..)));
            self.keyboard.push(keys);
        }
    }

    fn take_remote_events(&mut self, mut take: impl FnMut(Input) -> bool) -> (i32, i32) {
        self.frame.retain(|&input| !take(input));
        std::mem::take(&mut self.wheels)
    }
}

struct Rig {
    shared: &'static Shared,
    crossing: Crossing,
    m: Recording,
}

impl Rig {
    fn new(connected: bool) -> Rig {
        let shared: &'static Shared = Box::leak(Box::new(Shared::new()));
        shared.want_hook.store(true, Ordering::SeqCst);
        Rig {
            shared,
            crossing: Crossing::new(point(START)),
            m: Recording {
                here: point(START),
                frame: Vec::new(),
                wheels: (0, 0),
                pointer: Vec::new(),
                keyboard: Vec::new(),
                returned: Vec::new(),
                connected,
                accepting: true,
                client: 1,
                sent: Vec::new(),
                grabbed: false,
                releases: 0,
                hides: Vec::new(),
                shows: 0,
                spot: None,
            },
        }
    }

    /// One cycle of a master: catch up, dispatch the frame — when there is one, that
    /// is when the pointer moved or something was pressed — and end the cycle.
    fn cycle(&mut self, place: Option<Place>, inputs: &[Input]) {
        self.crossing.begin_cycle(self.shared, &mut self.m);
        if place.is_some() || !inputs.is_empty() {
            self.m.frame = inputs.to_vec();
            self.crossing.dispatch(self.shared, place, &mut self.m);
        }
        self.crossing.end_cycle(self.shared, &mut self.m);
    }

    /// The pointer crossed to the other machine.
    fn go_over(&mut self) {
        self.cycle(
            Some(Place::Enter {
                pixel: point(LANDING),
                host: "abcd".to_owned(),
            }),
            &[],
        );
        assert!(self.crossing.is_remote(), "the pointer went over");
    }

    /// The pointer reached the other machine, whose `Enter` could not be queued.
    fn go_over_refused(&mut self) {
        self.cycle(
            Some(Place::Enter {
                pixel: point(LANDING),
                host: "abcd".to_owned(),
            }),
            &[],
        );
        assert!(!self.crossing.is_remote(), "an Enter nobody took is a wall");
    }

    /// The pointer moved to `pixel` of the machine it is on.
    fn move_to(&mut self, pixel: (i32, i32)) {
        self.cycle(Some(Place::Remote(point(pixel))), &[]);
    }

    /// The pointer crossed back, landing at `here` on this machine.
    fn come_back(&mut self, here: (i32, i32)) {
        self.m.here = point(here);
        self.cycle(Some(Place::Local), &[]);
        assert!(!self.crossing.is_remote(), "the pointer came back");
    }

    /// A key of a keyboard. These keyboards are grabbed whenever the rules ask, and
    /// only then; a key they do not grab is the compositor's.
    fn type_key(&mut self, code: u16, press: Press) {
        if self.m.grabbed {
            self.crossing.grabbed_key(code, press, Instant::now());
        }
        self.crossing.flush_keys(self.shared, &mut self.m);
    }

    /// Where the last frame of this machine's pointer put it.
    fn local_cursor(&self) -> (i32, i32) {
        self.m.pointer.last().expect("a pointer frame").0
    }
}

#[test]
fn keys_go_to_the_other_machine_only_while_the_pointer_is_there() {
    let mut rig = Rig::new(true);
    rig.type_key(KEY_A, Press::Down);
    rig.type_key(KEY_A, Press::Up);
    assert!(rig.m.sent.is_empty(), "typed here: {:?}", rig.m.sent);

    rig.go_over();
    rig.type_key(KEY_A, Press::Down);
    rig.type_key(KEY_A, Press::Repeat);
    rig.type_key(KEY_A, Press::Up);
    assert_eq!(
        rig.m.sent[1..],
        [
            Wire::Key {
                code: KEY_A,
                pressed: true
            },
            Wire::Key {
                code: KEY_A,
                pressed: true
            },
            Wire::Key {
                code: KEY_A,
                pressed: false
            },
        ]
    );
}

#[test]
fn a_stream_that_takes_no_more_brings_the_pointer_home() {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.m.accepting = false;
    rig.move_to((10, 500));

    assert!(!rig.crossing.is_remote());
    assert_eq!(rig.m.releases, 1);
    assert_eq!(rig.m.shows, 1);
    assert_eq!(rig.m.returned, [START]);
    assert_eq!(rig.local_cursor(), START);
}

#[test]
fn a_client_replaced_while_the_pointer_is_over_there_brings_it_home_with_nothing_but_a_leave() {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.type_key(KEY_A, Press::Down);
    // Between two cycles: the stream never looked unconnected.
    rig.m.client = 2;
    rig.m.sent.clear();
    rig.cycle(None, &[]);

    assert!(!rig.crossing.is_remote());
    assert!(!rig.m.grabbed, "the keyboards are let go");
    assert_eq!(rig.m.shows, 1, "the cursor is shown again");
    assert_eq!(rig.local_cursor(), START, "back where it left");
    assert!(
        rig.m.sent.iter().all(|event| matches!(event, Wire::Leave)),
        "the new client got {:?} without an Enter",
        rig.m.sent
    );
}

#[test]
fn going_away_brings_the_pointer_home_first() {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.crossing.shut_down(&mut rig.m);
    assert!(!rig.crossing.is_remote());
    assert_eq!(rig.m.sent.last(), Some(&Wire::Leave));
    assert_eq!(rig.m.releases, 1);
    assert_eq!(rig.m.shows, 1);
    assert_eq!(rig.local_cursor(), START);
}

#[test]
fn the_rescue_held_over_there_brings_the_pointer_home_and_never_sends_its_key() {
    let mut rig = Rig::new(true);
    rig.go_over();
    for code in [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT] {
        rig.type_key(code, Press::Down);
    }
    rig.type_key(KEY_M, Press::Down);
    rig.type_key(KEY_M, Press::Repeat);
    rig.crossing
        .check_rescue(rig.shared, Instant::now() + HOLD, &mut rig.m);

    assert!(!rig.crossing.is_remote());
    assert!(
        !rig.shared.want_hook.load(Ordering::SeqCst),
        "the rescue's unhook was asked for"
    );
    assert_eq!(rig.m.sent.last(), Some(&Wire::Leave));
    assert!(
        !rig.m
            .sent
            .iter()
            .any(|e| matches!(e, Wire::Key { code: KEY_M, .. })),
        "{:?}",
        rig.m.sent
    );
    assert_eq!(rig.m.releases, 1);
    assert_eq!(rig.m.shows, 1);
}

#[test]
fn a_button_pressed_over_there_is_released_there_by_the_leave() {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.cycle(None, &[Input::Button(BTN_LEFT, Press::Down)]);
    rig.come_back((1919, 500));
    rig.cycle(None, &[Input::Button(BTN_LEFT, Press::Up)]);

    assert_eq!(
        rig.m.sent,
        [
            Wire::Enter { x: 0, y: 500 },
            Wire::Button {
                code: BTN_LEFT,
                pressed: true
            },
            Wire::Leave,
        ]
    );
    // The release nobody over there is waiting for any more is handed to this
    // machine's pointer, whose device drops what it never had pressed.
    assert_eq!(
        rig.m.pointer.last(),
        Some(&((1919, 500), vec![Input::Button(BTN_LEFT, Press::Up)]))
    );
}

#[test]
fn a_key_held_over_there_is_released_there_by_the_leave_and_not_again() {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.type_key(KEY_LEFTCTRL, Press::Down);
    rig.come_back((1919, 500));
    rig.type_key(KEY_LEFTCTRL, Press::Up);

    assert_eq!(
        rig.m.sent,
        [
            Wire::Enter { x: 0, y: 500 },
            Wire::Key {
                code: KEY_LEFTCTRL,
                pressed: true
            },
            Wire::Leave,
        ]
    );
}

#[test]
fn a_button_pressed_here_is_released_here_where_the_pointer_froze() {
    let mut rig = Rig::new(true);
    rig.cycle(None, &[Input::Button(BTN_LEFT, Press::Down)]);
    rig.go_over();
    rig.cycle(None, &[Input::Button(BTN_LEFT, Press::Up)]);

    assert!(
        !rig.m.sent.iter().any(|e| matches!(e, Wire::Button { .. })),
        "{:?}",
        rig.m.sent
    );
    assert_eq!(
        rig.m.pointer.last(),
        Some(&(START, vec![Input::Button(BTN_LEFT, Press::Up)]))
    );
}

#[test]
fn the_cursor_is_hidden_once_on_crossing_out_after_the_enter_is_queued() {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.move_to((10, 504));
    rig.move_to((20, 508));
    // From where the pointer last was here; the Enter was the first thing sent.
    assert_eq!(rig.m.hides, [(START, 1)]);
    assert_eq!(rig.m.shows, 0);
}

#[test]
fn an_enter_nobody_takes_is_a_wall_and_hides_nothing() {
    let mut rig = Rig::new(true);
    rig.m.accepting = false;
    rig.go_over_refused();
    assert!(rig.m.hides.is_empty(), "{:?}", rig.m.hides);
    assert_eq!(rig.m.shows, 0);
    assert!(!rig.m.grabbed);
    assert_eq!(rig.m.returned, [START]);
}

#[test]
fn the_frozen_pointer_moves_onto_what_hides_it_once_it_is_in_place_and_comes_home_from_where_it_left(
) {
    let mut rig = Rig::new(true);
    rig.go_over();
    rig.move_to((5, 500));
    assert!(rig.m.pointer.is_empty(), "nothing in place yet");

    rig.m.spot = Some(Point::new(1915, 500));
    rig.cycle(None, &[]);
    assert_eq!(rig.local_cursor(), (1915, 500));
    rig.cycle(None, &[]);
    rig.move_to((10, 500));
    assert_eq!(rig.m.pointer.len(), 1, "moved there once");

    // Home is where it left, not where it hid.
    rig.crossing.shut_down(&mut rig.m);
    assert_eq!(rig.local_cursor(), START);
}
