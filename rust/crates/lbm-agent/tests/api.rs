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
        } => {
            assert_eq!(layout_id, "TESTMON1");
            assert!(adjust_scale);
            assert!(document.layout.is_some());
        }
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
