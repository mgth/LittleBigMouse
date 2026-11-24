//! Hiding this machine's cursor while the pointer is on another one (docs/multi-pc.md).
//!
//! In the multi-PC mode the local pointer is frozen while another machine is driven
//! (`pipeline`), and the compositor keeps drawing it where it stopped. A uinput pointer
//! has no say in what the compositor draws, and Wayland gives a client no global "hide
//! the cursor". What a client may do is choose the cursor over its own surface. So, as
//! the Windows remote does with a one-pixel window (`remote/windows/hider.rs`), a
//! surface of ours goes under the frozen pointer: a few logical pixels on the
//! layer-shell overlay layer, transparent, never taking the keyboard, but with an input
//! region — it needs the pointer's focus, so it cannot be click-through. On
//! `wl_pointer.enter` it sets no cursor. Nothing global changes: the surface is
//! destroyed the moment the pointer comes home, and if this process dies the
//! compositor destroys it with the connection.
//!
//! The pointer is not left on the edge pixel it crossed. That edge is an outer edge of
//! this desktop — the other machines lie beyond it — which is where hot corners, screen
//! edge actions and auto-hidden panels wait for a pointer to rest; and a surface whose
//! last pixel is the pointer's, in a corner, leaves no room for the rounding of a
//! fractional scale. The pointer is moved [`INSET`] logical pixels inside the output it
//! was on, on both axes, rather than to the middle of that output as the Windows remote
//! does: here it moves while the person watches the other screen, and a few pixels is a
//! move nobody sees, even if the compositor draws the cursor for one frame on its way.
//!
//! That move is also what gives the surface the pointer's focus. A compositor picks the
//! surface under the pointer when the pointer moves, and need not when a surface maps
//! under a still one. So the surface is put in place first; once the compositor has
//! configured it, [`CursorHider::spot`] says where the pointer must go, and the pump
//! moves the frozen pointer there on its next cycle: a real motion onto the surface,
//! which brings the `enter`.
//!
//! On an X11 session XFixes hides the cursor of the whole screen for as long as this
//! client asks, and the server shows it again if the client goes away. No move is
//! needed there.
//!
//! The surface lives on a thread of its own, which owns the connection to the display
//! server and sleeps in `poll` until it is asked for something or the display server
//! says something. The routing thread never waits for it: it stores what it wants in an
//! atomic and wakes the thread through an eventfd, and reads the spot back with a
//! `try_lock` it retries on the next cycle. When nothing can hide the cursor — no
//! display server reachable, no layer-shell — one line says so and the cursor stays
//! visible, as it always was.

mod wayland;
mod x11;

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::geometry::Point;
use crate::remote::parking::{screen_of, Screen};

/// How far inside its output the frozen pointer is moved, in logical pixels: off the
/// edge pixel the compositor's edge actions watch, and far enough for the surface
/// around it to fit inside the output.
pub(crate) const INSET: i32 = 4;

/// The side of the hiding surface, in logical pixels. Odd, so the pointer sits on its
/// middle pixel, with one pixel to spare on each side for a fractional scale's rounding
/// of either the surface or the pointer.
pub(crate) const SIDE: i32 = 3;

/// Where the hiding surface goes for a pointer that left from a given point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement {
    /// The output, as an index in the list it was chosen from.
    pub output: usize,
    /// Where the frozen pointer is to sit, in desktop logical pixels.
    pub spot: (i32, i32),
    /// The surface's top-left corner relative to its output's, in that output's
    /// logical pixels: the layer-shell margins of a surface anchored top and left.
    pub left: i32,
    pub top: i32,
}

/// Where to hide a pointer that left this desktop from `from`: on the output it was on,
/// or the nearest one when it is on none (an output just went away), [`INSET`] inside
/// its edges, the surface centred on that spot and kept inside the output. `None` when
/// there is no output. Outputs are in desktop logical pixels, right and bottom excluded:
/// a point on the edge two outputs share belongs to the one it starts.
pub(crate) fn place(from: (i32, i32), outputs: &[Screen]) -> Option<Placement> {
    let output = screen_of(from, outputs)?;
    let screen = outputs[output];
    // One axis: the spot, and the surface's start relative to the output.
    let axis = |v: i32, low: i32, high: i32| -> (i32, i32) {
        let (first, last) = (low.saturating_add(INSET), high.saturating_sub(1 + INSET));
        let spot = if first <= last {
            v.clamp(first, last)
        } else {
            // An output too small to keep the inset: its middle.
            ((i64::from(low) + i64::from(high)) / 2) as i32
        };
        let start = spot
            .saturating_sub(SIDE / 2)
            .clamp(low, high.saturating_sub(SIDE).max(low));
        (spot, start - low)
    };
    let (x, left) = axis(from.0, screen.left, screen.right);
    let (y, top) = axis(from.1, screen.top, screen.bottom);
    Some(Placement {
        output,
        spot: (x, y),
        left,
        top,
    })
}

