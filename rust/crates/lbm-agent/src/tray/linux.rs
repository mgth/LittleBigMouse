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
///
/// The icon is registered when there is one to show and unregistered when the user hides
/// it, rather than being left in place as a passive item: hosts differ on what they do
/// with a passive item, and "hide the icon" has to mean the same thing everywhere.
pub async fn run(calls: UnboundedSender<Call>, open: Arc<dyn Fn() + Send + Sync>) {
    let (client, frames) = api::in_process();
    let (updates, mut pending) = tokio::sync::mpsc::unbounded_channel();
    let theirs = client.clone();
    let showing = async {
        let mut shown: Option<ksni::Handle<AgentTray>> = None;
        while let Some(state) = pending.recv().await {
            let mut model = TrayModel::new(calls.clone(), theirs.clone(), open.clone());
            model.show(&state);
            match (model.hidden(), shown.take()) {
                (true, Some(handle)) => handle.shutdown().await,
                (true, None) => {}
                (false, Some(handle)) => {
                    // The handle owns the model; hand it the new state rather than the
                    // one built here. A handle whose tray is gone answers None.
                    if handle
                        .update(|t: &mut AgentTray| t.0.show(&state))
                        .await
                        .is_none()
                    {
                        return;
                    }
                    shown = Some(handle);
                }
                (false, None) => match AgentTray(model).spawn().await {
                    Ok(handle) => shown = Some(handle),
                    Err(error) => {
                        eprintln!("[lbm-agent] no tray: {error}");
                        return;
                    }
                },
            }
        }
        if let Some(handle) = shown {
            handle.shutdown().await;
        }
    };
    let following = follow(&calls, client, frames, |state| {
        updates.send(state.clone()).is_ok()
    });
    tokio::select! {
        _ = showing => {}
        _ = following => {}
    }
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
