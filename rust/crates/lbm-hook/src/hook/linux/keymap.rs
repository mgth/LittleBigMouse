//! The desktop's keyboard layout, for the shortcuts a master reads below it.
//!
//! The evdev backend reads key codes: positions, before any layout. A shortcut names a
//! letter by what the layout prints, so `Ctrl+Alt+Shift+M` is the key right of `N` on
//! a US keyboard and the key right of `L` on a French one. Without the layout, the
//! rescue and the pointer lock are watched for on a key the user never presses.
//!
//! The compositor says which layout it applies: `wl_keyboard` hands any client the
//! keymap it uses, and again whenever it changes. No focus is needed for that, and no
//! key is ever seen through this connection. Which of the keymap's layouts is active is
//! another matter: `wl_keyboard.modifiers` carries it, and a compositor may send it
//! only to the client that has the focus, which this one never has. So the active
//! layout is also asked of KDE (`org.kde.KeyboardLayouts`, its index is the keymap's
//! group), and followed there. Where neither says, a keymap with one layout needs no
//! answer; one with several is read in its first, and the log says so.
//!
//! The keymap is read with xkbcommon, loaded at run time as the overlay does; without
//! either, the letters stay where a US keyboard has them.

use std::os::unix::fs::FileExt;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, Once};

use futures::StreamExt;

use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_keyboard::{self, KeymapFormat, WlKeyboard},
        wl_registry::WlRegistry,
        wl_seat::{self, Capability, WlSeat},
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use xkbcommon_dl::{xkb_keymap, XkbCommon};

use crate::hook::crossing::keys::Letters;
use crate::shared::Shared;

/// The largest keymap read: a real one is tens of kilobytes.
const MAX_KEYMAP: u32 = 1 << 20;

/// Follows the desktop's layout for as long as the process lives, on a thread of its
/// own: the connection only ever waits for the compositor. Started once.
pub fn spawn(shared: &'static Shared) {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("lbm-keymap".to_owned())
            .spawn(move || {
                if let Err(why) = follow(shared) {
                    eprintln!(
                        "[LittleBigMouse.Hook] keyboard layout: not read ({why}); shortcut letters are taken where a US keyboard has them"
                    );
                }
            });
        if let Err(error) = spawned {
            eprintln!("[LittleBigMouse.Hook] keyboard layout: no thread: {error}");
        }
        let spawned = std::thread::Builder::new()
            .name("lbm-layout".to_owned())
            .spawn(move || follow_kde(shared));
        if let Err(error) = spawned {
            eprintln!("[LittleBigMouse.Hook] keyboard layout: no thread for KDE's: {error}");
        }
    });
}

/// What is known of the desktop's layout, from whichever thread learnt it last.
struct Known {
    /// The compositor's keymap, NUL-terminated.
    keymap: Option<Vec<u8>>,
    /// Its active layout, once anyone said which.
    group: Option<u32>,
    /// The unknown group was already reported for this keymap.
    told_unknown: bool,
}

static KNOWN: Mutex<Known> = Mutex::new(Known {
    keymap: None,
    group: None,
    told_unknown: false,
});

/// A new keymap: read again in the group already known.
fn keymap_is(shared: &Shared, keymap: Vec<u8>) {
    let mut known = KNOWN.lock().unwrap_or_else(|p| p.into_inner());
    known.keymap = Some(keymap);
    known.told_unknown = false;
    apply(shared, &mut known);
    ASK_KDE_AGAIN.notify_one();
}

/// The active layout is `group`: read the keymap again if it changed.
fn group_is(shared: &Shared, group: u32) {
    let mut known = KNOWN.lock().unwrap_or_else(|p| p.into_inner());
    if known.group == Some(group) {
        return;
    }
    known.group = Some(group);
    apply(shared, &mut known);
}

fn apply(shared: &Shared, known: &mut Known) {
    let Some(xkb) = xkbcommon_dl::xkbcommon_option() else {
        return;
    };
    let Some(keymap) = &known.keymap else {
        return;
    };
    let read = letters_of_keymap(xkb, keymap, known.group.unwrap_or(0));
    match read {
        Some(read) => {
            if known.group.is_none() && read.layouts > 1 && !known.told_unknown {
                known.told_unknown = true;
                eprintln!(
                    "[LittleBigMouse.Hook] keyboard layout: {} layouts and none said to be active; shortcut letters are read in the first",
                    read.layouts
                );
            }
            adopt(shared, read.letters);
        }
        None => eprintln!(
            "[LittleBigMouse.Hook] keyboard layout: the compositor's keymap could not be read"
        ),
    }
}