/// What the routing thread and the hider's thread share.
struct Link {
    /// The latest request: its sequence number shifted left once, the low bit set when
    /// the cursor is to be hidden. One word, so the thread never sees a sequence number
    /// with another request's verdict.
    want: AtomicU64,
    /// Where the pointer left from, packed; written before `want`.
    from: AtomicU64,
    /// The spot the hiding surface is in place under, with the sequence number of the
    /// request it answers: an answer to an older request is not one.
    ready: Mutex<Option<(u64, (i32, i32))>>,
    /// Written to wake the thread.
    wake: OwnedFd,
}

fn pack((x, y): (i32, i32)) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

fn unpack(packed: u64) -> (i32, i32) {
    ((packed >> 32) as u32 as i32, packed as u32 as i32)
}

impl Link {
    fn request(&self, hide: Option<Point<i32>>) -> u64 {
        if let Some(from) = hide {
            self.from
                .store(pack((from.x(), from.y())), Ordering::Release);
        }
        // The routing thread is the only writer: a load and a store are enough.
        let seq = (self.want.load(Ordering::Relaxed) >> 1) + 1;
        self.want
            .store(seq << 1 | u64::from(hide.is_some()), Ordering::Release);
        let one = 1u64.to_ne_bytes();
        // Non-blocking: a counter that would overflow says EAGAIN, and the thread has
        // a wake pending anyway.
        unsafe {
            libc::write(self.wake.as_raw_fd(), one.as_ptr().cast(), one.len());
        }
        seq
    }
}

/// The one link of the process, made at the first arm along with its thread; `None`
/// when no eventfd could be had, and the cursor is then never hidden.
fn link() -> Option<&'static Link> {
    static LINK: OnceLock<Option<&'static Link>> = OnceLock::new();
    *LINK.get_or_init(|| {
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            eprintln!(
                "[LittleBigMouse.Hook] cursor hider: no eventfd ({}); the cursor stays visible while the pointer is on another machine",
                std::io::Error::last_os_error()
            );
            return None;
        }
        let link: &'static Link = Box::leak(Box::new(Link {
            want: AtomicU64::new(0),
            from: AtomicU64::new(0),
            ready: Mutex::new(None),
            wake: unsafe { OwnedFd::from_raw_fd(fd) },
        }));
        let spawned = std::thread::Builder::new()
            .name("lbm-cursor-hider".to_string())
            .spawn(move || serve(link));
        if let Err(e) = spawned {
            eprintln!(
                "[LittleBigMouse.Hook] cursor hider: no thread ({e}); the cursor stays visible while the pointer is on another machine"
            );
            return None;
        }
        Some(link)
    })
}

/// The routing thread's side. Never blocks.
pub(crate) struct CursorHider {
    link: Option<&'static Link>,
    /// The sequence number of the last hide asked for; 0 when the cursor is to show.
    hiding: u64,
}

impl CursorHider {
    /// Starts the hider's thread the first time: to be called where slow things are
    /// allowed (`Router::arm`, before the grabs).
    pub(crate) fn new() -> CursorHider {
        CursorHider {
            link: link(),
            hiding: 0,
        }
    }

    /// The pointer left this machine from `from`: hide the cursor.
    pub(crate) fn hide(&mut self, from: Point<i32>) {
        if let Some(link) = self.link {
            self.hiding = link.request(Some(from));
        }
    }

    /// The pointer is home: show the cursor.
    pub(crate) fn show(&mut self) {
        if let Some(link) = self.link {
            link.request(None);
            self.hiding = 0;
        }
    }

