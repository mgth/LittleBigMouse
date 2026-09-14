//! Display changes the hook does not report — the poll of C#'s `LinuxLayoutFactory`,
//! woken by the events that announce them.
//!
//! What is checked: the sysfs plug signature (a plug, an unplug, a monitor swapped),
//! and the modification time of the files the desktops rewrite the moment their
//! output layout changes — KWin's `kwinoutputconfig.json` (Plasma 6), mutter's
//! `monitors.xml` — for what the plug signature cannot see (a move, a scale, an
//! output switched off). A spurious rewrite settles to the same display signature and
//! is absorbed by the reconciler's idempotence guard.
//!
//! When: at once when the kernel announces a DRM hotplug (netlink uevent) or one of
//! those files is written (inotify on its directory), and every two seconds as in C#
//! while either source is missing. With both up the timer is only a safety net. An
//! event only brings the (cheap) check forward: the check still decides.

use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lbm_display::linux::drm;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;

use crate::reconcile::Input;

/// C#: `LinuxLayoutFactory.PollInterval` — the cadence while an event source is missing.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The cadence once both event sources are up: a safety net.
pub const SAFETY_NET: Duration = Duration::from_secs(30);

/// How long the events of one change are left to gather before the check: a plug is a
/// burst of uevents (per connector, per GPU), an output change a burst of writes.
pub const COALESCE: Duration = Duration::from_millis(250);

/// How often a missing event source is tried again.
///
/// The loop already ticks every two seconds while one is down, but asking the kernel for
/// a descriptor thirty times a minute to be told the same thing is not a retry, it is a
/// spin. What takes an event source away — a file indexer holding every inotify watch —
/// is something a person fixes on a human timescale, and until they do the two-second
/// poll is doing the work.
pub const RETRY: Duration = Duration::from_secs(60);

/// The event sources. Linux has two; no other platform here has any, so everything
/// that keeps them alive is Linux-only and the poll is all the others have.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The desktops' output files being rewritten.
    Inotify = 0,
    /// The kernel announcing a DRM hotplug.
    Uevents = 1,
}

#[cfg(target_os = "linux")]
impl Source {
    const ALL: [Source; 2] = [Source::Inotify, Source::Uevents];

    fn name(self) -> &'static str {
        match self {
            Source::Inotify => "inotify on the output files",
            Source::Uevents => "DRM uevents",
        }
    }
}

/// Which event sources are up, and whether their absence has been said out loud.
///
/// A flag each rather than a count, because bringing a source back needs to know *which*
/// one is missing and a count cannot say. The second pair is what keeps a machine that
/// stays saturated from writing the same line into the log every minute: a loss is news
/// once, and so is a recovery.
#[cfg(target_os = "linux")]
#[derive(Debug, Default)]
pub struct Live {
    up: [AtomicBool; 2],
    told: [AtomicBool; 2],
}

#[cfg(target_os = "linux")]
impl Live {
    /// Both sources are up, so the timer is only a safety net.
    pub fn all_up(&self) -> bool {
        Source::ALL.iter().all(|s| self.up(*s))
    }

    pub fn up(&self, source: Source) -> bool {
        self.up[source as usize].load(Ordering::Relaxed)
    }

    /// `source` is up. `true` if it had been lost and complained about, which is worth
    /// saying too — a log that says only that things break never says they were fixed.
    fn came_up(&self, source: Source) -> bool {
        self.up[source as usize].store(true, Ordering::Relaxed);
        self.told[source as usize].swap(false, Ordering::Relaxed)
    }

    /// `source` is down. Its reader task calls this on the way out.
    fn went(&self, source: Source) {
        self.up[source as usize].store(false, Ordering::Relaxed);
    }

    /// `source` could not be raised. `true` the first time since it was last up, which is
    /// when it is worth saying out loud.
    fn lost(&self, source: Source) -> bool {
        !self.told[source as usize].swap(true, Ordering::Relaxed)
    }
}

