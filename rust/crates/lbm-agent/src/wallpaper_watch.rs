//! The desktop saying its wallpapers changed, for the masters this machine is a remote of
//! (docs/multi-pc.md, "Wallpapers").
//!
//! Events only, and only while somebody watches: the agent asks its
//! [`Desktop`](crate::desktop::Desktop) for a watch when a master links and lets go of it
//! when the last one leaves, so a machine nobody looks at has no descriptor, no thread and
//! no registration of its own. Nothing here ever asks the desktop anything on a timer —
//! the one delay is armed by an event ([`Burst`]) and ends with the reading it gathers.
//!
//! What an event means is only "read again": the reading decides, and the same pictures
//! again are no change for anybody (`peers::actor`).
//!
//! - **Plasma**: inotify on `plasma-org.kde.plasma.desktop-appletsrc`, which plasmashell
//!   rewrites (by a rename) when a containment's wallpaper is configured. plasmashell does
//!   have a `wallpaperChanged` signal on the bus, but it is only emitted by its own
//!   `setWallpaper` method — not by the settings dialog, which is how a person does it.
//! - **Windows**: the `WM_SETTINGCHANGE` broadcast of `SPI_SETDESKWALLPAPER`, received by
//!   the window `winwatch` already has, and a change notification on the registry key the
//!   shell keeps the wallpaper in, for what changes it without a broadcast (a slide show).
//! - **Anything else**: no watch. The wallpaper follows at the next link or display event.

use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;

/// Where a watch says the wallpapers may have changed.
#[derive(Clone, Debug)]
pub struct Changes(UnboundedSender<()>);

impl Changes {
    /// Says so. `false` once nobody listens: the watch has nothing left to do.
    pub fn changed(&self) -> bool {
        self.0.send(()).is_ok()
    }
}

/// A [`Changes`] and where what it says comes out.
pub fn channel() -> (Changes, UnboundedReceiver<()>) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (Changes(sender), receiver)
}

/// A watch, for as long as this is held: dropping it takes down whatever was installed.
pub struct Watching(#[allow(dead_code)] Box<dyn Send + Sync>);

impl Watching {
    /// `held` is whatever keeps the watch up, and ends it when dropped.
    pub fn new(held: impl Send + Sync + 'static) -> Self {
        Watching(Box::new(held))
    }
}

impl std::fmt::Debug for Watching {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watching")
    }
}

/// How long after the last event of a change the desktop is read: one change is several
/// events (a file written then renamed over, a broadcast and a registry write, one message
/// per top-level window), and the desktop may not show the new picture at the first.
pub const QUIET: Duration = Duration::from_millis(400);

/// The longest a reading waits for events to stop coming. A desktop that never goes
/// quiet is still read, a few times a minute at worst — every one of them asked for by
/// an event.
pub const LONGEST: Duration = Duration::from_secs(2);

/// The events of one change, gathered into one reading.
///
/// Armed by an event and by nothing else: with no event there is no deadline, and the
/// agent's loop has no timer to wake for.
#[derive(Clone, Copy, Debug, Default)]
pub struct Burst {
    /// When the first event of the burst came.
    first: Option<Instant>,
    /// When the last one did.
    last: Option<Instant>,
}

