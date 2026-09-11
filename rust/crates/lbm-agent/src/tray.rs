//! The tray (plan, phase 3: `ksni` under Linux, without GTK; `Shell_NotifyIcon` under
//! Windows comes with the Windows agent) — port of C#'s `TrayIconController`.
//!
//! The tray is a frontend like the others: it follows the agent's state through the
//! frontends' API and its menu sends the same requests (`Start`, `Stop`, `Refresh`,
//! `Quit`), in process instead of over the socket. The icon is the engine's state, as in
//! C#: on, off, dead, paused (the display asleep included). A click opens the frontend.

use std::sync::Arc;

use ksni::menu::StandardItem;
use ksni::{Category, MenuItem, ToolTip, TrayMethods};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::api::{self, Call, Request};
use crate::icons::{self, TrayIcon};

/// What the tray shows and where its clicks go.
pub struct AgentTray {
    icon: TrayIcon,
    /// The engine's state in words, for the tooltip.
    state: String,
    calls: UnboundedSender<Call>,
    /// Where the answers to the tray's requests go (read by [`run`], which ignores them).
    client: api::Client,
    next_id: u64,
    open: Arc<dyn Fn() + Send + Sync>,
}

impl AgentTray {
    /// A tray sending its requests into `calls`, opening the frontend with `open`.
    pub fn new(
        calls: UnboundedSender<Call>,
        client: api::Client,
        open: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        AgentTray {
            icon: TrayIcon::Dead,
            state: "Starting".to_owned(),
            calls,
            client,
            next_id: 0,
            open,
        }
    }

    /// Sends `method` to the agent, as a frontend would.
    fn ask(&mut self, method: &str) {
        let request: Request =
            serde_json::from_value(json!({ "Method": method })).expect("a method of the API");
        // The subscription is id 0: the tray's requests count from 1.
        self.next_id += 1;
        let _ = self.calls.send(Call {
            id: self.next_id,
            request,
            client: self.client.clone(),
        });
    }

    /// Shows a state of the agent (a `Snapshot`, as the API sends it).
    pub fn show(&mut self, state: &Value) {
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
}

impl ksni::Tray for AgentTray {
    fn id(&self) -> String {
        "LittleBigMouse".to_owned()
    }

    fn title(&self) -> String {
        "LittleBigMouse".to_owned()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icons::pixmaps(self.icon)
            .iter()
            .map(|p| ksni::Icon {
                width: p.width as i32,
                height: p.height as i32,
                data: p.argb.clone(),
            })
            .collect()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "LittleBigMouse".to_owned(),
            description: self.state.clone(),
            ..Default::default()
        }
    }

    /// C#: a click on the icon opens the window.
    fn activate(&mut self, _x: i32, _y: i32) {
        (self.open)();
    }

    /// C#'s menu, less "Check for update": the distribution package owns updates here.
    fn menu(&self) -> Vec<MenuItem<Self>> {
        fn item(label: &str, activate: fn(&mut AgentTray)) -> MenuItem<AgentTray> {
            StandardItem {
                label: label.to_owned(),
                activate: Box::new(activate),
                ..Default::default()
            }
            .into()
        }
        vec![
            item("Open", |t| (t.open)()),
            item("Start", |t| t.ask("Start")),
            item("Stop", |t| t.ask("Stop")),
            item("Refresh", |t| t.ask("Refresh")),
            MenuItem::Separator,
            item("Exit", |t| t.ask("Quit")),
        ]
    }
}

/// Puts the tray up and keeps it on the agent's state until the agent goes. Without a
/// tray host (no StatusNotifierWatcher: a bare session, a test runner) it says so and
/// returns: the agent runs on without one.
pub async fn run(calls: UnboundedSender<Call>, open: Arc<dyn Fn() + Send + Sync>) {
    let (client, mut frames) = api::in_process();
    let subscribed = calls.send(Call {
        id: 0,
        request: Request::Subscribe,
        client: client.clone(),
    });
    if subscribed.is_err() {
        return;
    }
    let tray = match AgentTray::new(calls, client, open).spawn().await {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("[lbm-agent] no tray: {error}");
            return;
        }
    };
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
        if tray.update(|t| t.show(state)).await.is_none() {
            return;
        }
    }
    tray.shutdown().await;
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use ksni::Tray;

    use super::*;

    fn tray() -> (
        AgentTray,
        tokio::sync::mpsc::UnboundedReceiver<Call>,
        Arc<AtomicU32>,
    ) {
        let (calls, calls_rx) = tokio::sync::mpsc::unbounded_channel();
        let (client, _frames) = api::in_process();
        let opened = Arc::new(AtomicU32::new(0));
        let counter = opened.clone();
        let open = Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        (AgentTray::new(calls, client, open), calls_rx, opened)
    }

    fn click(tray: &mut AgentTray, label: &str) {
        let menu = tray.menu();
        let Some(MenuItem::Standard(item)) = menu
            .into_iter()
            .find(|i| matches!(i, MenuItem::Standard(s) if s.label == label))
        else {
            panic!("no {label} entry");
        };
        (item.activate)(tray);
    }

    #[test]
    fn the_menu_drives_the_agent_as_a_frontend_would() {
        let (mut tray, mut calls, opened) = tray();
        for label in ["Start", "Stop", "Refresh", "Exit"] {
            click(&mut tray, label);
        }
        let sent: Vec<(u64, Request)> =
            std::iter::from_fn(|| calls.try_recv().ok().map(|c| (c.id, c.request))).collect();
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

        click(&mut tray, "Open");
        tray.activate(0, 0);
        assert_eq!(opened.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn the_icon_and_the_tooltip_follow_the_state() {
        let (mut tray, _calls, _) = tray();
        assert_eq!(tray.icon(), TrayIcon::Dead);
        tray.show(&json!({ "Engine": "Running", "Suspended": false }));
        assert_eq!(tray.icon(), TrayIcon::On);
        assert_eq!(tray.tool_tip().description, "Running");
        tray.show(&json!({ "Engine": "Running", "Suspended": true }));
        assert_eq!(tray.icon(), TrayIcon::Paused);
        assert_eq!(tray.icon_pixmap().len(), 6);
    }
}