/// KDE's keyboard layouts service.
const KDE: &str = "org.kde.keyboard";

/// Asks KDE for the active layout again: a new keymap is the desktop's layouts being
/// (re)loaded, which is also when its `/Layouts` may have just appeared.
static ASK_KDE_AGAIN: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Follows KDE's active layout for as long as the session bus lives, from whenever KDE's
/// service appears, and again each time it comes back. Elsewhere, `wl_keyboard.modifiers`
/// is all there is.
fn follow_kde(shared: &'static Shared) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    let followed = runtime.block_on(async {
        let connection = zbus::Connection::session().await?;
        follow_layouts(&connection, &ASK_KDE_AGAIN, |group| group_is(shared, group)).await
    });
    if let Err(error) = followed {
        eprintln!(
            "[LittleBigMouse.Hook] keyboard layout: KDE's active layout not followed ({error})"
        );
    }
}

/// Says each active layout KDE gives on `connection`. Waits for the service to have an
/// owner, follows it until it has none, and waits again: a hook started before KDE's
/// session, or a KDE restarted, is followed all the same.
async fn follow_layouts(
    connection: &zbus::Connection,
    ask_again: &tokio::sync::Notify,
    mut on_group: impl FnMut(u32),
) -> zbus::Result<()> {
    let bus = zbus::fdo::DBusProxy::new(connection).await?;
    // Subscribed before asking whether it is there: an arrival in between is not missed.
    let mut owners = bus
        .receive_name_owner_changed_with_args(&[(0, KDE)])
        .await?;
    let name = zbus::names::BusName::try_from(KDE)?;
    loop {
        if bus.name_has_owner(name.clone()).await? {
            follow_service(connection, &mut owners, ask_again, &mut on_group).await?;
        }
        loop {
            let Some(change) = owners.next().await else {
                return Ok(());
            };
            if change.args().is_ok_and(|a| a.new_owner().is_some()) {
                break;
            }
        }
    }
}

/// Follows KDE's service while it has an owner. Its name may come before its `/Layouts`:
/// a question it cannot answer yet is asked again at the next keymap, and its signals
/// are heard meanwhile.
async fn follow_service(
    connection: &zbus::Connection,
    owners: &mut zbus::fdo::NameOwnerChangedStream,
    ask_again: &tokio::sync::Notify,
    on_group: &mut impl FnMut(u32),
) -> zbus::Result<()> {
    let layouts = zbus::Proxy::new(connection, KDE, "/Layouts", "org.kde.KeyboardLayouts").await?;
    // Subscribed before the first question: a switch in between is not missed.
    let mut changed = layouts.receive_signal("layoutChanged").await?;
    let mut relisted = layouts.receive_signal("layoutListChanged").await?;
    let mut told = false;
    let mut ask = true;
    loop {
        if std::mem::take(&mut ask) {
            if let Ok(group) = layouts.call::<_, _, u32>("getLayout", &()).await {
                on_group(group);
                if !std::mem::replace(&mut told, true) {
                    eprintln!(
                        "[LittleBigMouse.Hook] keyboard layout: the active layout is followed in KDE's"
                    );
                }
            }
        }
        tokio::select! {
            message = changed.next() => {
                let Some(message) = message else { return Ok(()) };
                if let Ok(group) = message.body().deserialize::<u32>() {
                    on_group(group);
                }
            }
            message = relisted.next() => {
                if message.is_none() {
                    return Ok(());
                }
                ask = true;
            }
            () = ask_again.notified() => ask = true,
            change = owners.next() => match change {
                None => return Ok(()),
                Some(change) if change.args().is_ok_and(|a| a.new_owner().is_none()) => {
                    return Ok(())
                }
                Some(_) => ask = true,
            },
        }
    }
}

fn follow(shared: &'static Shared) -> Result<(), String> {
    xkbcommon_dl::xkbcommon_option().ok_or("no libxkbcommon")?;
    let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let (globals, mut queue) =
        registry_queue_init::<State>(&connection).map_err(|e| e.to_string())?;
    let handle = queue.handle();
    let _seat: WlSeat = globals
        .bind(&handle, 1..=5, ())
        .map_err(|e| format!("no seat: {e}"))?;
    let mut state = State {
        shared,
        keyboard: None,
    };
    loop {
        queue
            .blocking_dispatch(&mut state)
            .map_err(|e| e.to_string())?;
    }
}