/// Whether a missing source should be tried again now, `since` the last attempt.
///
/// A function rather than the condition written inline in the loop, because "the retry
/// never fires" is the bug this whole thing exists to prevent and it should not be
/// something only a running agent can disprove.
#[cfg(target_os = "linux")]
fn retry_due(live: &Live, since: Duration) -> bool {
    !live.all_up() && since >= RETRY
}

/// Holds a source up for as long as its reader lives, and marks it down when that ends.
///
/// A guard rather than a line at the bottom of the reader's loop, because that line is
/// the **trigger for the whole retry** and every way of leaving the loop has to run it —
/// a panic in the relevance test, or the task dropped at shutdown, would otherwise leave
/// the source marked up with nobody reading it, and the retry would never fire.
///
/// Deleting the line was the one perturbation the tests did not catch, which is what
/// turned it into a type.
#[cfg(target_os = "linux")]
pub struct Reader {
    source: Source,
    live: Arc<Live>,
}

#[cfg(target_os = "linux")]
impl Reader {
    /// Marks `source` up for as long as the guard is held.
    ///
    /// `true` from [`Live::came_up`] means it had been complained about, so its return is
    /// worth a line too.
    fn new(source: Source, live: Arc<Live>) -> (Reader, bool) {
        let recovered = live.came_up(source);
        (Reader { source, live }, recovered)
    }
}

#[cfg(target_os = "linux")]
impl Drop for Reader {
    fn drop(&mut self) {
        self.live.went(self.source);
    }
}

/// C#: `OutputConfigPaths`, in the XDG configuration directory.
fn output_config_paths() -> Vec<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")));
    config
        .map(|dir| vec![dir.join("kwinoutputconfig.json"), dir.join("monitors.xml")])
        .unwrap_or_default()
}

/// The latest modification of the output configuration files; `None` when none exists.
fn output_config_stamp(paths: &[PathBuf]) -> Option<SystemTime> {
    paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
}

/// What one tick found.
#[derive(Debug, Default)]
pub struct Poll {
    plug: String,
    stamp: Option<SystemTime>,
    started: bool,
}

impl Poll {
    /// One tick: whether the displays changed since the last. The first tick only
    /// records (C#: the stamp counts once it is known).
    pub fn tick(&mut self, plug: String, stamp: Option<SystemTime>) -> bool {
        let changed = self.started && (plug != self.plug || stamp != self.stamp);
        self.plug = plug;
        self.stamp = stamp;
        self.started = true;
        changed
    }
}

/// Watches until `inputs` is closed, sending [`Input::DisplayChanged`] on a change.
pub async fn watch_displays(inputs: UnboundedSender<Input>) {
    let paths = output_config_paths();
    let nudge = Arc::new(Notify::new());
    #[cfg(target_os = "linux")]
    let live = Arc::new(Live::default());
    #[cfg(target_os = "linux")]
    events::raise(&paths, &nudge, &live);
    #[cfg(target_os = "linux")]
    let mut tried = tokio::time::Instant::now();

    let mut poll = Poll::default();
    loop {
        let plug = tokio::task::spawn_blocking(drm::plug_signature)
            .await
            .unwrap_or_default();
        if poll.tick(plug, output_config_stamp(&paths))
            && inputs.send(Input::DisplayChanged).is_err()
        {
            return;
        }
        if inputs.is_closed() {
            return;
        }
        // A source that is down is tried again from time to time. Without this the agent
        // that started while a file indexer held every inotify watch stays on its
        // two-second poll until it is restarted — long after the indexer was closed,
        // which is a thing that happens the same afternoon.
        #[cfg(target_os = "linux")]
        let period = {
            if retry_due(&live, tried.elapsed()) {
                events::raise(&paths, &nudge, &live);
                tried = tokio::time::Instant::now();
            }
            if live.all_up() {
                SAFETY_NET
            } else {
                POLL_INTERVAL
            }
        };
        // No event source on this platform, so the timer is the only thing that notices.
        #[cfg(not(target_os = "linux"))]
        let period = POLL_INTERVAL;
        tokio::select! {
            _ = tokio::time::sleep(period) => {}
            _ = nudge.notified() => tokio::time::sleep(COALESCE).await,
        }
    }
}

