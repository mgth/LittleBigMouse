//! The tray — port of C#'s `TrayIconController`: `ksni` under Linux (no GTK), the
//! notification area (`Shell_NotifyIcon`) under Windows.
//!
//! The tray is a frontend like the others: it follows the agent's state through the
//! frontends' API and its menu sends the same requests (`Start`, `Stop`, `Refresh`,
//! `Quit`), in process instead of over the endpoint. The icon is the engine's state, as in
//! C#: on, off, dead, paused (the display asleep included). A click opens the frontend.
//!
//! The icon can also be asked not to be there at all: `HideTrayIcon` travels in the state
//! like everything else, and the platforms take the icon down and put it back rather than
//! grey it out — that is what the option meant in C#, where it set `Visible`.
//!
//! Multi-PC: the icon also says where the pointer is ([`Badge`]) — faded on a master
//! whose mouse and keyboard went to another machine, dotted on the machine they drive.
//! The tooltip says it too, but reading it takes the pointer onto the icon, which is
//! where it is not while the other machine has it. The badge changes with the state the
//! crossing publishes: nothing is polled.
//!
//! Under Windows the tray is also where the update check speaks (`crate::update`): its
//! menu has "Check for update", and what the check finds comes as a notice, a click on
//! which carries on (install, or open the release page).
//!
//! [`TrayModel`] is all of that; `linux` and `windows` only draw it.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::api::{self, Call, Request};
use crate::icons::{Badge, MenuIcon, TrayIcon};
use crate::update::{self, Notice};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::run;
#[cfg(windows)]
pub use windows::run;

/// What the menu does (C#'s `TrayMenu`; "Check for update" under Windows only, where
/// there is a check).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Start,
    Stop,
    Refresh,
    CheckForUpdate,
    Exit,
}

impl Action {
    /// The menu, in order; `None` is a separator.
    #[cfg(not(windows))]
    pub const MENU: &[Option<Action>] = &[
        Some(Action::Open),
        Some(Action::Start),
        Some(Action::Stop),
        Some(Action::Refresh),
        None,
        Some(Action::Exit),
    ];
    /// The menu, in order; `None` is a separator.
    #[cfg(windows)]
    pub const MENU: &[Option<Action>] = &[
        Some(Action::Open),
        Some(Action::Start),
        Some(Action::Stop),
        Some(Action::Refresh),
        None,
        Some(Action::CheckForUpdate),
        Some(Action::Exit),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Open => "Open",
            Action::Start => "Start",
            Action::Stop => "Stop",
            Action::Refresh => "Refresh",
            Action::CheckForUpdate => "Check for update",
            Action::Exit => "Exit",
        }
    }

    /// Its pictogram, the UI's for the same thing: Start and Stop are the toolbar's
    /// buttons, Refresh its reload, Open the application's logo.
    pub fn icon(self) -> MenuIcon {
        match self {
            Action::Open => MenuIcon::Logo,
            Action::Start => MenuIcon::Start,
            Action::Stop => MenuIcon::Stop,
            Action::Refresh => MenuIcon::Refresh,
            Action::CheckForUpdate => MenuIcon::Update,
            Action::Exit => MenuIcon::Exit,
        }
    }

    /// The request it sends; `None` for Open, which launches the frontend instead, and
    /// for Check for update, which goes to the update check.
    fn method(self) -> Option<&'static str> {
        match self {
            Action::Open | Action::CheckForUpdate => None,
            Action::Start => Some("Start"),
            Action::Stop => Some("Stop"),
            Action::Refresh => Some("Refresh"),
            Action::Exit => Some("Quit"),
        }
    }
}

/// What the tray shows and where its clicks go.
pub struct TrayModel {
    icon: TrayIcon,
    /// Where the multi-PC pointer is, over the engine's state.
    badge: Badge,
    /// The engine's state in words, for the tooltip.
    state: String,
    /// The user asked for no icon at all.
    hidden: bool,
    calls: UnboundedSender<Call>,
    /// Where the answers to the tray's requests go (read by [`follow`], which ignores
    /// them).
    client: api::Client,
    next_id: u64,
    open: Arc<dyn Fn() + Send + Sync>,
    /// The update check, where there is one.
    updates: Option<UnboundedSender<update::Command>>,
    /// The check's notice waiting to be shown.
    notice: Option<Notice>,
    /// What a click on the notice last shown does.
    clicked: Option<update::Command>,
}

