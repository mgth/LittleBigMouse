//! The client side against `wire-contract/goldens`: the commands are rebuilt byte for
//! byte from the C#-produced `ui-to-daemon` goldens, and every event of the
//! Rust-produced `daemon-to-ui/events.txt` reads back with its payload.

use std::fs;
use std::path::PathBuf;

use lbm_ipc::client::{self, DaemonEvent};

fn golden(path: &str) -> String {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("crate lives at rust/crates/lbm-ipc")
        .join("wire-contract/goldens")
        .join(path);
    fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("{}: {e}", file.display()))
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_owned()
}

#[test]
fn commands_match_the_csharp_goldens() {
    assert_eq!(client::run(), golden("ui-to-daemon/command-run.xml"));
    assert_eq!(client::stop(), golden("ui-to-daemon/command-stop.xml"));
    assert_eq!(client::quit(), golden("ui-to-daemon/command-quit.xml"));
    assert_eq!(
        client::shortcut("Ctrl+Alt+Shift+M"),
        golden("ui-to-daemon/command-shortcut.xml")
    );
    // The Load golden wraps the current layout golden.
    assert_eq!(
        client::load(&golden("ui-to-daemon/layout-v5.6-current.xml")),
        golden("ui-to-daemon/command-load.xml")
    );
}

#[test]
fn every_event_the_daemon_emits_reads_back() {
    let events: Vec<_> = golden("daemon-to-ui/events.txt")
        .lines()
        .map(|line| client::parse_event(line).unwrap_or_else(|| panic!("unread: {line}")))
        .collect();
    let kinds: Vec<DaemonEvent> = events.iter().map(|m| m.event).collect();
    assert_eq!(
        kinds,
        [
            DaemonEvent::Running,
            DaemonEvent::Stopped,
            DaemonEvent::Paused,
            DaemonEvent::DisplayChanged,
            DaemonEvent::SettingsChanged,
            DaemonEvent::DesktopChanged,
            DaemonEvent::Suspended,
            DaemonEvent::Resumed,
            DaemonEvent::Rescued,
            DaemonEvent::LoadFailed,
            DaemonEvent::RunRefused,
            DaemonEvent::Loaded,
            DaemonEvent::ShortcutUnavailable,
            DaemonEvent::FocusChanged,
            DaemonEvent::Probed,
        ]
    );
    let payload = |event: DaemonEvent| {
        events
            .iter()
            .find(|m| m.event == event)
            .map(|m| m.payload.as_str())
            .unwrap()
    };
    assert_eq!(payload(DaemonEvent::Running), "");
    assert_eq!(payload(DaemonEvent::Loaded), "2 zones (2 main)");
    // RunRefused always says why; the frontend shows the reason as it is.
    assert_eq!(
        payload(DaemonEvent::RunRefused),
        "the layout does not match the attached displays: no display under Dock"
    );
    assert_eq!(
        payload(DaemonEvent::FocusChanged),
        r"C:\Games\A&B\<Stopped DisplayChanged>.exe"
    );
    assert!(payload(DaemonEvent::Probed).starts_with("<ProbeReport Algorithm=\"Cross\""));
    assert_eq!(
        payload(DaemonEvent::Probed),
        golden("daemon-to-ui/probe-report.xml")
    );
}
