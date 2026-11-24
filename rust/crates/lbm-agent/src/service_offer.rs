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

    const KEY_W: PCWSTR = w!(r"SOFTWARE\LittleBigMouse\InputService");

    /// The offer the service publishes now, if it publishes one.
    pub fn read() -> Option<SecureInput> {
        let mut port = 0u32;
        let mut size = 4u32;
        // SAFETY: a DWORD and its size.
        unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                KEY_W,
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
                HKEY_LOCAL_MACHINE,
                KEY_W,
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

    /// Following the offer: `changed` hears every change of it (and once at the start).
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
        // SAFETY: a fresh, unnamed, auto-reset event.
        let stop = unsafe { CreateEventW(None, false, false, None) }.ok()?;
        let handed = Stop(stop);
        let thread = std::thread::Builder::new()
            .name("service-offer".to_owned())
            .spawn(move || follow(handed, changed))
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

    /// One registration at a time: on `SOFTWARE\LittleBigMouse` and all below it when
    /// it exists, else on `SOFTWARE` for the moment it is made. Each notification is
    /// passed on (a read is cheap), then the registration is made again.
    fn follow(stop: Stop, changed: UnboundedSender<()>) {
        let mut first = true;
        loop {
            // SAFETY: keys and events opened and closed in this iteration; `stop`
            // outlives the thread (its owner joins before closing it).
            let stopped = unsafe {
                let mut key = HKEY::default();
                let (opened, subtree, filter) = if RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    w!(r"SOFTWARE\LittleBigMouse"),
                    0,
                    KEY_NOTIFY,
                    &mut key,
                )
                .is_ok()
                {
                    (
                        true,
                        true,
                        REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                    )
                } else {
                    let opened =
                        RegOpenKeyExW(HKEY_LOCAL_MACHINE, w!("SOFTWARE"), 0, KEY_NOTIFY, &mut key)
                            .is_ok();
                    (opened, false, REG_NOTIFY_CHANGE_NAME)
                };
                let Ok(event) = CreateEventW(None, false, false, None) else {
                    if opened {
                        let _ = RegCloseKey(key);
                    }
                    return;
                };
                let armed =
                    opened && RegNotifyChangeKeyValue(key, subtree, filter, event, true).is_ok();
                // Read once the first registration is armed: a change between the
                // caller's own read and it is not lost.
                if std::mem::take(&mut first) && changed.send(()).is_err() {
                    return;
                }
                let stopped = if armed {
                    WaitForMultipleObjects(&[stop.0, event], false, INFINITE).0
                        != WAIT_OBJECT_0.0 + 1
                } else {
                    eprintln!("[lbm-agent] the service's offer cannot be followed");
                    true
                };
                let _ = CloseHandle(event);
                if opened {
                    let _ = RegCloseKey(key);
                }
                stopped
            };
            if stopped || changed.send(()).is_err() {
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

    /// Writes and removes the real key: an administrator's test (CI runners are).
    #[cfg(windows)]
    #[tokio::test]
    async fn the_offer_written_is_heard_and_read_and_its_removal_too() {
        use windows::core::w;
        use windows::Win32::System::Registry::{
            RegDeleteKeyW, RegSetKeyValueW, HKEY_LOCAL_MACHINE, REG_DWORD, REG_SZ,
        };
        let key = w!(r"SOFTWARE\LittleBigMouse\InputService");
        let write = || -> bool {
            let port = 24818u32;
            let id: Vec<u16> = ID.encode_utf16().chain([0]).collect();
            // SAFETY: a DWORD and a NUL-terminated string, with their sizes.
            unsafe {
                RegSetKeyValueW(
                    HKEY_LOCAL_MACHINE,
                    key,
                    w!("Port"),
                    REG_DWORD.0,
                    Some(&port as *const u32 as *const _),
                    4,
                )
                .is_ok()
                    && RegSetKeyValueW(
                        HKEY_LOCAL_MACHINE,
                        key,
                        w!("MachineId"),
                        REG_SZ.0,
                        Some(id.as_ptr() as *const _),
                        (id.len() * 2) as u32,
                    )
                    .is_ok()
            }
        };
        // SAFETY: a constant key name.
        let _ = unsafe { RegDeleteKeyW(HKEY_LOCAL_MACHINE, key) };
        if !write() {
            eprintln!("not an administrator: HKLM cannot be written, nothing to test");
            return;
        }
        // SAFETY: as above.
        let _ = unsafe { RegDeleteKeyW(HKEY_LOCAL_MACHINE, key) };
        assert_eq!(read(), None);

        let (changed, mut changes) = tokio::sync::mpsc::unbounded_channel();
        let _watch = watch(changed).unwrap();
        assert!(heard(&mut changes).await, "the first word of the watch");
        assert!(write());
        assert!(heard(&mut changes).await, "the offer written is heard");
        assert_eq!(read().map(|offer| offer.port), Some(24818));
        // SAFETY: as above.
        let _ = unsafe { RegDeleteKeyW(HKEY_LOCAL_MACHINE, key) };
        assert!(heard(&mut changes).await, "the offer removed is heard");
        assert_eq!(read(), None);
    }

    #[cfg(windows)]
    async fn heard(changes: &mut tokio::sync::mpsc::UnboundedReceiver<()>) -> bool {
        tokio::time::timeout(std::time::Duration::from_secs(5), changes.recv())
            .await
            .is_ok()
    }
}