impl TrayModel {
    /// A tray sending its requests into `calls` as `client`, opening the frontend with
    /// `open`.
    pub fn new(
        calls: UnboundedSender<Call>,
        client: api::Client,
        open: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        TrayModel {
            icon: TrayIcon::Dead,
            badge: Badge::None,
            state: "Starting".to_owned(),
            hidden: false,
            calls,
            client,
            next_id: 0,
            open,
            updates: None,
            notice: None,
            clicked: None,
        }
    }

    /// The tray of an agent that checks for updates: its menu's entry and its notices'
    /// clicks go to `updates`.
    pub fn with_updates(mut self, updates: UnboundedSender<update::Command>) -> Self {
        self.updates = Some(updates);
        self
    }

    /// The update check has something to say: kept until the platform shows it.
    pub fn tell(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }

    /// The notice to show now, if any; a click on it will do what it says.
    pub fn take_notice(&mut self) -> Option<Notice> {
        let notice = self.notice.take()?;
        self.clicked = notice.click;
        Some(notice)
    }

    /// The notice last shown was clicked.
    pub fn notice_clicked(&mut self) {
        if let (Some(command), Some(updates)) = (self.clicked.take(), &self.updates) {
            let _ = updates.send(command);
        }
    }

    /// A menu entry (or a click on the icon: Open).
    pub fn act(&mut self, action: Action) {
        if action == Action::CheckForUpdate {
            if let Some(updates) = &self.updates {
                let _ = updates.send(update::Command::Check);
            }
            return;
        }
        let Some(method) = action.method() else {
            (self.open)();
            return;
        };
        let request: Request =
            serde_json::from_value(json!({ "Method": method })).expect("a method of the API");
        // The subscription is id 0: the tray's requests count from 1.
        self.next_id += 1;
        let _ = self.calls.send(Call::Asked {
            id: self.next_id,
            request,
            client: self.client.clone(),
        });
    }

    /// Shows a state of the agent (a `Snapshot`, as the API sends it).
    pub fn show(&mut self, state: &Value) {
        // Absent means "not hidden": a state from an agent holding no layout yet has no
        // app options to report, and starting invisible would be the worse guess.
        self.hidden = state["HideTrayIcon"] == true;
        let engine = state["Engine"].as_str().unwrap_or("Dead");
        let suspended = state["Suspended"] == true;
        self.icon = TrayIcon::for_state(engine, suspended);
        self.state = match (engine, suspended) {
            (_, true) => "The display is off",
            ("Running", _) => "Running",
            ("Stopped", _) => "Stopped",
            ("Paused", _) => "Paused (an excluded application has the focus)",
            _ => "No hook",
        }
        .to_owned();
        // Multi-PC: the cursor alone says where the keyboard goes, and it may be on a
        // screen nobody is looking at.
        let away = state["DrivenMachine"].as_str();
        let driven_by = state["DrivenBy"].as_str();
        if let Some(machine) = away {
            self.state = format!("{} · mouse and keyboard on {machine}", self.state);
        }
        if let Some(master) = driven_by {
            self.state = format!("{} · driven by {master}", self.state);
        }
        // Gone from here wins: a machine both driven and driving another has handed
        // the pointer it was given on.
        self.badge = match (away, driven_by) {
            (Some(_), _) => Badge::Away,
            (None, Some(_)) => Badge::Driven,
            (None, None) => Badge::None,
        };
        if state["PointerLocked"] == true {
            self.state = format!("{} · pointer locked", self.state);
        }
        // Multi-PC: a remote where Windows drops what this machine sends. Its pointer
        // stood still and this one came back: the user deserves to know why.
        for remote in state["InjectionBlocked"].as_array().into_iter().flatten() {
            let name = remote["Name"].as_str().unwrap_or("a remote");
            // Elevation is advised only for an integrity difference the remote saw.
            let elevated = remote.get("Cause").is_none_or(|cause| cause == "Integrity");
            self.state = format!("{}\n{}", self.state, injection_blocked(name, elevated));
        }
        // Another sharing tool moves the pointer whatever this engine does: said here,
        // where a user looks when the mouse leaves the screen with LittleBigMouse stopped.
        let running: Vec<lbm_ipc::sharing::Running> =
            serde_json::from_value(state["InputSharing"].clone()).unwrap_or_default();
        for warning in lbm_ipc::sharing::warnings(&running) {
            self.state = format!("{}\n{warning}", self.state);
        }
    }

