//! The frontends' API over a real endpoint (a Unix socket, a Windows pipe), against an
//! agent driving the fake hook.

use std::io;
use std::time::Duration;

use lbm_agent::fake_hook::FakeHook;
use lbm_agent::hook::HookClient;
use lbm_agent::reconcile::{LayoutState, Timings, World};
use lbm_agent::runtime::Agent;
use lbm_agent::world::AgentWorld;
use lbm_ipc::framing::{read_frame, write_frame};
use lbm_ipc::protocol::{Command, Event};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};

const ZONES: &str = r#"<ZonesLayout><MainZones><Zone Id="0"></Zone></MainZones></ZonesLayout>"#;

/// One monitor, a layout the user wants.
struct Enabled(Option<LayoutState>);

/// A compositor double behind the real output validation and the real API/runtime.
struct OutputWorld {
    state: Enabled,
    live: Vec<lbm_layout::linux::LinuxMonitor>,
    commands: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    fail_after_change: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl World for OutputWorld {
    fn display_signature(&mut self) -> String {
        lbm_display::linux::display_signature(&self.live)
    }
    fn rebuild_layout(&mut self) {
        self.state.rebuild_layout();
    }
    fn layout(&self) -> Option<LayoutState> {
        self.state.layout()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.state.set_enabled(enabled);
    }
}
impl AgentWorld for OutputWorld {
    fn topology_requires_stopped_engine(&self) -> bool {
        true
    }
    fn apply_topology(
        &mut self,
        _: &str,
        _: &lbm_store::LayoutDocument,
        _: bool,
        how: lbm_display::linux::topology::How,
    ) -> Result<Vec<String>, String> {
        use std::sync::atomic::Ordering::SeqCst;
        if how == lbm_display::linux::topology::How::ForReal {
            self.commands.fetch_add(1, SeqCst);
            self.live[0].logical_x += 100.;
            if self.fail_after_change.load(SeqCst) {
                return Err("partially applied topology".into());
            }
        }
        Ok(vec!["place Windows screens".into()])
    }
    fn desktop(&self) -> Option<lbm_ipc::protocol::Desktop> {
        None
    }
    fn save_layout(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn zones(&self) -> Option<(String, bool)> {
        Some((ZONES.into(), false))
    }
    fn save_enabled(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn layout_id(&self) -> Option<String> {
        Some("outputs".into())
    }
    fn display_outputs(&mut self) -> Result<lbm_ipc::displays::Outputs, String> {
        Ok(lbm_agent::displays::outputs(self.live.clone()))
    }
    fn change_output(
        &mut self,
        connector: &str,
        change: lbm_ipc::displays::Change,
        signature: &str,
    ) -> Result<lbm_ipc::displays::Outputs, String> {
        use lbm_display::linux::{
            topology::{change_checked_with, Change},
            Backend,
        };
        use std::sync::atomic::Ordering::SeqCst;
        let action = match change {
            lbm_ipc::displays::Change::Primary => Change::Primary,
            lbm_ipc::displays::Change::Attach => Change::Attach,
            lbm_ipc::displays::Change::Detach => Change::Detach,
        };
        let live = std::cell::RefCell::new(&mut self.live);
        change_checked_with(
            Backend::KScreen,
            action,
            connector,
            signature,
            || Ok(live.borrow().to_vec()),
            |_, _| {
                self.commands.fetch_add(1, SeqCst);
                let mut live = live.borrow_mut();
                for m in live.iter_mut() {
                    if action == Change::Primary {
                        m.primary = m.connector_name == connector;
                    } else if m.connector_name == connector {
                        m.enabled = action == Change::Attach;
                    }
                }
                if self.fail_after_change.load(SeqCst) {
                    Err("partially applied".into())
                } else {
                    Ok(())
                }
            },
        )
        .map(lbm_agent::displays::outputs)
    }
}

impl World for Enabled {
    fn display_signature(&mut self) -> String {
        "one-monitor".into()
    }
    fn rebuild_layout(&mut self) {
        self.0.get_or_insert(LayoutState {
            enabled: true,
            is_virtual: false,
            saved: true,
        });
    }
    fn layout(&self) -> Option<LayoutState> {
        self.0
    }
    fn set_enabled(&mut self, enabled: bool) {
        if let Some(l) = &mut self.0 {
            l.enabled = enabled;
        }
    }
}

impl AgentWorld for Enabled {
    fn zones(&self) -> Option<(String, bool)> {
        self.0.map(|_| (ZONES.to_owned(), false))
    }

    /// The desktop the fixture's zones sit on. Named rather than inferred, as the real
    /// world names it: a world that answered None would be exercising the fallback.
    fn desktop(&self) -> Option<lbm_ipc::protocol::Desktop> {
        self.0.map(|_| lbm_ipc::protocol::Desktop {
            left: 0,
            top: 0,
            width: 3840,
            height: 1080,
        })
    }
    fn save_enabled(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn save_layout(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn layout_id(&self) -> Option<String> {
        self.0.map(|_| "DELA0B1_ABC123".to_owned())
    }
}

/// The hook's and the agent's endpoints for one test: in its directory, or pipes named
/// after it on Windows.
#[cfg(unix)]
fn endpoints(dir: &tempfile::TempDir) -> (String, String) {
    let at = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
    (at("hook.sock"), at("lbm-agent.sock"))
}

#[cfg(windows)]
fn endpoints(_dir: &tempfile::TempDir) -> (String, String) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    (
        format!(r"\\.\pipe\lbm-agent-api-test-hook-{pid}-{n}"),
        format!(r"\\.\pipe\lbm-agent-api-test-{pid}-{n}"),
    )
}

#[cfg(unix)]
fn listen(
    endpoint: &str,
) -> (
    tokio::sync::mpsc::UnboundedReceiver<lbm_agent::api::Call>,
    lbm_agent::api::Listener,
) {
    lbm_agent::api::listen(endpoint).unwrap()
}

#[cfg(windows)]
fn listen(
    endpoint: &str,
) -> (
    tokio::sync::mpsc::UnboundedReceiver<lbm_agent::api::Call>,
    lbm_agent::api::Listener,
) {
    lbm_agent::api::listen_pipe(endpoint).unwrap()
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<S: AsyncRead + AsyncWrite + Unpin + Send> Stream for S {}

#[cfg(unix)]
async fn open(endpoint: &str) -> Box<dyn Stream> {
    Box::new(tokio::net::UnixStream::connect(endpoint).await.unwrap())
}

/// A pipe busy with the previous client is waited for, as clients do.
#[cfg(windows)]
async fn open(endpoint: &str) -> Box<dyn Stream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    for _ in 0..250 {
        match ClientOptions::new().open(endpoint) {
            Ok(pipe) => return Box::new(pipe),
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    panic!("the pipe {endpoint} never opened");
}

/// A frontend: requests out, answers and events in.
struct Frontend {
    stream: Box<dyn Stream>,
    next: u64,
}

impl Frontend {
    async fn connect(endpoint: &str) -> Frontend {
        Frontend {
            stream: open(endpoint).await,
            next: 0,
        }
    }

    async fn send(&mut self, mut request: Value) -> u64 {
        self.next += 1;
        request["Id"] = json!(self.next);
        write_frame(&mut self.stream, &request.to_string())
            .await
            .unwrap();
        self.next
    }

    async fn receive(&mut self) -> Value {
        let frame = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut self.stream))
            .await
            .expect("a frame within 5 s")
            .unwrap();
        serde_json::from_str(&frame).unwrap()
    }

    /// The answer to `request`, skipping events.
    async fn ask(&mut self, request: Value) -> Value {
        let id = self.send(request).await;
        loop {
            let frame = self.receive().await;
            if frame["Id"] == json!(id) {
                return frame;
            }
        }
    }

    /// The next `Hook` event named `name`: its payload.
    async fn hook(&mut self, name: &str) -> String {
        loop {
            let frame = self.receive().await;
            if frame["Event"] == "Hook" && frame["Hook"] == name {
                return frame["Payload"].as_str().unwrap().to_owned();
            }
        }
    }

    /// The next `State` event whose snapshot satisfies `want`.
    async fn state(&mut self, want: impl Fn(&Value) -> bool) -> Value {
        loop {
            let frame = self.receive().await;
            if frame["Event"] == "State" && want(&frame["State"]) {
                return frame["State"].clone();
            }
        }
    }
}

#[tokio::test]
async fn a_frontend_sees_the_agent_and_drives_it() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let agent = tokio::spawn(async move {
        let mut agent = Agent::new(Enabled(None), Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    let hello = frontend
        .ask(json!({ "Method": "Hello", "Client": "test" }))
        .await;
    assert_eq!(hello["Result"]["Agent"], "lbm-agent");
    assert_eq!(hello["Result"]["Protocol"], 3);

    // Subscribed: the state now, then every change — here, the hook taking the layout.
    let subscribed = frontend.ask(json!({ "Method": "Subscribe" })).await;
    assert_eq!(subscribed["Result"]["LayoutId"], "DELA0B1_ABC123");
    let running = frontend
        .state(|s| s["Engine"] == "Running" && s["HookConnected"] == true)
        .await;
    assert_eq!(running["Enabled"], true);
    assert!(fake.hooked());

    // The user's Stop, from the frontend: recorded, unhooked, and seen.
    let stop = frontend.ask(json!({ "Method": "Stop" })).await;
    assert_eq!(stop["Result"], Value::Null);
    let stopped = frontend
        .state(|s| s["Engine"] == "Stopped" && s["Enabled"] == false)
        .await;
    assert_eq!(stopped["LayoutId"], "DELA0B1_ABC123");
    assert!(!fake.hooked());

    // Anything else is refused, not guessed at; a snapshot is always there.
    let unknown = frontend.ask(json!({ "Method": "Teleport" })).await;
    assert!(unknown["Error"]
        .as_str()
        .unwrap()
        .starts_with("bad request"));
    let snapshot = frontend.ask(json!({ "Method": "Snapshot" })).await;
    assert_eq!(snapshot["Result"]["Engine"], "Stopped");

    // Quit: the hook is told to leave, and so does the agent.
    let quit = frontend.ask(json!({ "Method": "Quit" })).await;
    assert_eq!(quit["Result"], Value::Null);
    tokio::time::timeout(Duration::from_secs(5), agent)
        .await
        .expect("the agent left")
        .unwrap();
    // The agent left only once its Quit was written: the hook has it.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(fake.received().last(), Some(Command::Quit)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the hook was told to quit");
}

/// The hook says it once, and usually with no window open: the state is what keeps it.
#[tokio::test]
async fn a_window_opened_later_learns_the_shortcut_could_not_be_armed() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    tokio::spawn(async move {
        let mut agent = Agent::new(Enabled(None), Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut first = Frontend::connect(&api_endpoint).await;
    first.ask(json!({ "Method": "Subscribe" })).await;
    first.state(|s| s["HookConnected"] == true).await;
    fake.broadcast(&Event::ShortcutUnavailable {
        shortcut: "Ctrl+Alt+F9".to_string(),
    });
    first.hook("ShortcutUnavailable").await;

    let mut later = Frontend::connect(&api_endpoint).await;
    let state = later.ask(json!({ "Method": "Subscribe" })).await;
    assert_eq!(state["Result"]["ShortcutUnavailable"], "Ctrl+Alt+F9");
}

#[tokio::test]
async fn a_subscriber_hears_the_hook_and_what_it_saw() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    tokio::spawn(async move {
        let mut agent = Agent::new(Enabled(None), Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    // Subscribed before the hook takes the layout: its own words come through.
    let mut frontend = Frontend::connect(&api_endpoint).await;
    frontend.ask(json!({ "Method": "Subscribe" })).await;
    frontend
        .state(|s| s["Engine"] == "Running" && s["HookConnected"] == true)
        .await;
    assert!(fake.hooked());

    // The probe report is the agent's own: it holds the zones the report is about.
    let probe = frontend.ask(json!({ "Method": "Probe" })).await;
    assert_eq!(probe["Result"], Value::Null);
    let report = frontend.hook("Probed").await;
    assert!(report.starts_with("<ProbeReport"), "{report}");

    // The foreground processes: each forwarded, each remembered once.
    for process in ["/usr/bin/kate", "/usr/bin/firefox", "/usr/bin/kate", ""] {
        fake.broadcast(&Event::FocusChanged {
            process: process.to_string(),
        });
    }
    assert_eq!(frontend.hook("FocusChanged").await, "/usr/bin/kate");
    assert_eq!(frontend.hook("FocusChanged").await, "/usr/bin/firefox");
    assert_eq!(frontend.hook("FocusChanged").await, "/usr/bin/kate");
    assert_eq!(frontend.hook("FocusChanged").await, "");
    let seen = frontend.ask(json!({ "Method": "SeenProcesses" })).await;
    assert_eq!(seen["Result"], json!(["/usr/bin/kate", "/usr/bin/firefox"]));

    // The hook goes away: said as C# says it. The probe still answers — it never
    // needed the hook, and the editor asks for it over layouts no hook will ever run.
    drop(fake);
    frontend.hook("Dead").await;
    let without_a_hook = frontend.ask(json!({ "Method": "Probe" })).await;
    assert_eq!(without_a_hook["Result"], Value::Null);
    assert!(frontend.hook("Probed").await.starts_with("<ProbeReport"));
}

/// The real world over a store in `dir`: no display backend, so one fallback output.
fn system_world(
    dir: &std::path::Path,
) -> lbm_agent::world::SystemWorld<lbm_store::JsonLayoutStore, lbm_agent::world::Platform> {
    lbm_agent::world::SystemWorld::new(lbm_agent::discovery::Discovery::Fallback, persistence(dir))
        .with_wallpaper(dir.join("wallpaper.json"), dir.join("wallpapers"))
}

fn persistence(
    dir: &std::path::Path,
) -> lbm_store::LayoutPersistence<lbm_store::JsonLayoutStore, lbm_agent::world::Platform> {
    let excluded = dir.join("Excluded.txt");
    // The session autostart of this test: an entry in its own directory, never the user's.
    let autostart = lbm_agent::autostart::XdgAutostart::new(
        dir.join("autostart"),
        Vec::new(),
        std::path::PathBuf::from("/usr/bin/lbm-agent"),
    );
    lbm_store::LayoutPersistence::with_excluded_list_file(
        lbm_store::JsonLayoutStore::new(dir.join("config")),
        lbm_agent::world::Platform::with_autostart(autostart),
        move || excluded.clone(),
    )
}

/// What a frontend holds: the same layout, built from the same displays and store.
fn frontend_layout(dir: &std::path::Path) -> lbm_layout::model::Layout {
    let mut layout = lbm_layout::model::Layout::new(lbm_layout::model::LayoutOptions::default());
    let mut persistence = persistence(dir);
    lbm_layout::linux::populate(&mut layout, &[], |l| persistence.load(l)).unwrap();
    layout
}

fn stored_layout(dir: &std::path::Path) -> Value {
    let layouts = dir.join("config").join("layouts");
    let file = std::fs::read_dir(&layouts)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn loads(fake: &FakeHook) -> usize {
    fake.received()
        .iter()
        .filter(|c| matches!(c, Command::Load { .. }))
        .count()
}

#[tokio::test]
async fn the_agent_writes_the_wallpaper_settings_a_frontend_sends() {
    // The frontends edit; the agent writes and paints. The file is the C# plugin's own,
    // so what lands on disk has to be what that plugin reads back.
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    let layout_id = state["LayoutId"].as_str().unwrap().to_owned();
    let screen = frontend_layout(dir.path()).monitors()[0].id.clone();

    let settings = json!({
        "Mode": "PerScreen",
        "PerScreen": { screen.clone(): {
            "Kind": 1,
            "Style": "Fill",
            "Color": "#102030",
        }},
    });
    let saved = frontend
        .ask(json!({
            "Method": "SaveWallpaper",
            "LayoutId": layout_id.clone(),
            "Settings": settings,
        }))
        .await;
    assert_eq!(saved["Result"], Value::Null, "{saved}");

    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("wallpaper.json")).unwrap())
            .unwrap();
    assert_eq!(written[&layout_id]["Mode"], "PerScreen");
    assert_eq!(
        written[&layout_id]["PerScreen"][&screen]["Color"],
        "#102030"
    );
    assert_eq!(
        written[&layout_id]["PerScreen"][&screen]["Kind"], 1,
        "Kind travels as the number System.Text.Json writes"
    );

    // Another layout's settings are not this agent's to write: the displays changed
    // under the editor, exactly as for an edit.
    let refused = frontend
        .ask(json!({
            "Method": "SaveWallpaper",
            "LayoutId": "SOMEONE_ELSE",
            "Settings": settings,
        }))
        .await;
    assert!(
        refused["Error"]
            .as_str()
            .unwrap_or_default()
            .contains("not the current one"),
        "{refused}"
    );
}

#[tokio::test]
async fn the_agent_writes_what_a_frontend_edits_and_previews_it_first() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    // A first run: the layout exists, the engine is off.
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    let layout_id = state["LayoutId"].as_str().unwrap().to_owned();
    assert_eq!(state["Enabled"], false);
    assert_eq!(state["Previewing"], false);
    let unsupported = frontend.ask(json!({"Method":"DisplayOutputs"})).await;
    assert!(
        unsupported["Error"].is_string(),
        "fallback discovery must not control real outputs"
    );

    // The editor moves the monitor and turns LoopX on.
    let mut edited = frontend_layout(dir.path());
    assert_eq!(edited.id, layout_id);
    let monitor = edited.monitors()[0].id.clone();
    edited.set_location(&monitor, lbm_layout::geo::Point::new(40.0, 20.0));
    edited.edit_options(|o| o.loop_x = true);
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&edited)).unwrap();

    // Live preview: the hook runs the edit, nothing is saved.
    let preview = json!({ "Method": "Preview", "LayoutId": layout_id, "Document": document });
    assert_eq!(frontend.ask(preview.clone()).await["Result"], Value::Null);
    frontend
        .state(|s| s["Previewing"] == true && s["Engine"] == "Running")
        .await;
    assert!(fake.hooked());
    assert_eq!(loads(&fake), 1);
    let refused = frontend.ask(json!({"Method":"ChangeOutput", "LayoutId":layout_id,
        "LayoutRevision":state["LayoutRevision"], "Signature":"unused", "Connector":"FALLBACK", "Change":"Detach"})).await;
    assert!(refused["Error"]
        .as_str()
        .unwrap()
        .contains("stop the engine"));
    assert!(!dir.path().join("config").join("layouts").exists());

    // The same edit again: the hook is not made to swap a layout for itself.
    frontend.ask(preview).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(loads(&fake), 1);

    // The preview ends: the engine was off, it goes back off.
    frontend.ask(json!({ "Method": "EndPreview" })).await;
    frontend
        .state(|s| s["Previewing"] == false && s["Engine"] == "Stopped")
        .await;
    assert!(!fake.hooked());

    // Saved by the agent, for the layout the editor edits only.
    let elsewhere = frontend
        .ask(json!({ "Method": "SaveLayout", "LayoutId": "SOMEONE_ELSE", "Document": document }))
        .await;
    assert!(elsewhere["Error"]
        .as_str()
        .unwrap()
        .contains("not the current one"));
    let saved = frontend
        .ask(json!({ "Method": "SaveLayout", "LayoutId": layout_id, "Document": document }))
        .await;
    assert_eq!(saved["Result"], Value::Null);
    assert_eq!(stored_layout(dir.path())["Options"]["LoopX"], true);
    let snapshot = frontend.ask(json!({ "Method": "Snapshot" })).await;
    assert_eq!(snapshot["Result"]["Saved"], true);

    // Apply and start: the next edit is saved, recorded Enabled, and hooked.
    edited.edit_options(|o| o.loop_y = true);
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&edited)).unwrap();
    let start = frontend
        .ask(json!({ "Method": "Start", "LayoutId": layout_id, "Document": document }))
        .await;
    assert_eq!(start["Result"], Value::Null);
    frontend
        .state(|s| s["Engine"] == "Running" && s["Enabled"] == true)
        .await;
    let stored = stored_layout(dir.path());
    assert_eq!(stored["Options"]["LoopY"], true);
    assert_eq!(stored["Options"]["Enabled"], true);

    // The app-level options and the excluded list, written at once.
    let options = frontend
        .ask(json!({
            "Method": "SaveOptions",
            "Options": { "RescueShortcut": "Ctrl+Alt+F11" },
            "Excluded": ["/usr/bin/steam"],
            "LoadAtStartup": true,
        }))
        .await;
    assert_eq!(options["Result"], Value::Null);
    let global: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("config").join("options.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(global["RescueShortcut"], "Ctrl+Alt+F11");
    let excluded = std::fs::read_to_string(dir.path().join("Excluded.txt")).unwrap();
    assert!(excluded.lines().any(|l| l == "/usr/bin/steam"));

    // LoadAtStartup is the session autostart itself: the entry is there, and the state
    // says so.
    let entry = dir
        .path()
        .join("autostart")
        .join(lbm_agent::autostart::ENTRY);
    assert!(std::fs::read_to_string(&entry).unwrap().contains("Exec="));
    let state = frontend.ask(json!({ "Method": "Snapshot" })).await["Result"].clone();
    assert_eq!(state["LoadAtStartup"], true);
    assert_eq!(state["HideTrayIcon"], false);

    frontend
        .ask(json!({ "Method": "SaveOptions", "LoadAtStartup": false }))
        .await;
    assert!(!entry.exists(), "the session no longer starts the agent");
}

/// A session autostart that cannot be set up refuses the request, but only its own part:
/// the other options of the same request are applied, stored, and reach the hook.
#[tokio::test]
async fn an_autostart_that_fails_does_not_keep_the_other_options_from_the_hook() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();
    // The autostart directory is a file: no entry can be written in it.
    std::fs::write(dir.path().join("autostart"), "not a directory").unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    frontend.ask(json!({ "Method": "Subscribe" })).await;
    assert!(!fake.bound());

    let refused = frontend
        .ask(json!({
            "Method": "SaveOptions",
            "Options": { "BoundToAgent": true },
            "Excluded": ["/usr/bin/steam"],
            "LoadAtStartup": true,
        }))
        .await;
    assert!(
        refused["Error"].as_str().unwrap().contains("autostart"),
        "{refused}"
    );

    // Stored...
    let excluded = std::fs::read_to_string(dir.path().join("Excluded.txt")).unwrap();
    assert!(excluded.lines().any(|l| l == "/usr/bin/steam"));
    // ...told to the hook...
    tokio::time::timeout(Duration::from_secs(5), async {
        while !(fake.bound()
            && fake
                .excluded()
                .is_some_and(|e| e.iter().any(|p| p == "/usr/bin/steam")))
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the hook never got the options the store holds");
    // ...and the autostart reported as what the session does: nothing.
    let state = frontend.ask(json!({ "Method": "Snapshot" })).await["Result"].clone();
    assert_eq!(state["LoadAtStartup"], false);
}

#[tokio::test]
async fn probing_an_edit_is_correlated_and_leaves_the_running_preview_and_store_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();
    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({"Method":"Subscribe"})).await["Result"].clone();
    let id = state["LayoutId"].as_str().unwrap();
    let revision = state["LayoutRevision"].as_u64().unwrap();
    let mut edited = frontend_layout(dir.path());
    let source = edited.monitors()[0].active_source.clone().unwrap();

    // Run the original arrangement as a preview, then probe a different, unsaved edit.
    frontend
        .ask(json!({"Method":"Preview", "LayoutId":id,
        "Document":lbm_store::LayoutDocument::of(&edited)}))
        .await;
    frontend
        .state(|s| s["Previewing"] == true && s["Engine"] == "Running")
        .await;
    frontend.ask(json!({"Method":"Probe"})).await;
    let original = frontend.hook("Probed").await;
    let commands = fake.received();
    let excluded = std::fs::read(dir.path().join("Excluded.txt")).unwrap();
    let options = std::fs::read(dir.path().join("config/options.json")).ok();
    edited.edit_options(|o| o.loop_x = true);
    let request = json!({"Method":"ProbeLayout", "LayoutId":id,
        "LayoutRevision":revision, "Document":lbm_store::LayoutDocument::of(&edited)});

    let answer = frontend.ask(request.clone()).await;
    let report: lbm_ipc::probe::Report = serde_json::from_value(answer["Result"].clone())
        .unwrap_or_else(|e| panic!("{e}: {answer}"));
    assert_eq!(report.layout_id, id);
    assert_eq!(report.layout_revision, revision);
    assert_eq!(report.zones.len(), 1);
    let zone = &report.zones[0];
    assert_eq!(
        zone.device_id, source,
        "associate by active source, not monitor id"
    );
    assert_eq!(zone.edges.len(), 4);
    for edge in &zone.edges {
        let target = match edge.side {
            lbm_ipc::probe::Side::Left | lbm_ipc::probe::Side::Right => Some(zone.id),
            _ => None,
        };
        assert!(!edge.runs.is_empty());
        assert!(
            edge.runs
                .iter()
                .all(|r| r.target == target && r.from <= r.to),
            "{edge:?}"
        );
    }

    // A second client does not hear the first client's JSON sweep as a Probed event.
    let mut observer = Frontend::connect(&api_endpoint).await;
    observer.ask(json!({"Method":"Subscribe"})).await;
    frontend.ask(request.clone()).await;
    let barrier = observer.send(json!({"Method":"Snapshot"})).await;
    loop {
        let frame = observer.receive().await;
        assert_ne!(frame["Hook"], "Probed", "JSON probes are private replies");
        if frame["Id"] == barrier {
            break;
        }
    }

    let mut stale = request.clone();
    stale["LayoutRevision"] = json!(revision + 1);
    assert!(frontend.ask(stale).await["Error"].is_string());
    let mut elsewhere = request;
    elsewhere["LayoutId"] = json!("another desktop");
    assert!(frontend.ask(elsewhere).await["Error"].is_string());

    // The legacy report still describes the active preview, byte for byte.
    frontend.ask(json!({"Method":"Probe"})).await;
    assert_eq!(frontend.hook("Probed").await, original);
    let state = frontend.ask(json!({"Method":"Snapshot"})).await["Result"].clone();
    assert_eq!(state["Previewing"], true);
    assert_eq!(state["Engine"], "Running");
    assert_eq!(state["Enabled"], false);
    assert_eq!(
        fake.received(),
        commands,
        "probing must never command the hook"
    );
    assert_eq!(
        std::fs::read(dir.path().join("Excluded.txt")).unwrap(),
        excluded
    );
    assert!(!dir.path().join("config/layouts").exists());
    assert_eq!(
        std::fs::read(dir.path().join("config/options.json")).ok(),
        options
    );
    assert!(!dir.path().join("wallpaper.json").exists());

    frontend.ask(json!({"Method":"EndPreview"})).await;
    frontend
        .state(|s| s["Previewing"] == false && s["Engine"] == "Stopped")
        .await;
}

/// A preview belongs to the connection that asked for it, and dies with it.
///
/// A window that exits cleanly sends `EndPreview`. A window that is killed, crashes, or
/// whose machine drops off the network sends nothing — and a preview is a `Load` *and* a
/// `Run`, so without this the hook would go on driving an arrangement nobody saved, with
/// nothing left on screen able to stop it.
#[tokio::test]
async fn a_preview_ends_when_the_frontend_that_asked_for_it_goes_and_not_before() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    // The editor previews its edit.
    let mut editor = Frontend::connect(&api_endpoint).await;
    let state = editor.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    let layout_id = state["LayoutId"].as_str().unwrap().to_owned();
    let mut edited = frontend_layout(dir.path());
    let monitor = edited.monitors()[0].id.clone();
    edited.set_location(&monitor, lbm_layout::geo::Point::new(40.0, 20.0));
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&edited)).unwrap();
    let preview = json!({ "Method": "Preview", "LayoutId": layout_id, "Document": document });
    assert_eq!(editor.ask(preview).await["Result"], Value::Null);
    editor
        .state(|s| s["Previewing"] == true && s["Engine"] == "Running")
        .await;
    assert!(fake.hooked());

