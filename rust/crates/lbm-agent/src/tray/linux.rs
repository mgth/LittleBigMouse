//! The tray under Linux: a StatusNotifierItem over D-Bus (`ksni`, no GTK).

use std::sync::Arc;

use ksni::menu::StandardItem;
use ksni::{Category, MenuItem, ToolTip, TrayMethods};
use tokio::sync::mpsc::UnboundedSender;

use super::{follow, Action, TrayModel};
use crate::api::{self, Call};
use crate::icons;

/// The model, as a StatusNotifierItem.
pub struct AgentTray(pub TrayModel);

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
        icons::pixmaps(self.0.icon())
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
            description: self.0.state().to_owned(),
            ..Default::default()
        }
    }

    /// C#: a click on the icon opens the window.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.0.act(Action::Open);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        Action::MENU
            .iter()
            .map(|entry| match *entry {
                Some(action) => StandardItem {
                    label: action.label().to_owned(),
                    activate: Box::new(move |t: &mut AgentTray| t.0.act(action)),
                    ..Default::default()
                }
                .into(),
                None => MenuItem::Separator,
            })
            .collect()
    }
}

/// Puts the tray up and keeps it on the agent's state until the agent goes. Without a
/// tray host (no StatusNotifierWatcher: a bare session, a test runner) it says so and
/// returns: the agent runs on without one.
pub async fn run(calls: UnboundedSender<Call>, open: Arc<dyn Fn() + Send + Sync>) {
    let (client, frames) = api::in_process();
    let model = TrayModel::new(calls.clone(), client.clone(), open);
    let tray = match AgentTray(model).spawn().await {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("[lbm-agent] no tray: {error}");
            return;
        }
    };
    let (updates, mut pending) = tokio::sync::mpsc::unbounded_channel();
    let showing = async {
        while let Some(state) = pending.recv().await {
            if tray
                .update(|t: &mut AgentTray| t.0.show(&state))
                .await
                .is_none()
            {
                return;
            }
        }
    };
    let following = follow(&calls, client, frames, |state| {
        updates.send(state.clone()).is_ok()
    });
    tokio::select! {
        _ = showing => {}
        _ = following => {}
    }
    tray.shutdown().await;
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use ksni::Tray;

    use super::*;

    #[test]
    fn the_menu_is_the_models() {
        let (model, mut calls, opened) = super::super::tests::model();
        let mut tray = AgentTray(model);
        let labels: Vec<String> = tray
            .menu()
            .iter()
            .map(|item| match item {
                MenuItem::Standard(s) => s.label.clone(),
                MenuItem::Separator => "-".to_owned(),
                _ => "?".to_owned(),
            })
            .collect();
        assert_eq!(labels, ["Open", "Start", "Stop", "Refresh", "-", "Exit"]);

        let MenuItem::Standard(stop) = tray.menu().remove(2) else {
            panic!("an entry");
        };
        (stop.activate)(&mut tray);
        assert_eq!(calls.try_recv().unwrap().request, crate::api::Request::Stop);
        tray.activate(0, 0);
        assert_eq!(opened.load(Ordering::Relaxed), 1);
        assert_eq!(tray.icon_pixmap().len(), 6);
    }
}
