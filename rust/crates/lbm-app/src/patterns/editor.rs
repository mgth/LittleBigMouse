//! Temporarily raise only the owning editor above KWin's Top-layer pattern OSDs.
//! Winit's WindowLevel is a no-op on Wayland. KWin's scripting API also works
//! when its privileged plasma-window-management protocol is not advertised.
use std::{
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::sync::mpsc;

struct Reply(mpsc::Sender<String>);
#[zbus::interface(name = "org.littlebigmouse.PatternEditor")]
impl Reply {
    fn report(&self, value: String) {
        let _ = self.0.try_send(value);
    }
}

pub struct Guard {
    runtime: tokio::runtime::Runtime,
    connection: zbus::Connection,
    replies: mpsc::Receiver<String>,
    service: String,
    pid: u32,
    window: Option<String>,
    above: bool,
    below: bool,
}
impl Guard {
    pub fn new(pid: u32) -> Result<Self, String> {
        Self::prepare(pid)
            .map_err(|e| format!("Could not keep LittleBigMouse above the patterns: {e}"))
    }
    fn connect(pid: u32) -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let service = format!(
            "org.littlebigmouse.PatternEditor.p{}.n{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (tx, replies) = mpsc::channel(8);
        let connection = runtime.block_on(async {
            zbus::connection::Builder::session()?
                .name(service.as_str())?
                .serve_at("/", Reply(tx))?
                .build()
                .await
        })?;
        Ok(Self {
            runtime,
            connection,
            replies,
            service,
            pid,
            window: None,
            above: false,
            below: false,
        })
    }
    fn prepare(pid: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut guard = Self::connect(pid)?;
        // Capture the original state before making any changes, so all error paths restore it.
        let state = guard.run(&format!(
            "var w = workspace.windowList().find(w => w.pid === {pid} && w.normalWindow && !w.deleted);\n\
             if (!w) throw new Error('LittleBigMouse window not found');\n\
             return {{id: String(w.internalId), above: w.keepAbove, below: w.keepBelow}};"
        ))?;
        guard.window = Some(
            state["id"]
                .as_str()
                .ok_or("Invalid window identity")?
                .into(),
        );
        guard.above = state["above"].as_bool().ok_or("Missing window state")?;
        guard.below = state["below"].as_bool().ok_or("Missing window state")?;
        guard.raise()?;
        Ok(guard)
    }
    fn selector(&self) -> String {
        let id = serde_json::to_string(&self.window).expect("window identity");
        format!("var w = workspace.windowList().find(w => w.pid === {} && String(w.internalId) === {id} && !w.deleted);", self.pid)
    }
    pub fn raise(&mut self) -> Result<(), String> {
        self.run(&format!(
            "{}\nif (!w) throw new Error('LittleBigMouse window closed');\n\
             w.keepBelow = false; w.keepAbove = true; workspace.activeWindow = w; workspace.raiseWindow(w);\n\
             if (!w.keepAbove) throw new Error('KDE refused to keep the editor above the patterns'); return true;", self.selector()
        )).map(|_| ())
    }
    fn run(&mut self, body: &str) -> Result<serde_json::Value, String> {
        let service = serde_json::to_string(&self.service).expect("service name");
        let source = format!(
            "var result; try {{ result = {{value: (function() {{ {body} }})()}}; }}\n\
             catch (e) {{ result = {{error: String(e)}}; }}\n\
             callDBus({service}, '/', 'org.littlebigmouse.PatternEditor', 'Report', JSON.stringify(result));"
        );
        let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        file.write_all(source.as_bytes())
            .map_err(|e| e.to_string())?;
        self.runtime.block_on(async {
            let scripting = zbus::Proxy::new(
                &self.connection,
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting",
            )
            .await
            .map_err(|e| e.to_string())?;
            let plugin = &self.service;
            let id: i32 = tokio::time::timeout(
                Duration::from_secs(2),
                scripting.call(
                    "loadScript",
                    &(file.path().to_string_lossy().as_ref(), plugin.as_str()),
                ),
            )
            .await
            .map_err(|_| "KDE scripting did not respond".to_string())?
            .map_err(|e| e.to_string())?;
            if id < 0 {
                return Err("KDE could not load the temporary window helper".into());
            }
            let result = tokio::time::timeout(Duration::from_secs(2), async {
                let script = zbus::Proxy::new(
                    &self.connection,
                    "org.kde.KWin",
                    format!("/Scripting/Script{id}"),
                    "org.kde.kwin.Script",
                )
                .await
                .map_err(|e| e.to_string())?;
                script
                    .call::<_, _, ()>("run", &())
                    .await
                    .map_err(|e| e.to_string())?;
                let reply = self.replies.recv().await.ok_or("KDE helper disconnected")?;
                let value: serde_json::Value =
                    serde_json::from_str(&reply).map_err(|e| e.to_string())?;
                if let Some(error) = value.get("error") {
                    return Err(error.to_string());
                }
                Ok(value["value"].clone())
            })
            .await
            .map_err(|_| "KDE window helper timed out".to_string())
            .and_then(|r| r);
            let _ = tokio::time::timeout(
                Duration::from_secs(1),
                scripting.call_method("unloadScript", &(plugin.as_str(),)),
            )
            .await;
            result
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.window.is_some() {
            let body = format!(
                "{}\nif (w) {{ w.keepAbove = {}; w.keepBelow = {}; }} return true;",
                self.selector(),
                self.above,
                self.below
            );
            if let Err(error) = self.run(&body) {
                eprintln!("[patterns] Could not restore editor stacking: {error}");
            }
        }
    }
}

/// Restore and activate the editor without changing its keep-above preference.
/// Winit cannot focus or unminimize native Wayland windows by itself.
pub fn activate() -> Result<(), String> {
    let pid = std::process::id();
    let mut connection = Guard::connect(pid).map_err(|e| e.to_string())?;
    connection
        .run(&format!(
        "var w = workspace.windowList().find(w => w.pid === {pid} && w.normalWindow && !w.deleted);
\
         if (!w) throw new Error('LittleBigMouse window not found');
\
         w.minimized = false; workspace.activeWindow = w; workspace.raiseWindow(w); return true;"
    ))
        .map(|_| ())
}