impl Burst {
    /// An event, at `now`.
    pub fn event(&mut self, now: Instant) {
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    /// When to read, if an event asked for it.
    pub fn due(&self) -> Option<Instant> {
        Some((self.last? + QUIET).min(self.first? + LONGEST))
    }

    /// The reading is under way, or nobody wants it any more.
    pub fn clear(&mut self) {
        *self = Burst::default();
    }
}

/// `WM_SETTINGCHANGE`, and the `wParam` it carries when the wallpaper was set
/// (`SystemParametersInfo`'s action). Written out, not taken from the `windows` crate, so
/// that the filter is the same code — and is tested — on every platform.
pub const WM_SETTINGCHANGE: u32 = 0x001A;
pub const SPI_SETDESKWALLPAPER: usize = 0x0014;

/// Whether a message to the agent's window means the wallpaper was set.
///
/// Only the broadcast that names the wallpaper: the other `WM_SETTINGCHANGE`s (a work
/// area, a policy, the environment) come by the dozen and say nothing of it.
pub fn is_wallpaper_message(msg: u32, wparam: usize) -> bool {
    msg == WM_SETTINGCHANGE && wparam == SPI_SETDESKWALLPAPER
}

/// Follows a notification that has to be armed again after each time it fires (the
/// registry's): `arm` registers and says whether it could, `wait` sleeps until it fires
/// and says `false` when the watch is over instead.
///
/// The first registration is itself said as a change. A watch is asked for when a master
/// links, and the wallpapers are read for it then; what changed between that reading and
/// the registration — the thread had not got there yet — was notified to nobody, and only
/// a reading taken once armed can be trusted to be followed by a notification.
pub fn follow(mut arm: impl FnMut() -> bool, mut wait: impl FnMut() -> bool, changes: &Changes) {
    let mut armed_before = false;
    while arm() {
        if !armed_before && !changes.changed() {
            break;
        }
        armed_before = true;
        if !wait() || !changes.changed() {
            break;
        }
    }
}

/// The file plasmashell keeps its containments in, wallpapers included.
#[cfg(target_os = "linux")]
pub const PLASMA_CONFIG: &str = "plasma-org.kde.plasma.desktop-appletsrc";

/// The user's configuration directory, as the environment names it.
#[cfg(target_os = "linux")]
pub fn config_dir() -> Option<std::path::PathBuf> {
    use std::path::{Path, PathBuf};
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
}

/// The file to watch in `config_dir` for this `session`, if it is Plasma's.
///
/// The session says so, not the file: a Plasma that has not written its configuration
/// yet (a new account, a configuration just wiped) is the one whose first wallpaper would
/// otherwise never be seen by a master already linked. The file being there counts too,
/// for an agent started without the session's variables.
#[cfg(target_os = "linux")]
pub fn plasma_config(
    session: &lbm_display::linux::desktop::DesktopEnvironment,
    config_dir: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let file = config_dir.join(PLASMA_CONFIG);
    (session.is_kde() || file.exists()).then_some(file)
}

/// Says `changes` whenever `file` is written or replaced, until the watch is dropped.
///
/// The directory is watched, not the file: a desktop saves by writing beside the file and
/// renaming over it, which a watch on the file itself would follow to the old inode and
/// never see again (`watch::events`, whose descriptor and parsing these are).
///
/// Must be called from within the runtime: the reader is a task of it.
#[cfg(target_os = "linux")]
pub fn watch_file(file: &std::path::Path, changes: Changes) -> std::io::Result<Watching> {
    use std::os::fd::AsRawFd;

    use crate::watch::events;

    /// Ends the reader with the watch; the descriptor goes with the task.
    struct Reader(tokio::task::JoinHandle<()>);
    impl Drop for Reader {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    let file = file.to_path_buf();
    let mut target = events::watched(std::slice::from_ref(&file));
    let mut fd = tokio::io::unix::AsyncFd::new(events::inotify(&target.0)?)?;
    let reader = tokio::spawn(async move {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let touched = {
                let Ok(mut ready) = fd.readable().await else {
                    break;
                };
                match ready.try_io(|fd| events::read(fd.as_raw_fd(), &mut buf)) {
                    Ok(Ok(n)) => events::touches(&buf[..n], &target.1),
                    // Events were lost: whatever they were, read.
                    Ok(Err(e)) if e.raw_os_error() == Some(libc::ENOBUFS) => true,
                    Ok(Err(e)) if e.kind() == std::io::ErrorKind::Interrupted => false,
                    Ok(Err(error)) => {
                        eprintln!("[lbm-agent] multi-PC: the wallpaper watch failed: {error}");
                        break;
                    }
                    Err(_would_block) => false,
                }
            };
            if !touched {
                continue;
            }
            // The entry itself may be what changed: created as a link into another
            // directory (a dotfile manager's), or pointed somewhere else. What was
            // resolved when the watch was installed is then the wrong place to look, so
            // it is resolved again — on this event, and before saying the change, so
            // that the reading it asks for is taken with the new place already watched.
            let now = events::watched(std::slice::from_ref(&file));
            if now != target {
                match events::inotify(&now.0).and_then(tokio::io::unix::AsyncFd::new) {
                    Ok(moved) => {
                        fd = moved;
                        target = now;
                    }
                    Err(error) => eprintln!(
                        "[lbm-agent] multi-PC: the wallpaper watch could not follow {} to \
                         where it now points: {error}",
                        file.display()
                    ),
                }
            }
            if !changes.changed() {
                break;
            }
        }
    });
    Ok(Watching::new(Reader(reader)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_is_read_once_it_goes_quiet_and_no_later_than_the_longest_wait() {
        let start = Instant::now();
        let mut burst = Burst::default();
        assert_eq!(
            burst.due(),
            None,
            "no event, no deadline: nothing to wake for"
        );

        burst.event(start);
        assert_eq!(burst.due(), Some(start + QUIET));
        // Another event of the same change pushes the reading back…
        let later = start + Duration::from_millis(300);
        burst.event(later);
        assert_eq!(burst.due(), Some(later + QUIET));
        // …but a desktop that never goes quiet is read all the same.
        let much_later = start + LONGEST - Duration::from_millis(10);
        burst.event(much_later);
        assert_eq!(burst.due(), Some(start + LONGEST));

        burst.clear();
        assert_eq!(
            burst.due(),
            None,
            "read: nothing is owed until the next event"
        );
        burst.event(much_later);
        assert_eq!(
            burst.due(),
            Some(much_later + QUIET),
            "the next burst is its own"
        );
    }

    #[test]
    fn only_the_broadcast_that_names_the_wallpaper_is_a_wallpaper_message() {
        assert!(is_wallpaper_message(WM_SETTINGCHANGE, SPI_SETDESKWALLPAPER));
        // SPI_SETWORKAREA: a display matter, `winwatch`'s own.
        assert!(!is_wallpaper_message(WM_SETTINGCHANGE, 0x002F));
        // A policy or the environment changed: `wParam` 0 and a name in `lParam`.
        assert!(!is_wallpaper_message(WM_SETTINGCHANGE, 0));
        // WM_DISPLAYCHANGE, WM_PAINT: the same number in `wParam` means nothing there.
        assert!(!is_wallpaper_message(0x007E, SPI_SETDESKWALLPAPER));
        assert!(!is_wallpaper_message(0x000F, SPI_SETDESKWALLPAPER));
    }

    /// The registry watch's own gap: the agent read the wallpapers when the master
    /// linked, and the thread had not registered yet.
    #[test]
    fn what_changed_before_the_first_registration_is_read_once_it_is_armed() {
        let (changes, mut received) = channel();
        let said = std::cell::Cell::new(0);
        let mut count = || {
            while received.try_recv().is_ok() {
                said.set(said.get() + 1);
            }
            said.get()
        };
        let mut arms = 0;
        let mut waits = 0;
        follow(
            || {
                arms += 1;
                true
            },
            || {
                waits += 1;
                match waits {
                    // Asleep for the first time: armed, and nothing fired — yet a
                    // reading was asked for, the one that covers the gap.
                    1 => assert_eq!(count(), 1, "the first registration asks for a reading"),
                    // Fired once since: one more, and no other for arming again.
                    2 => assert_eq!(count(), 2, "armed again without another reading"),
                    _ => return false,
                }
                true
            },
            &changes,
        );
        assert_eq!(arms, 3, "armed again after each notification");
        assert_eq!(count(), 3, "the last notification before the watch ended");
    }

    #[test]
    fn a_registration_refused_says_nothing() {
        let (changes, mut received) = channel();
        follow(|| false, || unreachable!("nothing to wait for"), &changes);
        assert!(received.try_recv().is_err());
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use super::super::*;

        async fn said(received: &mut UnboundedReceiver<()>) -> bool {
            matches!(
                tokio::time::timeout(Duration::from_secs(1), received.recv()).await,
                Ok(Some(()))
            )
        }

        /// The real thing, in a directory of the test's: plasmashell saves by a rename
        /// over the file, and other files written beside it say nothing.
        #[tokio::test]
        async fn a_configuration_replaced_by_a_rename_is_said_and_its_neighbours_are_not() {
            let config = tempfile::tempdir().unwrap();
            let file = config
                .path()
                .join("plasma-org.kde.plasma.desktop-appletsrc");
            std::fs::write(&file, "[Containments][1]\n").unwrap();
            let (changes, mut received) = channel();
            let _watching = watch_file(&file, changes).expect(
                "no inotify watch to test with (ENOSPC is fs.inotify.max_user_watches spent)",
            );

            std::fs::write(config.path().join("kwinrc"), "[Desktops]\n").unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(200), received.recv())
                    .await
                    .is_err(),
                "another file of the directory is not the wallpaper"
            );

            let staged = config
                .path()
                .join("plasma-org.kde.plasma.desktop-appletsrc.Xy12");
            std::fs::write(&staged, "[Containments][1][Wallpaper]\nImage=new\n").unwrap();
            std::fs::rename(&staged, &file).unwrap();
            assert!(said(&mut received).await, "the rename was not seen");

            // And again: the watch is on the directory, so it survives the replacement.
            while received.try_recv().is_ok() {}
            let staged = config
                .path()
                .join("plasma-org.kde.plasma.desktop-appletsrc.Zw34");
            std::fs::write(&staged, "[Containments][1][Wallpaper]\nImage=newer\n").unwrap();
            std::fs::rename(&staged, &file).unwrap();
            assert!(said(&mut received).await, "the second rename was not seen");
        }

        /// A master linked before Plasma first wrote its configuration: the session says
        /// what to watch, and the file appearing is the first change.
        #[tokio::test]
        async fn a_plasma_session_is_watched_before_its_configuration_exists() {
            use lbm_display::linux::desktop::DesktopEnvironment;

            let config = tempfile::tempdir().unwrap();
            let plasma = DesktopEnvironment::new(Some("KDE"), None);
            let gnome = DesktopEnvironment::new(Some("ubuntu:GNOME"), None);
            assert_eq!(
                plasma_config(&gnome, config.path()),
                None,
                "another desktop, and no file of Plasma's: nothing to watch"
            );
            let file = plasma_config(&plasma, config.path())
                .expect("a Plasma session is watched whether or not it has saved anything");
            assert!(!file.exists());

            let (changes, mut received) = channel();
            let _watching = watch_file(&file, changes).expect("no inotify watch to test with");
            let staged = config.path().join(format!("{PLASMA_CONFIG}.Ab56"));
            std::fs::write(&staged, "[Containments][1][Wallpaper]\nImage=first\n").unwrap();
            std::fs::rename(&staged, &file).unwrap();
            assert!(said(&mut received).await, "the first save was not seen");

            // The file alone is enough for an agent started without the session's variables.
            assert_eq!(plasma_config(&gnome, config.path()), Some(file));
        }

        /// A dotfile manager installing the configuration after the master linked: the
        /// entry becomes a link, and the desktop then rewrites the file where it lives.
        #[tokio::test]
        async fn a_configuration_installed_as_a_link_is_followed_to_where_it_points() {
            let config = tempfile::tempdir().unwrap();
            let dotfiles = tempfile::tempdir().unwrap();
            let file = config.path().join(PLASMA_CONFIG);
            let (changes, mut received) = channel();
            let _watching = watch_file(&file, changes).expect("no inotify watch to test with");

            let real = dotfiles.path().join("appletsrc");
            std::fs::write(&real, "[Containments][1]\n").unwrap();
            std::os::unix::fs::symlink(&real, &file).unwrap();
            assert!(said(&mut received).await, "the link appearing was not seen");
            while received.try_recv().is_ok() {}

            // In place, then by a rename over it: both happen in the other directory.
            std::fs::write(&real, "[Containments][1][Wallpaper]\nImage=new\n").unwrap();
            assert!(
                said(&mut received).await,
                "a write through the link was not seen"
            );
            while received.try_recv().is_ok() {}
            let staged = dotfiles.path().join("appletsrc.Cd78");
            std::fs::write(&staged, "[Containments][1][Wallpaper]\nImage=newer\n").unwrap();
            std::fs::rename(&staged, &real).unwrap();
            assert!(
                said(&mut received).await,
                "a rename over the target was not seen"
            );
            while received.try_recv().is_ok() {}

            // Pointed elsewhere: the new place is the one followed from then on.
            let other = dotfiles.path().join("other-appletsrc");
            std::fs::write(&other, "[Containments][1]\n").unwrap();
            std::fs::remove_file(&file).unwrap();
            std::os::unix::fs::symlink(&other, &file).unwrap();
            assert!(said(&mut received).await, "the link changing was not seen");
            // The removal and the new link are two events; let both be handled.
            tokio::time::sleep(Duration::from_millis(200)).await;
            while received.try_recv().is_ok() {}
            std::fs::write(&other, "[Containments][1][Wallpaper]\nImage=other\n").unwrap();
            assert!(
                said(&mut received).await,
                "a write to the new target was not seen"
            );
        }

        #[tokio::test]
        async fn a_watch_let_go_of_leaves_nothing_reading() {
            let config = tempfile::tempdir().unwrap();
            let file = config
                .path()
                .join("plasma-org.kde.plasma.desktop-appletsrc");
            std::fs::write(&file, "").unwrap();
            let (changes, mut received) = channel();
            let watching = watch_file(&file, changes).expect("no inotify watch to test with");

            drop(watching);
            // The reader held the only other end: once it is gone the channel closes,
            // which is the proof that the task — and the descriptor it owned — ended.
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), received.recv())
                    .await
                    .expect("the reader outlived its watch"),
                None
            );
        }
    }
}