    pub fn icon(&self) -> TrayIcon {
        self.icon
    }

    /// Where the multi-PC pointer is, drawn over [`Self::icon`].
    pub fn badge(&self) -> Badge {
        self.badge
    }

    /// The engine's state in words.
    pub fn state(&self) -> &str {
        &self.state
    }

    /// Whether the user asked for no icon (C#'s `HideTrayIcon`).
    pub fn hidden(&self) -> bool {
        self.hidden
    }
}

/// Subscribes `client` to the agent and hands every state to `show`, until the agent goes
/// or `show` says the tray is gone.
pub async fn follow(
    calls: &UnboundedSender<Call>,
    client: api::Client,
    mut frames: UnboundedReceiver<String>,
    mut show: impl FnMut(&Value) -> bool,
) {
    let subscribed = calls.send(Call::Asked {
        id: 0,
        request: Request::Subscribe,
        client,
    });
    if subscribed.is_err() {
        return;
    }
    while let Some(frame) = frames.recv().await {
        let Ok(frame) = serde_json::from_str::<Value>(&frame) else {
            continue;
        };
        let state = if frame["Event"] == "State" {
            &frame["State"]
        } else if frame["Id"] == 0 {
            &frame["Result"]
        } else {
            continue;
        };
        if !show(state) {
            return;
        }
    }
}