    // A watcher — the tray, another window — sees the preview, and is who we ask after
    // the editor is gone.
    let mut watcher = Frontend::connect(&api_endpoint).await;
    let seen = watcher.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    assert_eq!(seen["Previewing"], true);

    // Someone else's connection ends. It is not their preview to end.
    let passerby = Frontend::connect(&api_endpoint).await;
    let answer = Frontend::connect(&api_endpoint).await;
    drop(passerby);
    drop(answer);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(fake.hooked(), "a passerby leaving ended someone's preview");
    assert_eq!(
        watcher.ask(json!({ "Method": "Snapshot" })).await["Result"]["Previewing"],
        true
    );

    // The editor is killed: no EndPreview, just a socket that closes.
    drop(editor);
    let stopped = watcher
        .state(|s| s["Previewing"] == false && s["Engine"] == "Stopped")
        .await;
    assert_eq!(stopped["LayoutId"], layout_id);
    assert!(!fake.hooked(), "the hook is left driving the preview");
    assert!(
        !dir.path().join("config").join("layouts").exists(),
        "a preview is never written, however it ends"
    );
}

/// The frontend's apply request, parsed by the agent's own type.
///
/// The screens really move when this succeeds, so the shape is worth pinning: a
/// misspelled member would land as a default — `AdjustScale` silently false, or worse a
/// document that does not parse at all and an apply that never happens.
#[test]
fn an_apply_topology_request_carries_the_document_and_the_scale_choice() {
    let frame: lbm_agent::api::RequestFrame = serde_json::from_value(serde_json::json!({
        "Id": 9,
        "Method": "ApplyTopology",
        "LayoutId": "TESTMON1",
        "Document": { "Layout": { "Options": {}, "Monitors": {} } },
        "AdjustScale": true
    }))
    .expect("the agent must understand what the window sends");

    match frame.request {
        lbm_agent::api::Request::ApplyTopology {
            layout_id,
            document,
            adjust_scale,
            dry_run,
            layout_revision,
        } => {
            assert_eq!(
                layout_revision, None,
                "older Linux clients remain compatible"
            );
            assert_eq!(layout_id, "TESTMON1");
            assert!(adjust_scale);
            assert!(document.layout.is_some());
            assert!(
                !dry_run,
                "a request that does not ask for a dry run really moves the screens"
            );
        }
        other => panic!("parsed as something else: {other:?}"),
    }
}

/// A dry run is asked for, never inferred: `DryRun` travels as its own field and the
/// window has to set it. The default is the one that moves screens, on purpose — a
/// frontend that forgot the field gets what the method is named after.
#[test]
fn a_dry_run_is_asked_for_in_so_many_words() {
    let frame: lbm_agent::api::RequestFrame = serde_json::from_value(serde_json::json!({
        "Id": 11,
        "Method": "ApplyTopology",
        "LayoutId": "TESTMON1",
        "Document": {},
        "DryRun": true
    }))
    .expect("the agent must understand a dry run");
    match frame.request {
        lbm_agent::api::Request::ApplyTopology { dry_run, .. } => assert!(dry_run),
        other => panic!("parsed as something else: {other:?}"),
    }
}

/// And without it, the scale adjustment is off — the reading that does nothing extra.
#[test]
fn an_apply_without_the_scale_choice_does_not_adjust_scales() {
    let frame: lbm_agent::api::RequestFrame = serde_json::from_value(serde_json::json!({
        "Id": 10,
        "Method": "ApplyTopology",
        "LayoutId": "TESTMON1",
        "Document": {}
    }))
    .expect("parsed");
    match frame.request {
        lbm_agent::api::Request::ApplyTopology { adjust_scale, .. } => assert!(
            !adjust_scale,
            "a missing AdjustScale must not mean 'rescale every monitor'"
        ),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn monitor_actions_rebuild_the_layout_and_reject_stale_or_running_requests() {
    use lbm_layout::linux::LinuxMonitor;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
        Arc,
    };
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let commands = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(AtomicBool::new(false));
    let world = OutputWorld {
        state: Enabled(Some(LayoutState {
            enabled: false,
            is_virtual: false,
            saved: true,
        })),
        live: vec![
            LinuxMonitor {
                connector_name: "DP-1".into(),
                ..LinuxMonitor::fallback()
            },
            LinuxMonitor {
                connector_name: "HDMI-1".into(),
                primary: false,
                ..LinuxMonitor::fallback()
            },
        ],
        commands: commands.clone(),
        fail_after_change: fail.clone(),
    };
    tokio::spawn(async move {
        let timings = Timings {
            debounce: Duration::from_millis(5),
            stability_step: Duration::from_millis(5),
            ..Default::default()
        };
        let mut agent = Agent::new(world, timings, hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let mut state = frontend.ask(json!({"Method":"Subscribe"})).await["Result"].clone();
    if state["Engine"] != "Stopped" || state["HookConnected"] != true {
        state = frontend
            .state(|s| s["Engine"] == "Stopped" && s["HookConnected"] == true)
            .await;
    }
    let outputs = frontend.ask(json!({"Method":"DisplayOutputs"})).await["Result"].clone();
    let mut request = json!({"Method":"ChangeOutput", "LayoutId":"outputs", "LayoutRevision":state["LayoutRevision"],
        "Connector":"HDMI-1", "Change":"Primary", "Signature":outputs["Signature"]});
    for (field, value) in [
        ("LayoutId", json!("other")),
        ("LayoutRevision", json!(999)),
        ("Signature", json!("stale")),
    ] {
        let mut stale = request.clone();
        stale[field] = value;
        assert!(frontend.ask(stale).await["Error"].is_string());
    }
    assert_eq!(commands.load(SeqCst), 0);
    let result = frontend.ask(request.clone()).await;
    assert_eq!(result["Result"]["Outputs"][1]["Primary"], true, "{result}");
    let old_revision = state["LayoutRevision"].as_u64().unwrap();
    state = frontend
        .state(|s| s["LayoutRevision"].as_u64().unwrap() > old_revision)
        .await;
    assert_eq!(state["LayoutId"], "outputs", "same identity, new revision");
    assert_eq!(commands.load(SeqCst), 1);

    for action in ["Detach", "Attach"] {
        let inventory = frontend.ask(json!({"Method":"DisplayOutputs"})).await["Result"].clone();
        request["LayoutRevision"] = state["LayoutRevision"].clone();
        request["Signature"] = inventory["Signature"].clone();
        request["Change"] = json!(action);
        let result = frontend.ask(request.clone()).await;
        assert_eq!(
            result["Result"]["Outputs"][1]["Enabled"],
            action == "Attach",
            "{result}"
        );
        let old = state["LayoutRevision"].as_u64().unwrap();
        state = frontend
            .state(|s| s["LayoutRevision"].as_u64().unwrap() > old)
            .await;
        if action == "Detach" {
            let mut last = request.clone();
            last["Connector"] = json!("DP-1");
            last["LayoutRevision"] = state["LayoutRevision"].clone();
            last["Signature"] = result["Result"]["Signature"].clone();
            assert!(frontend.ask(last).await["Error"]
                .as_str()
                .unwrap()
                .contains("last enabled"));
        }
    }
    let inventory = frontend.ask(json!({"Method":"DisplayOutputs"})).await["Result"].clone();
    request["LayoutRevision"] = state["LayoutRevision"].clone();
    request["Signature"] = inventory["Signature"].clone();
    request["Change"] = json!("Detach");
    frontend.ask(json!({"Method":"Start"})).await;
    frontend.state(|s| s["Engine"] == "Running").await;
    assert!(frontend.ask(request.clone()).await["Error"]
        .as_str()
        .unwrap()
        .contains("stop the engine"));
    assert_eq!(commands.load(SeqCst), 3);
    frontend.ask(json!({"Method":"Stop"})).await;
    frontend
        .state(|s| s["Engine"] == "Stopped" && s["Enabled"] == false)
        .await;

    fail.store(true, SeqCst);
    assert_eq!(frontend.ask(request).await["Error"], "partially applied");
    let old = state["LayoutRevision"].as_u64().unwrap();
    frontend
        .state(|s| s["LayoutRevision"].as_u64().unwrap() > old)
        .await;
    assert_eq!(commands.load(SeqCst), 4);
}

#[tokio::test]
async fn topology_dry_runs_keep_edits_and_failed_real_applications_rebuild() {
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
        Arc,
    };
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let commands = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(AtomicBool::new(false));
    let world = OutputWorld {
        state: Enabled(Some(LayoutState {
            enabled: false,
            is_virtual: false,
            saved: true,
        })),
        live: vec![lbm_layout::linux::LinuxMonitor::fallback()],
        commands: commands.clone(),
        fail_after_change: fail.clone(),
    };
    tokio::spawn(async move {
        let timings = Timings {
            debounce: Duration::from_millis(5),
            stability_step: Duration::from_millis(5),
            ..Default::default()
        };
        let mut agent = Agent::new(world, timings, hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let mut state = frontend.ask(json!({"Method":"Subscribe"})).await["Result"].clone();
    if state["Engine"] != "Stopped" || state["HookConnected"] != true {
        state = frontend
            .state(|s| s["Engine"] == "Stopped" && s["HookConnected"] == true)
            .await;
    }
    let mut request =
        json!({"Method":"ApplyTopology", "LayoutId":"outputs", "Document":{}, "DryRun":true});
    assert!(frontend.ask(request.clone()).await["Error"]
        .as_str()
        .unwrap()
        .contains("revision"));
    request["LayoutRevision"] = json!(state["LayoutRevision"].as_u64().unwrap() + 1);
    assert!(frontend.ask(request.clone()).await["Error"]
        .as_str()
        .unwrap()
        .contains("reload"));
    request["LayoutRevision"] = state["LayoutRevision"].clone();
    let result = frontend.ask(request.clone()).await;
    assert_eq!(result["Result"]["DryRun"], true, "{result}");
    assert_eq!(result["Result"]["Commands"][0], "place Windows screens");
    // Wait beyond the display debounce, so an accidental rebuild would be observable.
    tokio::time::sleep(Duration::from_millis(80)).await;
    let dry_state = frontend.ask(json!({"Method":"Snapshot"})).await;
    assert_eq!(
        dry_state["Result"]["LayoutRevision"],
        state["LayoutRevision"]
    );
    assert_eq!(commands.load(SeqCst), 0);
    request["DryRun"] = json!(false);
    fail.store(true, SeqCst);
    assert_eq!(
        frontend.ask(request.clone()).await["Error"],
        "partially applied topology"
    );
    let revision = state["LayoutRevision"].as_u64().unwrap();
    state = frontend
        .state(|s| s["LayoutRevision"].as_u64().unwrap() > revision)
        .await;
    assert_eq!(commands.load(SeqCst), 1);
    fail.store(false, SeqCst);
    request["LayoutRevision"] = state["LayoutRevision"].clone();
    assert_eq!(
        frontend.ask(request.clone()).await["Result"]["DryRun"],
        false
    );
    let revision = state["LayoutRevision"].as_u64().unwrap();
    state = frontend
        .state(|s| s["LayoutRevision"].as_u64().unwrap() > revision)
        .await;
    assert_eq!(commands.load(SeqCst), 2);
    request["LayoutRevision"] = state["LayoutRevision"].clone();
    frontend.ask(json!({"Method":"Start"})).await;
    frontend.state(|s| s["Engine"] == "Running").await;
    for dry in [true, false] {
        request["DryRun"] = json!(dry);
        assert!(frontend.ask(request.clone()).await["Error"]
            .as_str()
            .unwrap()
            .contains("stop the engine"));
    }
    assert_eq!(commands.load(SeqCst), 2);
}

#[tokio::test]
async fn wallpaper_editor_reads_the_agents_store_and_rejects_stale_or_invalid_updates() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs).with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({"Method":"Subscribe"})).await["Result"].clone();
    let id = state["LayoutId"].as_str().unwrap();
    let revision = state["LayoutRevision"].as_u64().unwrap();
    let answer = frontend
        .ask(json!({"Method":"WallpaperSettings","LayoutId":id,"LayoutRevision":revision}))
        .await;
    assert_eq!(
        answer["Result"]["Settings"]["Mode"], "PerScreen",
        "{answer}"
    );
    assert!(answer["Result"]["Supported"].is_boolean());
    assert!(
        !dir.path().join("wallpaper.json").exists(),
        "a read never seeds settings"
    );
    for method in ["WallpaperSettings", "ApplyWallpaper"] {
        let refused = frontend
            .ask(json!({"Method":method,"LayoutId":id,"LayoutRevision":revision+1,"Settings":{"Mode":"PerScreen"}}))
            .await;
        assert!(
            refused["Error"]
                .as_str()
                .unwrap()
                .contains("displays changed"),
            "{refused}"
        );
    }
    let rejected=frontend.ask(json!({"Method":"ApplyWallpaper","LayoutId":id,"LayoutRevision":revision,"Settings":{"Mode":"Span","SpanImagePath":dir.path().join("missing.png")}})).await;
    assert!(rejected["Error"].is_string(), "{rejected}");
    assert!(!dir.path().join("wallpaper.json").exists());
    std::fs::write(dir.path().join("wallpaper.json"), "corrupt").unwrap();
    let rejected = frontend
        .ask(json!({"Method":"WallpaperSettings","LayoutId":id,"LayoutRevision":revision}))
        .await;
    assert!(rejected["Error"].is_string());
}
