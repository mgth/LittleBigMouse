//! The Windows service's offer, as the agent announces it to its masters
//! (docs/multi-pc-service-windows.md): while the service listens for the input stream,
//! it writes its port and the fingerprint of its certificate under
//! `HKLM\SOFTWARE\LittleBigMouse\InputService`, which only administrators and SYSTEM
//! can write. The agent reads them there and nowhere else — not from its own
//! configuration, which the user's processes can write — and hands them to the
//! multi-PC manager, which says them in `Hello` (`SecureInput`).
//!
//! Followed by registry notifications, not polled: the service starts listening when an
//! administrator approves the first master, with the agent already running.

use lbm_net::wire::SecureInput;

/// The key, under `HKEY_LOCAL_MACHINE`: `lbm-service`'s `win::offer::KEY`.
pub const KEY: &str = r"SOFTWARE\LittleBigMouse\InputService";

/// The offer the values make, if they make one: a port and a fingerprint (64 hex
/// digits). Anything else is no offer.
pub fn offer(port: u32, machine_id: &str) -> Option<SecureInput> {
    let port = u16::try_from(port).ok().filter(|port| *port != 0)?;
    let fingerprint = machine_id.len() == 64
        && machine_id
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    fingerprint.then(|| SecureInput {
        port,
        machine_id: machine_id.to_owned(),
    })
}

#[cfg(windows)]
pub use windows_side::{read, watch, Watch};

/// No service off Windows.
#[cfg(not(windows))]
pub fn read() -> Option<SecureInput> {
    None
}