/// What the user is told about a remote where Windows drops the input this machine sends:
/// `elevated` when the remote saw a window of a higher integrity level in front, the only
/// case where running it elevated is the way out.
pub fn injection_blocked(name: &str, elevated: bool) -> String {
    if elevated {
        format!(
            "Windows blocks the mouse and keyboard on {name}: an elevated window has the \
             focus (turn on \"Start with elevated privileges\" there)"
        )
    } else {
        format!(
            "Windows blocks the mouse and keyboard on {name} for now (a secure desktop, such \
             as a UAC prompt or the lock screen, may be showing there)"
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    pub(super) fn model() -> (TrayModel, UnboundedReceiver<Call>, Arc<AtomicU32>) {
        let (calls, calls_rx) = tokio::sync::mpsc::unbounded_channel();
        let (client, _frames) = api::in_process();
        let opened = Arc::new(AtomicU32::new(0));
        let counter = opened.clone();
        let open = Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        (TrayModel::new(calls, client, open), calls_rx, opened)
    }

    #[test]
    fn the_menu_drives_the_agent_as_a_frontend_would() {
        let (mut tray, mut calls, opened) = model();
        for action in [Action::Start, Action::Stop, Action::Refresh, Action::Exit] {
            tray.act(action);
        }
        let sent: Vec<(u64, Request)> =
            std::iter::from_fn(|| calls.try_recv().ok().and_then(Call::asked)).collect();
        assert_eq!(
            sent,
            [
                (
                    1,
                    Request::Start {
                        keep_layout: false,
                        layout_id: None,
                        document: None,
                    },
                ),
                (2, Request::Stop),
                (3, Request::Refresh),
                (4, Request::Quit)
            ]
        );
        tray.act(Action::Open);
        assert_eq!(opened.load(Ordering::Relaxed), 1);
        assert!(calls.try_recv().is_err(), "Open asks the agent nothing");
    }

    #[test]
    fn every_entry_has_an_icon_of_its_own() {
        let icons: Vec<MenuIcon> = Action::MENU.iter().flatten().map(|a| a.icon()).collect();
        let mut expected = vec![
            MenuIcon::Logo,
            MenuIcon::Start,
            MenuIcon::Stop,
            MenuIcon::Refresh,
        ];
        if cfg!(windows) {
            expected.push(MenuIcon::Update);
        }
        expected.push(MenuIcon::Exit);
        assert_eq!(icons, expected);
    }

    #[test]
    fn check_for_update_is_offered_only_where_there_is_a_check() {
        assert_eq!(
            Action::MENU.contains(&Some(Action::CheckForUpdate)),
            cfg!(windows),
            "the package manager owns updates off Windows"
        );
    }

    #[test]
    fn the_update_entry_and_notices_drive_the_update_check() {
        let (tray, mut calls, opened) = model();
        let (updates, mut commands) = tokio::sync::mpsc::unbounded_channel();
        let mut tray = tray.with_updates(updates);

        tray.act(Action::CheckForUpdate);
        assert_eq!(commands.try_recv().ok(), Some(update::Command::Check));
        assert!(calls.try_recv().is_err(), "the agent is not asked");
        assert_eq!(opened.load(Ordering::Relaxed), 0, "nor the frontend opened");

        // A notice shown, then clicked: what it says it does is done, once.
        tray.tell(Notice {
            title: "Little Big Mouse 6.0.1 is available".to_owned(),
            text: "Click to download and install it.".to_owned(),
            warning: false,
            click: Some(update::Command::Install),
        });
        let shown = tray.take_notice().expect("a notice to show");
        assert_eq!(shown.title, "Little Big Mouse 6.0.1 is available");
        assert!(tray.take_notice().is_none(), "shown once");
        tray.notice_clicked();
        tray.notice_clicked();
        assert_eq!(commands.try_recv().ok(), Some(update::Command::Install));
        assert!(commands.try_recv().is_err(), "one click, one install");

        // A notice that leads nowhere: its click does nothing.
        tray.tell(Notice {
            title: "Little Big Mouse is up to date".to_owned(),
            text: "Version 6.0.1".to_owned(),
            warning: false,
            click: None,
        });
        let _ = tray.take_notice();
        tray.notice_clicked();
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn the_hide_option_is_followed_both_ways() {
        // It is a state like any other: the agent publishes it, so a change made from any
        // frontend reaches the tray without it having to read the store.
        let (mut tray, _calls, _) = model();
        assert!(!tray.hidden(), "no option said yet: the icon is there");

        tray.show(&json!({ "Engine": "Running", "HideTrayIcon": true }));
        assert!(tray.hidden());

        tray.show(&json!({ "Engine": "Running", "HideTrayIcon": false }));
        assert!(!tray.hidden(), "and it comes back");

        // An agent with no layout loaded reports nothing about the app options.
        tray.show(&json!({ "Engine": "Dead" }));
        assert!(!tray.hidden());
    }

    #[test]
    fn the_icon_and_the_words_follow_the_state() {
        let (mut tray, _calls, _) = model();
        assert_eq!(tray.icon(), TrayIcon::Dead);
        tray.show(&json!({ "Engine": "Running", "Suspended": false }));
        assert_eq!((tray.icon(), tray.state()), (TrayIcon::On, "Running"));
        tray.show(&json!({ "Engine": "Running", "Suspended": true }));
        assert_eq!(tray.icon(), TrayIcon::Paused);
    }

    #[test]
    fn the_words_say_which_machine_has_the_mouse_and_keyboard() {
        let (mut tray, _calls, _) = model();
        tray.show(&json!({ "Engine": "Running", "DrivenMachine": "laptop" }));
        assert_eq!(tray.state(), "Running · mouse and keyboard on laptop");
        assert_eq!(tray.icon(), TrayIcon::On, "the icon is the engine's");
        // Back home, and an older agent's state, say nothing about it.
        tray.show(&json!({ "Engine": "Running", "DrivenMachine": null }));
        assert_eq!(tray.state(), "Running");
        tray.show(&json!({ "Engine": "Running" }));
        assert_eq!(tray.state(), "Running");
    }

    #[test]
    fn the_badge_follows_the_pointer_across_machines() {
        let (mut tray, _calls, _) = model();
        tray.show(&json!({ "Engine": "Running" }));
        assert_eq!((tray.icon(), tray.badge()), (TrayIcon::On, Badge::None));
        // The master, its pointer gone to the laptop, and back.
        tray.show(&json!({ "Engine": "Running", "DrivenMachine": "laptop" }));
        assert_eq!((tray.icon(), tray.badge()), (TrayIcon::On, Badge::Away));
        tray.show(&json!({ "Engine": "Running", "DrivenMachine": null }));
        assert_eq!(tray.badge(), Badge::None);
        // The laptop, driven by the desk, then left.
        tray.show(&json!({ "Engine": "Running", "DrivenBy": "desk" }));
        assert_eq!((tray.icon(), tray.badge()), (TrayIcon::On, Badge::Driven));
        assert_eq!(tray.state(), "Running · driven by desk");
        tray.show(&json!({ "Engine": "Running", "DrivenBy": null }));
        assert_eq!((tray.badge(), tray.state()), (Badge::None, "Running"));
        // A badge over any state of the engine: the laptop's hook may be stopped.
        tray.show(&json!({ "Engine": "Stopped", "DrivenBy": "desk" }));
        assert_eq!((tray.icon(), tray.badge()), (TrayIcon::Off, Badge::Driven));
        // Driven and driving on: the pointer is not here.
        tray.show(&json!({ "Engine": "Running", "DrivenBy": "desk",
                           "DrivenMachine": "tablet" }));
        assert_eq!(tray.badge(), Badge::Away);
    }

    #[test]
    fn the_words_say_the_pointer_is_locked() {
        let (mut tray, _calls, _) = model();
        tray.show(&json!({ "Engine": "Running", "DrivenMachine": "laptop",
                           "PointerLocked": true }));
        assert_eq!(
            tray.state(),
            "Running · mouse and keyboard on laptop · pointer locked"
        );
        tray.show(&json!({ "Engine": "Running", "PointerLocked": true }));
        assert_eq!(tray.state(), "Running · pointer locked");
        tray.show(&json!({ "Engine": "Running", "PointerLocked": false }));
        assert_eq!(tray.state(), "Running");
    }

    #[test]
    fn the_words_say_where_windows_blocks_the_input() {
        let (mut tray, _calls, _) = model();
        tray.show(&json!({ "Engine": "Running",
            "InjectionBlocked": [{ "Name": "vm124", "Process": "osk.exe", "Cause": "Integrity" }] }));
        assert_eq!(
            tray.state(),
            "Running\nWindows blocks the mouse and keyboard on vm124: an elevated window \
             has the focus (turn on \"Start with elevated privileges\" there)"
        );
        // A cause the remote did not see (a secure desktop): no advice to elevate.
        tray.show(&json!({ "Engine": "Running",
            "InjectionBlocked": [{ "Name": "vm124", "Process": "", "Cause": "Unknown" }] }));
        assert_eq!(
            tray.state(),
            "Running\nWindows blocks the mouse and keyboard on vm124 for now (a secure \
             desktop, such as a UAC prompt or the lock screen, may be showing there)"
        );
        assert!(!tray.state().contains("elevated"));
        // No longer blocked, and an older agent's state, say nothing about it.
        tray.show(&json!({ "Engine": "Running", "InjectionBlocked": [] }));
        assert_eq!(tray.state(), "Running");
        tray.show(&json!({ "Engine": "Running" }));
        assert_eq!(tray.state(), "Running");
    }

    #[test]
    fn the_words_say_another_sharing_tool_runs() {
        let (mut tray, _calls, _) = model();
        tray.show(&json!({ "Engine": "Stopped", "InputSharing": [
            { "Tool": "Deskflow", "Process": "deskflow-core", "Pid": 40,
              "Role": "Server", "Connected": true },
        ]}));
        assert_eq!(
            tray.state(),
            "Stopped\nDeskflow is also running and can move the mouse to other machines, \
             and it is connected to one now"
        );
        // An older agent, or none found: nothing said.
        tray.show(&json!({ "Engine": "Stopped" }));
        assert_eq!(tray.state(), "Stopped");
        tray.show(&json!({ "Engine": "Stopped", "InputSharing": "garbled" }));
        assert_eq!(tray.state(), "Stopped");
    }

    #[tokio::test]
    async fn the_tray_follows_the_state_until_the_agent_goes() {
        let (calls, mut calls_rx) = tokio::sync::mpsc::unbounded_channel::<Call>();
        let (client, frames) = api::in_process();
        let agent = tokio::spawn(async move {
            // The subscription, answered, then two changes, then the agent leaves.
            let Some(Call::Asked {
                id,
                request,
                client,
            }) = calls_rx.recv().await
            else {
                panic!("a request");
            };
            assert_eq!((id, request), (0, Request::Subscribe));
            client.send(api::answer(0, Ok(json!({ "Engine": "Stopped" }))));
            client.send(json!({ "Id": 3, "Result": null }).to_string());
            client.send(json!({ "Event": "State", "State": { "Engine": "Running" } }).to_string());
        });
        let mut seen = Vec::new();
        follow(&calls, client, frames, |state| {
            seen.push(state["Engine"].as_str().unwrap().to_owned());
            true
        })
        .await;
        agent.await.unwrap();
        assert_eq!(
            seen,
            ["Stopped", "Running"],
            "answers to other requests skipped"
        );
    }
}