/// The kernel's and the desktop's announcements, as nudges to check now.
#[cfg(target_os = "linux")]
mod events {
    use std::collections::BTreeSet;
    use std::ffi::{CString, OsString};
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::sync::Arc;

    use tokio::io::unix::AsyncFd;
    use tokio::sync::Notify;

    use super::{Live, Reader, Source, POLL_INTERVAL, RETRY};

    /// Brings up whichever source is not up, and says so only when the answer changes.
    ///
    /// Called once at the start and again every [`RETRY`] while one is missing. A source
    /// already up is left alone: a second reader on the same events would double every
    /// nudge, and `Live` would then be wrong about it the first time one of the two died.
    pub fn raise(paths: &[PathBuf], nudge: &Arc<Notify>, live: &Arc<Live>) {
        if !live.up(Source::Inotify) {
            let (dirs, names) = watched(paths);
            match inotify(&dirs) {
                Ok(fd) => spawn(
                    Source::Inotify,
                    fd,
                    read,
                    move |buf| touches(buf, &names),
                    nudge,
                    live,
                ),
                Err(error) => complain(live, Source::Inotify, &error),
            }
        }
        if !live.up(Source::Uevents) {
            match uevents() {
                Ok(fd) => spawn(Source::Uevents, fd, read, is_display_uevent, nudge, live),
                Err(error) => complain(live, Source::Uevents, &error),
            }
        }
    }

    /// Says a source could not be raised — once, until it comes back.
    fn complain(live: &Arc<Live>, source: Source, error: &io::Error) {
        if live.lost(source) {
            eprintln!(
                "[lbm-agent] no {}: {error}. Checking every {:?} instead; trying again \
                 every {:?}.",
                source.name(),
                POLL_INTERVAL,
                RETRY
            );
        }
    }

    /// The directories to watch and the names that matter in them: each file's own, and
    /// its target's when it is a link (a dotfile manager's), which the desktop rewrites
    /// in its own directory.
    pub fn watched(paths: &[PathBuf]) -> (BTreeSet<PathBuf>, BTreeSet<OsString>) {
        let mut dirs = BTreeSet::new();
        let mut names = BTreeSet::new();
        for path in paths {
            let target = std::fs::canonicalize(path).ok();
            for file in std::iter::once(path.as_path()).chain(target.as_deref()) {
                if let (Some(dir), Some(name)) = (file.parent(), file.file_name()) {
                    dirs.insert(dir.to_path_buf());
                    names.insert(name.to_os_string());
                }
            }
        }
        (dirs, names)
    }