struct State {
    shared: &'static Shared,
    keyboard: Option<WlKeyboard>,
}

/// Puts the letters where the routing thread reads them, and has it read them again
/// when they moved.
fn adopt(shared: &Shared, letters: Letters) {
    let mut current = shared.letters.lock().unwrap_or_else(|p| p.into_inner());
    if *current == letters {
        return;
    }
    *current = letters;
    shared.shortcuts_revision.fetch_add(1, Ordering::SeqCst);
    eprintln!(
        "[LittleBigMouse.Hook] keyboard layout: shortcut letters read from the desktop's layout{}",
        if letters == Letters::US {
            " (as on a US keyboard)"
        } else {
            ""
        }
    );
}

/// The letters of a keymap in one of its layouts, and how many it has.
#[derive(Debug, PartialEq)]
struct Read {
    letters: Letters,
    layouts: u32,
}

/// The letters of `keymap` (XKB text, NUL-terminated) in its layout `group`.
fn letters_of_keymap(xkb: &XkbCommon, keymap: &[u8], group: u32) -> Option<Read> {
    if keymap.last() != Some(&0) {
        return None;
    }
    // SAFETY: a NUL-terminated text; both objects are released before returning.
    unsafe {
        let context = (xkb.xkb_context_new)(xkbcommon_dl::xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
        if context.is_null() {
            return None;
        }
        let compiled = (xkb.xkb_keymap_new_from_string)(
            context,
            keymap.as_ptr().cast(),
            xkbcommon_dl::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
            xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        );
        let letters = (!compiled.is_null()).then(|| Read {
            letters: letters_of(xkb, compiled, group),
            layouts: (xkb.xkb_keymap_num_layouts)(compiled),
        });
        if !compiled.is_null() {
            (xkb.xkb_keymap_unref)(compiled);
        }
        (xkb.xkb_context_unref)(context);
        letters
    }
}

/// The letters of a compiled keymap in its layout `group`.
///
/// # Safety
/// `keymap` is a live keymap.
unsafe fn letters_of(xkb: &XkbCommon, keymap: *mut xkb_keymap, group: u32) -> Letters {
    let (min, max) = unsafe {
        (
            (xkb.xkb_keymap_min_keycode)(keymap),
            (xkb.xkb_keymap_max_keycode)(keymap),
        )
    };
    // XKB keycodes are Linux codes plus 8.
    let keys = (min.max(8)..=max).filter_map(|keycode| {
        let code = u16::try_from(keycode - 8).ok()?;
        let layouts = unsafe { (xkb.xkb_keymap_num_layouts_for_key)(keymap, keycode) };
        // A key with fewer layouts than the keymap has the same in all of them.
        let layout = if group < layouts { group } else { 0 };
        let levels = unsafe { (xkb.xkb_keymap_num_levels_for_key)(keymap, keycode, layout) };
        let syms = (0..levels)
            .map(|level| {
                let mut syms = std::ptr::null();
                let n = unsafe {
                    (xkb.xkb_keymap_key_get_syms_by_level)(
                        keymap, keycode, layout, level, &mut syms,
                    )
                };
                if n >= 1 && !syms.is_null() {
                    unsafe { *syms }
                } else {
                    0
                }
            })
            .collect();
        Some((code, syms))
    });
    Letters::from_keysyms(keys)
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(handle, ()));
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(KeymapFormat::XkbV1),
                fd,
                size,
            } if size <= MAX_KEYMAP => {
                // The descriptor's offset may be anywhere: the keymap starts at zero.
                let mut bytes = vec![0; size as usize];
                if std::fs::File::from(fd).read_exact_at(&mut bytes, 0).is_ok() {
                    // The protocol's text ends with its NUL; one more costs nothing.
                    bytes.push(0);
                    keymap_is(state.shared, bytes);
                }
            }
            // Only if the compositor sends it here, which it may keep for the client
            // that has the focus.
            wl_keyboard::Event::Modifiers { group, .. } => group_is(state.shared, group),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_M: u16 = 50;
    const KEY_SEMICOLON: u16 = 39;
    const KEY_Q: u16 = 16;
    const KEY_A: u16 = 30;

    /// The letters of a layout xkeyboard-config names; `None` where this machine has
    /// no libxkbcommon or no such layout.
    fn letters_of_layout(layout: &str) -> Option<Letters> {
        let xkb = xkbcommon_dl::xkbcommon_option()?;
        let layout = std::ffi::CString::new(layout).ok()?;
        let names = xkbcommon_dl::xkb_rule_names {
            rules: std::ptr::null(),
            model: std::ptr::null(),
            layout: layout.as_ptr(),
            variant: std::ptr::null(),
            options: std::ptr::null(),
        };
        unsafe {
            let context =
                (xkb.xkb_context_new)(xkbcommon_dl::xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return None;
            }
            let keymap = (xkb.xkb_keymap_new_from_names)(
                context,
                &names,
                xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            let letters = (!keymap.is_null()).then(|| letters_of(xkb, keymap, 0));
            if !keymap.is_null() {
                (xkb.xkb_keymap_unref)(keymap);
            }
            (xkb.xkb_context_unref)(context);
            letters
        }
    }

    #[test]
    fn a_french_keymap_puts_m_right_of_l_and_a_where_us_has_q() {
        let Some(fr) = letters_of_layout("fr") else {
            eprintln!("no libxkbcommon or no French layout here: skipped");
            return;
        };
        let m = crate::hook::crossing::keys::Combination::parse("Ctrl+M", &fr);
        let m_us = crate::hook::crossing::keys::Combination::parse("Ctrl+M", &Letters::US);
        assert_ne!(m, m_us, "M is not where US has it");
        assert_eq!(
            crate::hook::crossing::keys::Combination::parse("Ctrl+Oem1", &Letters::US)
                .map(|c| c.key()),
            Some(KEY_SEMICOLON)
        );
        assert_eq!(m.map(|c| c.key()), Some(KEY_SEMICOLON));
        assert_eq!(
            crate::hook::crossing::keys::Combination::parse("Ctrl+A", &fr).map(|c| c.key()),
            Some(KEY_Q)
        );
    }

    #[test]
    fn a_us_keymap_is_the_table_letters_start_from() {
        let Some(us) = letters_of_layout("us") else {
            eprintln!("no libxkbcommon here: skipped");
            return;
        };
        assert_eq!(us, Letters::US);
        assert_eq!(
            crate::hook::crossing::keys::Combination::parse("Ctrl+M", &us).map(|c| c.key()),
            Some(KEY_M)
        );
        assert_eq!(
            crate::hook::crossing::keys::Combination::parse("Ctrl+A", &us).map(|c| c.key()),
            Some(KEY_A)
        );
    }

    /// The text of the keymap xkeyboard-config builds for `layout`, as a compositor
    /// sends it; `None` where this machine cannot build it.
    fn keymap_text(layout: &str) -> Option<Vec<u8>> {
        let xkb = xkbcommon_dl::xkbcommon_option()?;
        let layout = std::ffi::CString::new(layout).ok()?;
        let names = xkbcommon_dl::xkb_rule_names {
            rules: std::ptr::null(),
            model: std::ptr::null(),
            layout: layout.as_ptr(),
            variant: std::ptr::null(),
            options: std::ptr::null(),
        };
        unsafe {
            let context =
                (xkb.xkb_context_new)(xkbcommon_dl::xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return None;
            }
            let keymap = (xkb.xkb_keymap_new_from_names)(
                context,
                &names,
                xkbcommon_dl::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            let mut text = None;
            if !keymap.is_null() {
                let string = (xkb.xkb_keymap_get_as_string)(
                    keymap,
                    xkbcommon_dl::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                );
                if !string.is_null() {
                    let mut bytes = std::ffi::CStr::from_ptr(string)
                        .to_bytes_with_nul()
                        .to_vec();
                    bytes.shrink_to_fit();
                    text = Some(bytes);
                    libc_free(string.cast_mut().cast());
                }
                (xkb.xkb_keymap_unref)(keymap);
            }
            (xkb.xkb_context_unref)(context);
            text
        }
    }

    extern "C" {
        #[link_name = "free"]
        fn libc_free(pointer: *mut std::ffi::c_void);
    }

    /// The compositor's keymap holds every layout; which one is typed in is the group,
    /// and a group nobody said reads as the first.
    #[test]
    fn a_keymap_of_two_layouts_is_read_in_the_active_one() {
        let (Some(xkb), Some(text), Some(fr)) = (
            xkbcommon_dl::xkbcommon_option(),
            keymap_text("us,fr"),
            letters_of_layout("fr"),
        ) else {
            eprintln!("no libxkbcommon or no us,fr layouts here: skipped");
            return;
        };
        let first = letters_of_keymap(xkb, &text, 0).expect("readable");
        assert_eq!(
            first,
            Read {
                letters: Letters::US,
                layouts: 2
            }
        );
        let second = letters_of_keymap(xkb, &text, 1).expect("readable");
        assert_eq!(
            second,
            Read {
                letters: fr,
                layouts: 2
            }
        );
        // A group the keymap does not have is read as its first, as xkbcommon wraps it.
        assert_eq!(letters_of_keymap(xkb, &text, 7).map(|r| r.layouts), Some(2));
    }

    /// A session bus of its own, gone with it; `None` where there is no dbus-daemon.
    struct PrivateBus {
        daemon: std::process::Child,
        address: String,
    }

    impl PrivateBus {
        fn start() -> Option<PrivateBus> {
            use std::io::BufRead;
            let mut daemon = std::process::Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()?;
            let mut address = String::new();
            std::io::BufReader::new(daemon.stdout.take()?)
                .read_line(&mut address)
                .ok()?;
            Some(PrivateBus {
                daemon,
                address: address.trim().to_owned(),
            })
        }

        async fn connect(&self) -> zbus::Connection {
            zbus::connection::Builder::address(self.address.as_str())
                .expect("an address")
                .build()
                .await
                .expect("the private bus")
        }
    }

    impl Drop for PrivateBus {
        fn drop(&mut self) {
            let _ = self.daemon.kill();
            let _ = self.daemon.wait();
        }
    }

    /// KDE's service as the test publishes it.
    struct FakeLayouts(u32);

    #[zbus::interface(name = "org.kde.KeyboardLayouts")]
    impl FakeLayouts {
        #[zbus(name = "getLayout")]
        fn get_layout(&self) -> u32 {
            self.0
        }
    }

    async fn next(heard: &mut tokio::sync::mpsc::UnboundedReceiver<u32>, what: &str) -> u32 {
        tokio::time::timeout(std::time::Duration::from_secs(5), heard.recv())
            .await
            .unwrap_or_else(|_| panic!("{what}: no layout in time"))
            .expect("the follower is running")
    }

    async fn emit_layout_changed(connection: &zbus::Connection, group: u32) {
        connection
            .emit_signal(
                None::<()>,
                "/Layouts",
                "org.kde.KeyboardLayouts",
                "layoutChanged",
                &(group,),
            )
            .await
            .expect("the signal");
    }

    /// The hook starts before KDE: its name comes, then its `/Layouts`, then it is
    /// restarted. Each active layout is heard.
    #[tokio::test]
    async fn kde_started_after_the_hook_is_followed_all_the_same() {
        let Some(bus) = PrivateBus::start() else {
            eprintln!("no dbus-daemon here: skipped");
            return;
        };
        let hook = bus.connect().await;
        let ask_again: &'static tokio::sync::Notify =
            Box::leak(Box::new(tokio::sync::Notify::new()));
        let (said, mut heard) = tokio::sync::mpsc::unbounded_channel();
        let following = tokio::spawn(async move {
            follow_layouts(&hook, ask_again, |group| {
                let _ = said.send(group);
            })
            .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // The name first, nothing behind it yet: the question goes unanswered.
        let kde = bus.connect().await;
        // Serving no object yet, as a service does whose name comes first: a question
        // is answered "no such object".
        let _ = kde.object_server();
        kde.request_name(KDE).await.expect("the name");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // Its signals are heard all the same.
        emit_layout_changed(&kde, 1).await;
        assert_eq!(
            next(&mut heard, "the signal of a name with no layouts").await,
            1
        );

        // Its layouts published, then a keymap: asked again and answered.
        kde.object_server()
            .at("/Layouts", FakeLayouts(0))
            .await
            .expect("served");
        ask_again.notify_one();
        assert_eq!(next(&mut heard, "the layouts published").await, 0);

        // KDE restarted, on the French layout: asked as soon as it is back.
        drop(kde);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let kde = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(KDE)
            .unwrap()
            .serve_at("/Layouts", FakeLayouts(1))
            .unwrap()
            .build()
            .await
            .expect("KDE again");
        assert_eq!(next(&mut heard, "KDE restarted").await, 1);
        emit_layout_changed(&kde, 0).await;
        assert_eq!(next(&mut heard, "its switch").await, 0);

        following.abort();
    }

    #[test]
    fn a_keymap_text_that_is_not_one_reads_as_none() {
        let Some(xkb) = xkbcommon_dl::xkbcommon_option() else {
            return;
        };
        assert_eq!(letters_of_keymap(xkb, b"not a keymap\0", 0), None);
        assert_eq!(letters_of_keymap(xkb, b"no terminator", 0), None);
    }
}