#[cfg(windows)]
mod windows_side {
    use tokio::sync::mpsc::UnboundedSender;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY,
        HKEY_LOCAL_MACHINE, KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
        RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    };
    use windows::Win32::System::Threading::{
        CreateEventW, SetEvent, WaitForMultipleObjects, INFINITE,
    };

    use super::SecureInput;

    /// Where the offer is looked for: the key, its parent (watched with all below it)
    /// and the parent's parent (watched for the parent to be made). The service's, or
    /// a test's own.
    #[derive(Clone)]
    pub(super) struct Place {
        pub root: HKEY,
        pub key: Vec<u16>,
        pub parent: Vec<u16>,
        pub above: Vec<u16>,
    }

    // SAFETY: a predefined root key and plain strings.
    unsafe impl Send for Place {}

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    impl Place {
        pub fn new(root: HKEY, above: &str) -> Place {
            Place {
                root,
                key: wide(&format!(r"{above}\LittleBigMouse\InputService")),
                parent: wide(&format!(r"{above}\LittleBigMouse")),
                above: wide(above),
            }
        }

        fn service() -> Place {
            Place::new(HKEY_LOCAL_MACHINE, "SOFTWARE")
        }
    }

    /// The offer the service publishes now, if it publishes one.
    pub fn read() -> Option<SecureInput> {
        read_at(&Place::service())
    }

    pub(super) fn read_at(place: &Place) -> Option<SecureInput> {
        let key = PCWSTR(place.key.as_ptr());
        let mut port = 0u32;
        let mut size = 4u32;
        // SAFETY: a DWORD and its size.
        unsafe {
            RegGetValueW(
                place.root,
                key,
                w!("Port"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut port as *mut u32 as *mut _),
                Some(&mut size),
            )
        }
        .ok()
        .ok()?;
        let mut id = [0u16; 128];
        let mut size = std::mem::size_of_val(&id) as u32;
        // SAFETY: a buffer and its size in bytes; the value comes back NUL-terminated.
        unsafe {
            RegGetValueW(
                place.root,
                key,
                w!("MachineId"),
                RRF_RT_REG_SZ,
                None,
                Some(id.as_mut_ptr() as *mut _),
                Some(&mut size),
            )
        }
        .ok()
        .ok()?;
        let length = id.iter().position(|&c| c == 0).unwrap_or(id.len());
        super::offer(port, &String::from_utf16_lossy(&id[..length]))
    }

    /// Following the offer: `changed` hears every change of it, and once at the start.
    /// Stopped when dropped.
    pub struct Watch {
        stop: HANDLE,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    // SAFETY: an event handle, usable from any thread.
    unsafe impl Send for Watch {}

    impl Drop for Watch {
        fn drop(&mut self) {
            // SAFETY: the event is open until after the join.
            unsafe {
                let _ = SetEvent(self.stop);
                if let Some(thread) = self.thread.take() {
                    let _ = thread.join();
                }
                let _ = CloseHandle(self.stop);
            }
        }
    }

    struct Stop(HANDLE);
    // SAFETY: as above.
    unsafe impl Send for Stop {}

    pub fn watch(changed: UnboundedSender<()>) -> Option<Watch> {
        watch_at(Place::service(), changed)
    }

    pub(super) fn watch_at(place: Place, changed: UnboundedSender<()>) -> Option<Watch> {
        // SAFETY: a fresh, unnamed, auto-reset event.
        let stop = unsafe { CreateEventW(None, false, false, None) }.ok()?;
        let handed = Stop(stop);
        let thread = std::thread::Builder::new()
            .name("service-offer".to_owned())
            .spawn(move || follow(place, handed, changed))
            .inspect_err(|error| {
                eprintln!("[lbm-agent] the service's offer is not followed: {error}")
            })
            .ok();
        if thread.is_none() {
            // SAFETY: nobody else has it.
            let _ = unsafe { CloseHandle(stop) };
            return None;
        }
        Some(Watch { stop, thread })
    }

    fn open(root: HKEY, path: &[u16]) -> Option<HKEY> {
        let mut key = HKEY::default();
        // SAFETY: a NUL-terminated path and an out pointer to a local.
        unsafe { RegOpenKeyExW(root, PCWSTR(path.as_ptr()), 0, KEY_NOTIFY, &mut key) }
            .is_ok()
            .then_some(key)
    }

    /// One registration at a time: on the parent and all below it when it exists, else
    /// on the key above it, for the moment it is made. **Every** round asks for a read
    /// once its registration is armed, never before: a change that comes between a read
    /// and the next registration would otherwise be lost (the service writes `Port`, then
    /// `MachineId`). A parent made between the look and the registration on the key
    /// above is seen at once, and watched instead.
    fn follow(place: Place, stop: Stop, changed: UnboundedSender<()>) {
        loop {
            let (key, on_parent) = match open(place.root, &place.parent) {
                Some(key) => (key, true),
                None => match open(place.root, &place.above) {
                    Some(key) => (key, false),
                    None => {
                        eprintln!("[lbm-agent] the service's offer cannot be followed");
                        return;
                    }
                },
            };
            // SAFETY: the key and the event are this round's, closed at its end; `stop`
            // outlives the thread (its owner joins before closing it).
            let stopped = unsafe {
                let Ok(event) = CreateEventW(None, false, false, None) else {
                    let _ = RegCloseKey(key);
                    return;
                };
                let filter = if on_parent {
                    REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET
                } else {
                    REG_NOTIFY_CHANGE_NAME
                };
                let armed = RegNotifyChangeKeyValue(key, on_parent, filter, event, true).is_ok();
                let parent_appeared = !on_parent
                    && open(place.root, &place.parent)
                        .inspect(|k| {
                            let _ = RegCloseKey(*k);
                        })
                        .is_some();
                let outcome = if !armed {
                    eprintln!("[lbm-agent] the service's offer cannot be followed");
                    Some(true)
                } else if parent_appeared {
                    // Watched from the next round, on the parent itself.
                    None
                } else if changed.send(()).is_err() {
                    Some(true)
                } else {
                    Some(
                        WaitForMultipleObjects(&[stop.0, event], false, INFINITE).0
                            != WAIT_OBJECT_0.0 + 1,
                    )
                };
                let _ = CloseHandle(event);
                let _ = RegCloseKey(key);
                outcome
            };
            if stopped == Some(true) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn a_port_and_a_fingerprint_make_an_offer() {
        assert_eq!(
            offer(24818, ID),
            Some(SecureInput {
                port: 24818,
                machine_id: ID.to_owned()
            })
        );
    }

    #[test]
    fn anything_else_is_no_offer() {
        assert_eq!(offer(0, ID), None);
        assert_eq!(offer(70000, ID), None);
        assert_eq!(offer(24818, "abc"), None);
        assert_eq!(offer(24818, &ID.to_uppercase()), None);
        assert_eq!(offer(24818, &ID.replace('0', "g")), None);
    }

    /// Under a key of the test's own in HKCU: the machine's offer is never touched, and
    /// no elevation is needed.
    #[cfg(windows)]
    #[tokio::test]
    async fn the_offer_written_is_heard_and_read_and_its_removal_too() {
        use windows::core::PCWSTR;
        use windows::Win32::System::Registry::{
            RegCreateKeyW, RegDeleteTreeW, RegSetKeyValueW, HKEY, HKEY_CURRENT_USER, REG_DWORD,
            REG_SZ,
        };
        let above = format!(r"Software\LittleBigMouseTest-{}", std::process::id());
        let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain([0]).collect() };
        let place = windows_side::Place::new(HKEY_CURRENT_USER, &above);
        let delete = |path: &str| {
            let path = wide(path);
            // SAFETY: a NUL-terminated path under the test's own key.
            let _ = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr())) };
        };
        delete(&above);
        let mut made = HKEY::default();
        let above_w = wide(&above);
        // SAFETY: as above; the handle is not needed.
        unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(above_w.as_ptr()), &mut made) }
            .ok()
            .unwrap();
        let key_w = place.key.clone();
        let write = || {
            let port = 24818u32;
            let id = wide(ID);
            // SAFETY: a DWORD and a NUL-terminated string, with their sizes.
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key_w.as_ptr()),
                    windows::core::w!("Port"),
                    REG_DWORD.0,
                    Some(&port as *const u32 as *const _),
                    4,
                )
                .ok()
                .unwrap();
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key_w.as_ptr()),
                    windows::core::w!("MachineId"),
                    REG_SZ.0,
                    Some(id.as_ptr() as *const _),
                    (id.len() * 2) as u32,
                )
                .ok()
                .unwrap();
            }
        };
        assert_eq!(windows_side::read_at(&place), None);
        let (changed, mut changes) = tokio::sync::mpsc::unbounded_channel();
        let _watch = windows_side::watch_at(place.clone(), changed).unwrap();
        assert!(heard(&mut changes).await, "the first word of the watch");
        // The parent did not exist: made with the offer, it is seen from above.
        write();
        let mut read = None;
        for _ in 0..3 {
            if !heard(&mut changes).await {
                break;
            }
            read = windows_side::read_at(&place);
            if read.is_some() {
                break;
            }
        }
        assert_eq!(
            read.map(|offer| offer.port),
            Some(24818),
            "the offer written is heard"
        );
        // What the writes may still have queued is not taken for the removal's.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        while changes.try_recv().is_ok() {}
        delete(&format!(r"{above}\LittleBigMouse\InputService"));
        assert!(heard(&mut changes).await, "the offer removed is heard");
        assert_eq!(windows_side::read_at(&place), None);
        delete(&above);
    }

    /// A notification: not a watch that stopped (`None`), nor a timeout.
    #[cfg(windows)]
    async fn heard(changes: &mut tokio::sync::mpsc::UnboundedReceiver<()>) -> bool {
        matches!(
            tokio::time::timeout(std::time::Duration::from_secs(5), changes.recv()).await,
            Ok(Some(()))
        )
    }
}