    /// Where the frozen pointer is to go, once the surface hiding it is in place.
    pub(crate) fn spot(&self) -> Option<Point<i32>> {
        let link = self.link.filter(|_| self.hiding != 0)?;
        let ready = *link.ready.try_lock().ok()?;
        ready
            .filter(|&(seq, _)| seq == self.hiding)
            .map(|(_, (x, y))| Point::new(x, y))
    }
}

/// Whatever hides the cursor on this session's display server.
enum Backend {
    Wayland(wayland::Hider),
    X11(x11::Hider),
}

/// Why there is no backend.
enum Missing {
    /// The display server could not be reached now; it may be later.
    Unreachable(String),
    /// It lacks what is needed, and will go on lacking it.
    Unsupported(String),
}

impl Backend {
    fn connect() -> Result<Backend, Missing> {
        let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        if set("WAYLAND_DISPLAY") {
            wayland::Hider::connect().map(Backend::Wayland)
        } else if set("DISPLAY") {
            x11::Hider::connect().map(Backend::X11)
        } else {
            Err(Missing::Unsupported(
                "no display server in the environment".into(),
            ))
        }
    }

    /// Makes the display server match the request; the spot the pointer is to go to
    /// once the surface hiding it is in place. `Err` when the connection is lost.
    fn reconcile(&mut self, hide: Option<(i32, i32)>) -> Result<Option<(i32, i32)>, String> {
        match self {
            Backend::Wayland(hider) => hider.reconcile(hide),
            Backend::X11(hider) => hider.reconcile(hide.is_some()).map(|()| None),
        }
    }

    /// Sleeps until woken through `wake` or the display server says something, and
    /// handles what it said. `Err` when the connection is lost.
    fn wait(&mut self, wake: RawFd) -> Result<(), String> {
        match self {
            Backend::Wayland(hider) => hider.wait(wake),
            Backend::X11(hider) => hider.wait(wake),
        }
    }
}

/// Waits for `fds` to be readable; `revents` filled in.
fn poll(fds: &mut [libc::pollfd]) {
    // EINTR and the like only end the wait early: the caller looks again.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1);
    }
}

fn read_wake(link: &Link) {
    let mut count = [0u8; 8];
    unsafe {
        libc::read(
            link.wake.as_raw_fd(),
            count.as_mut_ptr().cast(),
            count.len(),
        );
    }
}