    fn spawn(
        source: Source,
        fd: OwnedFd,
        read: fn(RawFd, &mut [u8]) -> io::Result<usize>,
        relevant: impl Fn(&[u8]) -> bool + Send + 'static,
        nudge: &Arc<Notify>,
        live: &Arc<Live>,
    ) {
        let (nudge, live) = (nudge.clone(), live.clone());
        let fd = match AsyncFd::new(fd) {
            Ok(fd) => fd,
            // The descriptor exists but the reactor would not take it. Said like any
            // other failure to raise it, and tried again with the rest.
            Err(error) => return complain(&live, source, &error),
        };
        // The guard marks the source up now and down when this task ends, whichever way
        // it ends.
        let (reader, recovered) = Reader::new(source, live);
        if recovered {
            eprintln!("[lbm-agent] {} is back", source.name());
        }
        tokio::spawn(async move {
            let _reader = reader;
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                let Ok(mut ready) = fd.readable().await else {
                    break;
                };
                match ready.try_io(|fd| read(fd.as_raw_fd(), &mut buf)) {
                    Ok(Ok(n)) => {
                        if relevant(&buf[..n]) {
                            nudge.notify_one();
                        }
                    }
                    // Events were lost: whatever they were, check.
                    Ok(Err(e)) if e.raw_os_error() == Some(libc::ENOBUFS) => nudge.notify_one(),
                    Ok(Err(e)) if e.kind() == io::ErrorKind::Interrupted => {}
                    Ok(Err(error)) => {
                        eprintln!("[lbm-agent] a display event source failed: {error}");
                        break;
                    }
                    Err(_would_block) => {}
                }
            }
            // `_reader` drops here: back to the two-second poll, and to being tried
            // again every `RETRY`.
        });
    }

    fn read(fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: `buf` is valid for `buf.len()` bytes, `fd` is open for the call.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    //==================//
    // inotify          //
    //==================//

    /// Every way a file comes to change in a directory: written in place, replaced by a
    /// rename (how KWin and mutter save), created, removed, touched.
    const MASK: u32 = libc::IN_CLOSE_WRITE
        | libc::IN_MODIFY
        | libc::IN_ATTRIB
        | libc::IN_CREATE
        | libc::IN_DELETE
        | libc::IN_MOVED_FROM
        | libc::IN_MOVED_TO;

    /// An inotify descriptor watching `dirs`; an error if none can be watched.
    ///
    /// `ENOSPC` here is not a full disk: it is this user's `fs.inotify.max_user_watches`
    /// spent, usually by a file indexer that watches the whole home directory. The agent
    /// survives it — [`start`] leaves `live` short of `SOURCES`, so the loop keeps its
    /// two-second poll instead of the thirty-second safety net — but it loses the instant
    /// reaction to a desktop saving its outputs.
    pub fn inotify(dirs: &BTreeSet<PathBuf>) -> io::Result<OwnedFd> {
        // SAFETY: plain syscall; the descriptor is owned right after.
        let raw = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a fresh descriptor nobody else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let mut error = io::Error::from(io::ErrorKind::NotFound);
        let mut watching = false;
        for dir in dirs {
            let Ok(path) = CString::new(dir.as_os_str().as_bytes()) else {
                continue;
            };
            // SAFETY: `path` is NUL-terminated and outlives the call.
            if unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), MASK) } >= 0 {
                watching = true;
            } else {
                error = io::Error::last_os_error();
            }
        }
        if watching {
            Ok(fd)
        } else {
            Err(error)
        }
    }

    /// Whether a read of `struct inotify_event`s names one of `names` (or reports that
    /// events were dropped).
    pub fn touches(mut buf: &[u8], names: &BTreeSet<OsString>) -> bool {
        const HEADER: usize = 16; // wd, mask, cookie, len
        let mut touched = false;
        while buf.len() >= HEADER {
            let field = |at: usize| u32::from_ne_bytes(buf[at..at + 4].try_into().unwrap());
            let (mask, len) = (field(4), field(12) as usize);
            let Some(name) = buf.get(HEADER..HEADER + len) else {
                break;
            };
            let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(len)];
            touched |= mask & libc::IN_Q_OVERFLOW != 0
                || names.contains(std::ffi::OsStr::from_bytes(name));
            buf = &buf[HEADER + len..];
        }
        touched
    }

    //==================//
    // uevents          //
    //==================//

    /// The kernel's uevent multicast group (udev's is 2).
    const KERNEL_GROUP: u32 = 1;

    /// A socket on the kernel's uevents; receiving them needs no privilege.
    pub fn uevents() -> io::Result<OwnedFd> {
        // SAFETY: plain syscall; the descriptor is owned right after.
        let raw = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                libc::NETLINK_KOBJECT_UEVENT,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a fresh descriptor nobody else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: an all-zero sockaddr_nl is valid; the fields that matter are set below.
        let mut address: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        address.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        address.nl_groups = KERNEL_GROUP;
        // SAFETY: `address` is a sockaddr_nl of the size given.
        let bound = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&raw const address).cast(),
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if bound < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(fd)
    }

    /// Whether a kernel uevent (`action@devpath` then `KEY=value`, NUL-separated) is one
    /// the displays may have changed on: a DRM hotplug (a connector's status, a card
    /// probed), or a DRM device appearing or going (a card, an MST connector).
    pub fn is_display_uevent(message: &[u8]) -> bool {
        let mut fields = message.split(|b| *b == 0).skip(1);
        let (mut drm, mut hotplug, mut appears) = (false, false, false);
        for field in &mut fields {
            match field {
                b"SUBSYSTEM=drm" => drm = true,
                b"HOTPLUG=1" => hotplug = true,
                b"ACTION=add" | b"ACTION=remove" => appears = true,
                _ => {}
            }
        }
        drm && (hotplug || appears)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_a_different_plug_or_a_newer_output_file() {
        let t0 = SystemTime::UNIX_EPOCH;
        let t1 = t0 + Duration::from_secs(1);
        let mut poll = Poll::default();
        assert!(!poll.tick("a".into(), Some(t0)), "the first tick records");
        assert!(!poll.tick("a".into(), Some(t0)));
        assert!(poll.tick("b".into(), Some(t0)), "plugged");
        assert!(
            poll.tick("b".into(), Some(t1)),
            "the compositor rewrote its outputs"
        );
        assert!(!poll.tick("b".into(), Some(t1)));
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use std::collections::BTreeSet;
        use std::ffi::OsString;

        use super::super::events::{self, is_display_uevent, touches, watched};
        use super::super::*;

        fn event(mask: u32, name: &str) -> Vec<u8> {
            let mut padded = name.as_bytes().to_vec();
            padded.resize(name.len().div_ceil(16) * 16 + 16, 0);
            let mut e = Vec::new();
            e.extend(1i32.to_ne_bytes());
            e.extend(mask.to_ne_bytes());
            e.extend(0u32.to_ne_bytes());
            e.extend((padded.len() as u32).to_ne_bytes());
            e.extend(padded);
            e
        }

        #[test]
        fn an_inotify_read_matters_when_it_names_an_output_file() {
            let names = BTreeSet::from([OsString::from("kwinoutputconfig.json")]);
            let mut buf = event(libc::IN_CLOSE_WRITE, "kwinrc");
            assert!(!touches(&buf, &names));
            buf.extend(event(libc::IN_MOVED_TO, "kwinoutputconfig.json"));
            assert!(
                touches(&buf, &names),
                "saved by a rename, after another file"
            );
            assert!(
                touches(&event(libc::IN_Q_OVERFLOW, ""), &names),
                "events were lost"
            );
            assert!(
                !touches(&buf[..20], &names),
                "a torn read is not guessed at"
            );
        }

        #[test]
        fn a_drm_hotplug_or_a_drm_device_coming_or_going_is_a_display_event() {
            let hotplug = b"change@/devices/pci0000:00/0000:00:01.0/drm/card1\0ACTION=change\0DEVPATH=/devices/pci0000:00/0000:00:01.0/drm/card1\0SUBSYSTEM=drm\0HOTPLUG=1\0CONNECTOR=95\0DEVNAME=dri/card1\0SEQNUM=4711\0";
            assert!(is_display_uevent(hotplug));
            let mst = b"add@/devices/pci0000:00/0000:00:01.0/drm/card1/card1-DP-5\0ACTION=add\0DEVPATH=/devices/pci0000:00/0000:00:01.0/drm/card1/card1-DP-5\0SUBSYSTEM=drm\0DEVTYPE=drm_connector\0SEQNUM=4712\0";
            assert!(is_display_uevent(mst));
            let usb = b"add@/devices/pci0000:00/usb1/1-1\0ACTION=add\0SUBSYSTEM=usb\0SEQNUM=4713\0";
            assert!(!is_display_uevent(usb));
            let property = b"change@/devices/pci0000:00/0000:00:01.0/drm/card1\0ACTION=change\0SUBSYSTEM=drm\0SEQNUM=4714\0";
            assert!(!is_display_uevent(property), "a DRM change without HOTPLUG");
        }

        #[test]
        fn the_kernel_uevents_are_followed_without_privilege() {
            events::uevents().expect("a uevent socket");
        }

        #[test]
        fn a_linked_output_file_is_watched_where_its_target_lives() {
            let home = tempfile::tempdir().unwrap();
            let config = home.path().join("config");
            let dotfiles = home.path().join("dotfiles");
            std::fs::create_dir_all(&config).unwrap();
            std::fs::create_dir_all(&dotfiles).unwrap();
            std::fs::write(dotfiles.join("monitors.xml"), "<monitors/>").unwrap();
            std::os::unix::fs::symlink(dotfiles.join("monitors.xml"), config.join("monitors.xml"))
                .unwrap();
            let (dirs, names) = watched(&[
                config.join("kwinoutputconfig.json"),
                config.join("monitors.xml"),
            ]);
            let dotfiles = std::fs::canonicalize(dotfiles).unwrap();
            assert_eq!(dirs, BTreeSet::from([config.clone(), dotfiles]));
            assert_eq!(
                names,
                BTreeSet::from([
                    OsString::from("kwinoutputconfig.json"),
                    OsString::from("monitors.xml")
                ])
            );
        }

        /// The real thing: a desktop saving its outputs (a write, then a rename over the
        /// file) wakes the watch at once, well before the timer would.
        #[tokio::test]
        async fn saving_the_output_file_brings_the_check_forward() {
            let config = tempfile::tempdir().unwrap();
            let file = config.path().join("kwinoutputconfig.json");
            std::fs::write(&file, "[]").unwrap();

            let nudge = Arc::new(Notify::new());
            let live = Arc::new(Live::default());
            events::raise(std::slice::from_ref(&file), &nudge, &live);
            // Asked of inotify by name. `raise` brings up **two** sources, and until
            // `Live` carried a flag each this could only be asked as "how many came up" —
            // so on a machine with no inotify watches left the uevent socket alone
            // answered 1, the assertion passed claiming "inotify is up", and the failure
            // came a second later as `Elapsed`, naming nothing. The reason is on stderr,
            // which cargo prints for a failing test.
            assert!(
                live.up(Source::Inotify),
                "no inotify watch to test with. ENOSPC above is this user's \
                 fs.inotify.max_user_watches spent, not a full disk — look for a file \
                 indexer holding them. The agent itself survives this: it keeps its \
                 two-second poll and only loses the instant reaction."
            );

            let staged = config.path().join("kwinoutputconfig.json.tmp");
            std::fs::write(&staged, r#"[{"name":"outputs"}]"#).unwrap();
            std::fs::rename(&staged, &file).unwrap();
            tokio::time::timeout(Duration::from_secs(1), nudge.notified())
                .await
                .expect("nudged");
        }

        /// The point of the retry: a source that went away comes back, and still works.
        ///
        /// Losing one for real needs a saturated machine, so the loss is made the way the
        /// reader task makes it — `Live::went`, the one line it runs on its way out.
        /// What is being tested is what `raise` does *afterwards*.
        #[tokio::test]
        async fn a_source_that_went_away_is_raised_again_and_works() {
            let config = tempfile::tempdir().unwrap();
            let file = config.path().join("kwinoutputconfig.json");
            std::fs::write(&file, "[]").unwrap();
            let nudge = Arc::new(Notify::new());
            let live = Arc::new(Live::default());

            events::raise(std::slice::from_ref(&file), &nudge, &live);
            assert!(live.up(Source::Inotify), "no inotify watch to test with");

            live.went(Source::Inotify);
            assert!(!live.all_up(), "the loop would not know to retry");

            events::raise(std::slice::from_ref(&file), &nudge, &live);
            assert!(live.up(Source::Inotify), "it was never raised again");

            // And the new watch is a working one, not just a flag set back to true.
            let staged = config.path().join("kwinoutputconfig.json.tmp");
            std::fs::write(&staged, r#"[{"name":"outputs"}]"#).unwrap();
            std::fs::rename(&staged, &file).unwrap();
            tokio::time::timeout(Duration::from_secs(1), nudge.notified())
                .await
                .expect("the source came back but says nothing");
        }

        /// A source already up is left alone. Two readers on one set of events would
        /// double every nudge, and `Live` would then be wrong about the source the first
        /// time one of the two died.
        #[tokio::test]
        async fn a_source_that_is_up_is_not_raised_a_second_time() {
            let config = tempfile::tempdir().unwrap();
            let file = config.path().join("kwinoutputconfig.json");
            std::fs::write(&file, "[]").unwrap();
            let nudge = Arc::new(Notify::new());
            let live = Arc::new(Live::default());

            events::raise(std::slice::from_ref(&file), &nudge, &live);
            assert!(live.up(Source::Inotify), "no inotify watch to test with");
            events::raise(std::slice::from_ref(&file), &nudge, &live);

            let staged = config.path().join("kwinoutputconfig.json.tmp");
            std::fs::write(&staged, r#"[{"name":"outputs"}]"#).unwrap();
            std::fs::rename(&staged, &file).unwrap();
            tokio::time::timeout(Duration::from_secs(1), nudge.notified())
                .await
                .expect("nudged");
            // One rename, one nudge. A second reader would have left another permit
            // behind, and `notified()` would return at once instead of waiting.
            assert!(
                tokio::time::timeout(Duration::from_millis(200), nudge.notified())
                    .await
                    .is_err(),
                "the same events were read twice, so the source was raised twice"
            );
            // And the reader is still running, so its source is still up. A guard let go
            // too early would have the loop retrying a source that is working, spawning
            // a second reader a minute.
            assert!(
                live.up(Source::Inotify),
                "the source reads events but is marked down"
            );
        }

        /// When the loop goes back for a missing source, and when it leaves well alone.
        #[test]
        fn a_missing_source_is_tried_again_but_not_before_its_time() {
            let live = Live::default();
            let (_, _) = (live.came_up(Source::Inotify), live.came_up(Source::Uevents));
            assert!(live.all_up());
            assert!(
                !retry_due(&live, RETRY * 100),
                "nothing is missing, so there is nothing to raise — however long it has been"
            );

            live.went(Source::Inotify);
            assert!(
                !retry_due(&live, RETRY - Duration::from_millis(1)),
                "a source down for less than the interval is not tried again yet"
            );
            assert!(retry_due(&live, RETRY), "and at the interval it is");
            assert!(retry_due(&live, RETRY * 10));
        }

        /// A reader that ends marks its source down, **however** it ends.
        ///
        /// This is what makes the retry fire at all, and it was the one perturbation the
        /// other tests did not catch: deleting the line that marked the source down left
        /// every one of them green, because they all made the loss by hand. Hence the guard,
        /// and hence this.
        #[test]
        fn a_reader_that_ends_marks_its_source_down() {
            let live = Arc::new(Live::default());

            let (reader, recovered) = Reader::new(Source::Inotify, live.clone());
            assert!(live.up(Source::Inotify));
            assert!(!recovered, "it had not been lost, so nothing to announce");
            drop(reader);
            assert!(!live.up(Source::Inotify), "nothing would ever retry it");

            // Including the ways a plain line at the end of the loop would miss: a panic on
            // the way out, and the task dropped without running to completion.
            let live2 = live.clone();
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (_reader, _) = Reader::new(Source::Uevents, live2);
                panic!("the relevance test blew up");
            }));
            assert!(panicked.is_err());
            assert!(
                !live.up(Source::Uevents),
                "a panicking reader left its source marked up with nobody reading it"
            );
        }

        /// The bookkeeping that decides what reaches the log, away from any descriptor.
        #[test]
        fn a_loss_is_said_once_and_so_is_the_recovery() {
            let live = Live::default();

            assert!(live.lost(Source::Inotify), "the first loss is news");
            assert!(!live.lost(Source::Inotify), "and the second is not");
            assert!(!live.lost(Source::Inotify));
            // The other source keeps its own counsel.
            assert!(live.lost(Source::Uevents), "each source speaks for itself");

            assert!(live.came_up(Source::Inotify), "coming back is news too");
            assert!(live.up(Source::Inotify));
            assert!(!live.all_up(), "the other one is still down");

            // And once it has been up, losing it again is news again.
            live.went(Source::Inotify);
            assert!(live.lost(Source::Inotify));

            assert!(live.came_up(Source::Uevents));
            assert!(
                !live.came_up(Source::Uevents),
                "a source that never went is not a recovery"
            );
        }
    }
}
