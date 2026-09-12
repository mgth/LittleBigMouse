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
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// The event sources (inotify, uevents).
const SOURCES: usize = 2;

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
    let live = Arc::new(AtomicUsize::new(0));
    #[cfg(target_os = "linux")]
    events::start(&paths, &nudge, &live);

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
        let period = if live.load(Ordering::Relaxed) == SOURCES {
            SAFETY_NET
        } else {
            POLL_INTERVAL
        };
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use tokio::io::unix::AsyncFd;
    use tokio::sync::Notify;

    /// Starts both sources; `live` counts those up (and down again if one fails).
    pub fn start(paths: &[PathBuf], nudge: &Arc<Notify>, live: &Arc<AtomicUsize>) {
        let (dirs, names) = watched(paths);
        match inotify(&dirs) {
            Ok(fd) => spawn(fd, read, move |buf| touches(buf, &names), nudge, live),
            Err(error) => eprintln!("[lbm-agent] no inotify on the output files: {error}"),
        }
        match uevents() {
            Ok(fd) => spawn(fd, read, is_display_uevent, nudge, live),
            Err(error) => eprintln!("[lbm-agent] no DRM uevents: {error}"),
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
        fd: OwnedFd,
        read: fn(RawFd, &mut [u8]) -> io::Result<usize>,
        relevant: impl Fn(&[u8]) -> bool + Send + 'static,
        nudge: &Arc<Notify>,
        live: &Arc<AtomicUsize>,
    ) {
        let (nudge, live) = (nudge.clone(), live.clone());
        let Ok(fd) = AsyncFd::new(fd) else { return };
        live.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
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
            // Back to the two-second poll.
            live.fetch_sub(1, Ordering::Relaxed);
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
    fn inotify(dirs: &BTreeSet<PathBuf>) -> io::Result<OwnedFd> {
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
            let live = Arc::new(AtomicUsize::new(0));
            events::start(std::slice::from_ref(&file), &nudge, &live);
            assert!(live.load(Ordering::Relaxed) >= 1, "inotify is up");

            let staged = config.path().join("kwinoutputconfig.json.tmp");
            std::fs::write(&staged, r#"[{"name":"outputs"}]"#).unwrap();
            std::fs::rename(&staged, &file).unwrap();
            tokio::time::timeout(Duration::from_secs(1), nudge.notified())
                .await
                .expect("nudged");
        }
    }
}