/// The hider's thread: connects at the first hide, then follows the requests.
fn serve(link: &'static Link) {
    let mut backend: Option<Backend> = None;
    let mut given_up = false;
    let mut told_unreachable = false;
    loop {
        // Drained before the request is read: a request written after this wakes the
        // next wait.
        read_wake(link);
        let word = link.want.load(Ordering::Acquire);
        let (seq, hide) = (word >> 1, word & 1 == 1);
        let from = unpack(link.from.load(Ordering::Acquire));

        if hide && backend.is_none() && !given_up {
            match Backend::connect() {
                Ok(connected) => backend = Some(connected),
                Err(Missing::Unreachable(why)) => {
                    // Tried again at the next crossing, said once.
                    if !told_unreachable {
                        told_unreachable = true;
                        eprintln!(
                            "[LittleBigMouse.Hook] cursor hider: {why}; the cursor stays visible while the pointer is on another machine"
                        );
                    }
                }
                Err(Missing::Unsupported(why)) => {
                    given_up = true;
                    eprintln!(
                        "[LittleBigMouse.Hook] cursor hider: {why}; the cursor stays visible while the pointer is on another machine"
                    );
                }
            }
        }

        let mut spot = None;
        if let Some(connected) = backend.as_mut() {
            match connected.reconcile(hide.then_some(from)) {
                Ok(s) => spot = s,
                Err(why) => {
                    eprintln!("[LittleBigMouse.Hook] cursor hider: connection lost ({why})");
                    backend = None;
                }
            }
        }
        if let Ok(mut ready) = link.ready.lock() {
            *ready = spot.filter(|_| hide).map(|s| (seq, s));
        }

        match backend.as_mut() {
            Some(connected) => {
                if let Err(why) = connected.wait(link.wake.as_raw_fd()) {
                    eprintln!("[LittleBigMouse.Hook] cursor hider: connection lost ({why})");
                    backend = None;
                }
            }
            None => poll(&mut [libc::pollfd {
                fd: link.wake.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            }]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn output(left: i32, top: i32, width: i32, height: i32) -> Screen {
        Screen {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    /// The maintainer's desk in logical pixels: a 3840×2160 panel at 1.25 and, to its
    /// right, a 2560×1440 one at 1.658 (1544×868 once scaled), tops aligned.
    const DESK: [Screen; 2] = [output(0, 0, 3072, 1728), output(3072, 0, 1544, 868)];

    /// The surface's rectangle in desktop pixels, right and bottom excluded.
    fn surface(p: &Placement, outputs: &[Screen]) -> (i32, i32, i32, i32) {
        let o = outputs[p.output];
        let (x, y) = (o.left + p.left, o.top + p.top);
        (x, y, x + SIDE, y + SIDE)
    }

    fn assert_holds(from: (i32, i32), outputs: &[Screen]) {
        let p = place(from, outputs).expect("a placement");
        let o = outputs[p.output];
        let (l, t, r, b) = surface(&p, outputs);
        assert!(
            l <= p.spot.0 && p.spot.0 < r && t <= p.spot.1 && p.spot.1 < b,
            "{from:?}: the spot {:?} is under the surface {:?}",
            p.spot,
            (l, t, r, b)
        );
        assert!(
            o.left <= l && r <= o.right && o.top <= t && b <= o.bottom,
            "{from:?}: the surface {:?} is inside its output {o:?}",
            (l, t, r, b)
        );
    }

    #[test]
    fn a_pointer_that_left_by_the_right_edge_of_a_fractional_output_is_hidden_a_few_pixels_inside_it(
    ) {
        let p = place((4615, 400), &DESK).unwrap();
        assert_eq!(p.output, 1);
        assert_eq!(p.spot, (4611, 400));
        // In the 1.658 output's own logical pixels.
        assert_eq!((p.left, p.top), (1538, 399));
    }

    #[test]
    fn a_pointer_in_a_corner_left_of_and_above_the_origin_is_moved_inside_on_both_axes() {
        let desk = [output(-1920, -300, 1920, 1080), output(0, 0, 3072, 1728)];
        let p = place((-1920, -300), &desk).unwrap();
        assert_eq!(p.output, 0);
        assert_eq!(p.spot, (-1916, -296));
        assert_eq!((p.left, p.top), (3, 3));
        let p = place((-1, 779), &desk).unwrap();
        assert_eq!(p.spot, (-5, 775));
        assert_eq!((p.left, p.top), (1914, 1074));
    }

    #[test]
    fn a_point_on_the_edge_two_outputs_share_belongs_to_the_one_it_starts() {
        let p = place((3072, 10), &DESK).unwrap();
        assert_eq!(p.output, 1);
        assert_eq!(p.spot, (3076, 10));
        assert_eq!((p.left, p.top), (3, 9));
        assert_eq!(place((3071, 10), &DESK).unwrap().output, 0);
    }

    #[test]
    fn a_point_on_no_output_is_hidden_on_the_nearest_one() {
        // Below the smaller output, beside the larger one.
        let p = place((4000, 1500), &DESK).unwrap();
        assert_eq!(p.output, 1);
        assert_eq!(p.spot, (4000, 863));
    }

    #[test]
    fn an_output_too_small_for_the_inset_is_hidden_in_its_middle() {
        let tiny = [output(10, 10, 6, 2)];
        let p = place((10, 10), &tiny).unwrap();
        assert_eq!(p.spot, (13, 11));
        assert_holds((10, 10), &[output(10, 10, 6, 3)]);
    }

    #[test]
    fn no_output_means_nothing_to_hide_on() {
        assert_eq!(place((0, 0), &[]), None);
    }

    #[test]
    fn the_spot_is_always_under_a_surface_inside_its_output() {
        let desk = [
            output(-1544, -868, 1544, 868),
            output(0, 0, 3072, 1728),
            output(3072, 200, 1544, 868),
        ];
        for from in [
            (-1544, -868),
            (-1, -1),
            (0, 0),
            (3071, 1727),
            (3072, 200),
            (4615, 1067),
            (5000, 5000),
            (-9000, 0),
        ] {
            assert_holds(from, &desk);
        }
    }

    #[test]
    fn a_point_packs_and_unpacks_to_itself() {
        for p in [(0, 0), (-1916, -296), (i32::MIN, i32::MAX)] {
            assert_eq!(unpack(pack(p)), p);
        }
    }
}
