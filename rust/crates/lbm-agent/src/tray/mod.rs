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
//! [`TrayModel`] is all of that; `linux` and `windows` only draw it.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::api::{self, Call, Request};
use crate::icons::TrayIcon;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::run;
#[cfg(windows)]
pub use windows::run;

/// What the menu does (C#'s `TrayMenu`, less "Check for update": the agent does not
/// update itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Start,
    Stop,
    Refresh,
    Exit,
}

impl Action {
    /// The menu, in order; `None` is a separator.
    pub const MENU: [Option<Action>; 6] = [
        Some(Action::Open),
        Some(Action::Start),
        Some(Action::Stop),
        Some(Action::Refresh),
        None,
        Some(Action::Exit),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Open => "Open",
            Action::Start => "Start",
            Action::Stop => "Stop",
            Action::Refresh => "Refresh",
            Action::Exit => "Exit",
        }
    }

    /// The request it sends; `None` for Open, which launches the frontend instead.
    fn method(self) -> Option<&'static str> {
        match self {
            Action::Open => None,
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
            state: "Starting".to_owned(),
            hidden: false,
            calls,
            client,
            next_id: 0,
            open,
        }
    }

    /// A menu entry (or a click on the icon: Open).
    pub fn act(&mut self, action: Action) {
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
    }

    pub fn icon(&self) -> TrayIcon {
        self.icon
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
