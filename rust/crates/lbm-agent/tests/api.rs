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
static SHARING_LOOKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static DESKFLOW_RUNS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Stands in for the process list: an orphaned `deskflow-core server` holding a client,
/// while `DESKFLOW_RUNS` says so; every look counted.
fn scan_sharing() -> Vec<lbm_ipc::sharing::Running> {
    use std::sync::atomic::Ordering;
    SHARING_LOOKS.fetch_add(1, Ordering::SeqCst);
    if DESKFLOW_RUNS.load(Ordering::SeqCst) {
        vec![lbm_ipc::sharing::Running {
            tool: lbm_ipc::sharing::Tool::Deskflow,
            process: "deskflow-core".to_owned(),
            pid: 40,
            role: Some(lbm_ipc::sharing::Role::Server),
            window: false,
            connected: Some(true),
        }]
    } else {
        Vec::new()
    }
}

/// The other sharing tools are looked for when a window subscribes and when the engine
/// starts, and are in the state from then on; nothing looks in between.
#[tokio::test]
async fn another_sharing_tool_is_in_the_state_from_the_moments_it_is_looked_for() {
    use std::sync::atomic::Ordering;
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    tokio::spawn(async move {
        let mut agent = Agent::new(Enabled(None), Timings::default(), hook, inputs)
            .with_api(calls)
            .with_input_sharing_scan(scan_sharing);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut first = Frontend::connect(&api_endpoint).await;
    let state = first.ask(json!({ "Method": "Subscribe" })).await;
    assert_eq!(
        state["Result"]["InputSharing"],
        Value::Null,
        "absent when none"
    );
    first.state(|s| s["Engine"] == "Running").await;
    let looks = SHARING_LOOKS.load(Ordering::SeqCst);
    assert!(looks >= 2, "at the subscription and at the start: {looks}");

    // Deskflow starts: nobody watches, so nothing changes until a moment that looks.
    DESKFLOW_RUNS.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(SHARING_LOOKS.load(Ordering::SeqCst), looks, "nothing polls");

    // A window opening is one: it sees it, and so does the window already open.
    let mut later = Frontend::connect(&api_endpoint).await;
    let state = later.ask(json!({ "Method": "Subscribe" })).await;
    assert_eq!(
        state["Result"]["InputSharing"],
        json!([{ "Tool": "Deskflow", "Process": "deskflow-core", "Pid": 40,
                 "Role": "Server", "Connected": true }])
    );
    first
        .state(|s| s["InputSharing"][0]["Tool"] == "Deskflow")
        .await;

    // Deskflow gone, the engine stopped then started: the start looks again.
    DESKFLOW_RUNS.store(false, Ordering::SeqCst);
    first.ask(json!({ "Method": "Stop" })).await;
    first.state(|s| s["Engine"] == "Stopped").await;
    first.ask(json!({ "Method": "Start" })).await;
    first
        .state(|s| s["Engine"] == "Running" && s["InputSharing"].is_null())
        .await;
}

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

    // The user gave it a key: the windows already open are told, the next ones never hear
    // of the warning.
    fake.broadcast(&Event::ShortcutAvailable {
        shortcut: "Ctrl+Alt+F9".to_string(),
    });
    later.state(|s| s["ShortcutUnavailable"].is_null()).await;
    let mut last = Frontend::connect(&api_endpoint).await;
    let state = last.ask(json!({ "Method": "Subscribe" })).await;
    assert_eq!(state["Result"]["ShortcutUnavailable"], Value::Null);
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
    // Never the session's desktop: Plasma matches screens by position, and this test's
    // fallback output sits at 0,0 like the developer's own primary screen.
    let desktop = std::sync::Arc::new(lbm_agent::desktop::FakeDesktop::default());
    let painted = desktop.clone();
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_api(calls)
            .with_desktop(desktop);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    let layout_id = state["LayoutId"].as_str().unwrap().to_owned();
    let screen = frontend_layout(dir.path()).monitors()[0].id.clone();
    assert!(
        painted.applied().is_empty(),
        "nothing to paint before there are settings"
    );

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

    // And the desktop it was handed is painted: one screen, found by where it is, with
    // the colour saved rendered into this agent's own wallpaper directory — the very
    // request (`editor_<hash>.png` in a tempdir) that once reached a developer's Plasma.
    eventually(
        || !painted.applied().is_empty(),
        "the saved wallpaper never reached the desktop",
    )
    .await;
    let painted = painted.applied();
    assert_eq!(painted.len(), 1, "{painted:?}");
    let [only] = painted[0].as_slice() else {
        panic!("one screen expected: {painted:?}");
    };
    assert_eq!((only.x, only.y), (0.0, 0.0));
    assert!(
        only.image
            .as_ref()
            .is_some_and(|image| image.starts_with(dir.path().join("wallpapers"))),
        "{only:?}"
    );
    let rendered = image::open(only.image.as_ref().unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(rendered.get_pixel(0, 0).0, [0x10, 0x20, 0x30, 0xff]);

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
async fn an_option_carried_by_the_layout_reaches_a_running_hook_only() {
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
    let last_load = |fake: &FakeHook| {
        fake.received().iter().rev().find_map(|c| match c {
            Command::Load { zones, .. } => Some(zones.clone()),
            _ => None,
        })
    };
    let touch =
        |on: bool| json!({ "Method": "SaveOptions", "Options": { "TouchMouseIndependent": on } });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    frontend.ask(json!({ "Method": "Subscribe" })).await;

    // Stopped: stored, and nothing is put in front of the hook.
    assert_eq!(frontend.ask(touch(true)).await["Result"], Value::Null);
    assert_eq!(loads(&fake), 0);

    // The Start carries it.
    frontend.ask(json!({ "Method": "Start" })).await;
    frontend.state(|s| s["Engine"] == "Running").await;
    assert!(last_load(&fake)
        .unwrap()
        .contains(r#"TouchMouseIndependent="True""#));

    // Running: the change is loaded at once, and the hook stays up.
    assert_eq!(frontend.ask(touch(false)).await["Result"], Value::Null);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !last_load(&fake)
            .unwrap()
            .contains(r#"TouchMouseIndependent="False""#)
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the running hook never got the new option");
    assert!(fake.hooked());

    // Saying the same thing again changes nothing on the wire.
    let before = loads(&fake);
    assert_eq!(frontend.ask(touch(false)).await["Result"], Value::Null);
    frontend.ask(json!({ "Method": "Snapshot" })).await;
    assert_eq!(loads(&fake), before);
}

#[tokio::test]
async fn an_option_saved_during_a_preview_reaches_the_previewed_layout() {
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
    let state = frontend.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    let layout_id = state["LayoutId"].as_str().unwrap().to_owned();
    let mut edited = frontend_layout(dir.path());
    let monitor = edited.monitors()[0].id.clone();
    edited.set_location(&monitor, lbm_layout::geo::Point::new(40.0, 20.0));
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&edited)).unwrap();
    let preview = json!({ "Method": "Preview", "LayoutId": layout_id, "Document": document });
    assert_eq!(frontend.ask(preview).await["Result"], Value::Null);
    frontend
        .state(|s| s["Previewing"] == true && s["Engine"] == "Running")
        .await;
    let geometry = |zones: &str| zones.split_once("<MainZones>").unwrap().1.to_owned();
    let last_load = |fake: &FakeHook| {
        fake.received().iter().rev().find_map(|c| match c {
            Command::Load { zones, .. } => Some(zones.clone()),
            _ => None,
        })
    };
    let previewed = geometry(&last_load(&fake).unwrap());

    let saved = json!({ "Method": "SaveOptions", "Options": { "TouchMouseIndependent": true } });
    assert_eq!(frontend.ask(saved).await["Result"], Value::Null);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !last_load(&fake)
            .unwrap()
            .contains(r#"TouchMouseIndependent="True""#)
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the previewed layout never got the option");
    assert_eq!(
        geometry(&last_load(&fake).unwrap()),
        previewed,
        "the geometry being edited was lost"
    );
}

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
    // An agent given no desktop has none: never the session's, whatever runs the test.
    assert_eq!(answer["Result"]["Supported"], false, "{answer}");
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

#[tokio::test]
async fn vcp_checks_the_current_layout_setting_and_monitor_before_hardware_access() {
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
    let state = frontend.ask(json!({"Method":"Snapshot"})).await["Result"].clone();
    let id = state["LayoutId"].as_str().unwrap();
    let revision = state["LayoutRevision"].as_u64().unwrap();
    for write in [Value::Null, json!({"Token":1,"Code":16,"Value":40})] {
        let mut request = json!({"Method":"Vcp", "LayoutId":id,"LayoutRevision":revision+1,"Monitor":"missing","Write":write});
        let answer = frontend.ask(request.clone()).await;
        assert!(
            answer["Error"]
                .as_str()
                .unwrap()
                .contains("displays changed"),
            "{answer}"
        );
        request["LayoutRevision"] = json!(revision);
        let answer = frontend.ask(request.clone()).await;
        assert!(
            answer["Error"].as_str().unwrap().contains("Enable VCP"),
            "{answer}"
        );
    }
    let answer = frontend
        .ask(json!({"Method":"SaveOptions","Options":{"VcpControl":true}}))
        .await;
    assert!(answer.get("Error").is_none(), "{answer}");
    let answer = frontend
        .ask(json!({"Method":"Vcp", "LayoutId":id,"LayoutRevision":revision,"Monitor":"missing"}))
        .await;
    assert!(
        answer["Error"]
            .as_str()
            .unwrap()
            .contains("no longer connected"),
        "{answer}"
    );
}

#[tokio::test]
async fn network_remotes_load_associations_without_a_ddc_bus_and_reject_stale_commands() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let world = system_world(dir.path());
    let config_dir = dir.path().to_owned();
    let local = frontend_layout(dir.path());
    let monitor = local.monitors()[0].id.clone();
    std::fs::write(dir.path().join("samsung-tizen.json"), json!({monitor.clone():{"IpAddress":"192.168.1.20","Token":"private-token","Name":"Samsung"}}).to_string()).unwrap();
    tokio::spawn(async move {
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_remote_config(config_dir)
            .with_api(calls);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({"Method":"Snapshot"})).await["Result"].clone();
    let id = state["LayoutId"].as_str().unwrap();
    let revision = state["LayoutRevision"].as_u64().unwrap();
    let request = json!({"Method":"Remote","LayoutId":id,"LayoutRevision":revision,"Monitor":monitor,"Command":{"Brand":"Samsung","Action":"Info"}});
    let answer = frontend.ask(request.clone()).await;
    assert!(answer["Error"].as_str().unwrap().contains("Enable VCP"));
    frontend
        .ask(json!({"Method":"SaveOptions","Options":{"VcpControl":true}}))
        .await;
    let answer = frontend.ask(request.clone()).await;
    assert_eq!(answer["Result"]["Samsung"]["Paired"], true, "{answer}");
    assert_eq!(answer["Result"]["Samsung"]["IpAddress"], "192.168.1.20");
    assert!(!answer.to_string().contains("private-token"));
    let mut stale = request;
    stale["LayoutRevision"] = json!(revision + 1);
    stale["Command"] = json!({"Brand":"Samsung","Action":"Key","Key":"KEY_HOME"});
    let answer = frontend.ask(stale).await;
    assert!(answer["Error"].as_str().unwrap().contains("layout changed"));
    assert!(
        !dir.path().join("network-remotes.json").exists(),
        "reading does not rewrite associations"
    );
}

/// The next `Wallpapers` a master receives.
async fn next_wallpapers(
    link: &mut lbm_net::link::Link,
) -> (u64, Vec<lbm_ipc::peer::PeerWallpaper>) {
    use lbm_net::link::Event;
    use lbm_net::wire::Control;
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), link.next())
            .await
            .expect("an event within 5 s");
        match event {
            Some(Event::Control(Control::Wallpapers {
                revision,
                wallpapers,
            })) => return (revision, wallpapers),
            Some(Event::Closed(reason)) => panic!("the link ended: {reason:?}"),
            None => panic!("the link ended"),
            Some(_) => {}
        }
    }
}

/// What a desktop shows on the one screen of [`Screens`]: a solid colour. As wide as that
/// screen after its second and third rebuild, within the pixel of slack the matching
/// allows (it grows by one at every rebuild).
fn solid(color: [u8; 3]) -> lbm_agent::desktop::Shown {
    lbm_agent::desktop::Shown {
        bounds: lbm_layout::geo::Rect::new(0., 0., 1923., 1080.),
        path: None,
        style: lbm_wallpaper::Style::Fill,
        repeat: lbm_wallpaper::desktop::Repeat::Both,
        color: [color[0], color[1], color[2], 255],
    }
}

fn colour_of(display: &str, color: &str) -> lbm_ipc::peer::PeerWallpaper {
    lbm_ipc::peer::PeerWallpaper {
        display: display.into(),
        image: None,
        color: Some(color.into()),
    }
}

#[tokio::test]
async fn a_remote_sends_its_wallpapers_at_link_up_and_after_a_display_change_and_only_then() {
    use lbm_net::wire::{Hello, Role};

    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let config = dir.path().join("config");
    let master = lbm_net::identity::Identity::load_or_create(&dir.path().join("master")).unwrap();
    lbm_net::peers::PeerStore::load(&config)
        .insert(lbm_net::peers::PairedPeer {
            machine_id: master.fingerprint().to_owned(),
            name: "desk".into(),
            address: String::new(),
            role: Role::Master,
        })
        .unwrap();
    // The remote's desktop, never the developer's: what it shows is the test's.
    let desktop = std::sync::Arc::new(lbm_agent::desktop::FakeDesktop::default());
    desktop.show(vec![solid([0x10, 0x20, 0x30])]);
    let shown = desktop.clone();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let peers = lbm_agent::peers::Service::new(
        config.clone(),
        lbm_agent::peers::Setup {
            bind: "127.0.0.1".parse().unwrap(),
            input_endpoint: None,
            options: lbm_net::link::Options::default(),
            clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
        },
    );
    tokio::spawn(async move {
        let world = Screens {
            state: Enabled(None),
            rebuilds: 0,
        };
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_api(calls)
            .with_peers(peers)
            .with_desktop(desktop);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    let on = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": true, "Port": 0, "Name": "tower" }))
        .await["Result"]
        .clone();
    let address = on["ListenAddress"].as_str().unwrap().to_owned();
    // Rebuilds with no master linked read nothing: nobody would see it.
    frontend.ask(json!({ "Method": "Refresh" })).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(shown.asked(), 0, "a machine nobody watches reads nothing");

    let mut link = lbm_net::link::connect(
        address.as_str(),
        &master,
        Hello::new("desk", "0", Role::Master),
        lbm_net::link::Options::default(),
    )
    .await
    .unwrap()
    .accept()
    .await
    .unwrap();
    let (first, wallpapers) = next_wallpapers(&mut link).await;
    assert_eq!(wallpapers, [colour_of("DELA0B1_ABC123", "#102030")]);

    // The wallpaper changes and the desktop says nothing of it (one that cannot): nothing
    // is read — there is no timer — until the next display event, which brings the new one.
    shown.show(vec![solid([0x40, 0x50, 0x60])]);
    let asked = shown.asked();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(shown.asked(), asked, "never polled");
    frontend.ask(json!({ "Method": "Refresh" })).await;
    let (next, wallpapers) = next_wallpapers(&mut link).await;
    assert!(next > first, "{next} after {first}");
    assert_eq!(wallpapers, [colour_of("DELA0B1_ABC123", "#405060")]);
}

/// Whether `link` stays without a `Wallpapers` for `within`.
async fn no_wallpapers(link: &mut lbm_net::link::Link, within: Duration) -> bool {
    use lbm_net::link::Event;
    use lbm_net::wire::Control;
    let end = tokio::time::Instant::now() + within;
    loop {
        match tokio::time::timeout_at(end, link.next()).await {
            Err(_) => return true,
            Ok(Some(Event::Control(Control::Wallpapers { .. }))) => return false,
            Ok(Some(Event::Closed(reason))) => panic!("the link ended: {reason:?}"),
            Ok(None) => panic!("the link ended"),
            Ok(Some(_)) => {}
        }
    }
}

/// Waits for `desktop` to be watched `count` times over.
async fn watched(desktop: &lbm_agent::desktop::FakeDesktop, count: usize) {
    for _ in 0..500 {
        if desktop.watching() == count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "{} watch(es) on the desktop, {count} expected",
        desktop.watching()
    );
}

#[tokio::test]
async fn a_wallpaper_changed_in_the_system_is_sent_once_per_burst_and_only_a_linked_master_has_the_desktop_watched(
) {
    use lbm_agent::wallpaper_watch::{LONGEST, QUIET};
    use lbm_net::wire::{Hello, Role};

    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let config = dir.path().join("config");
    let master = lbm_net::identity::Identity::load_or_create(&dir.path().join("master")).unwrap();
    lbm_net::peers::PeerStore::load(&config)
        .insert(lbm_net::peers::PairedPeer {
            machine_id: master.fingerprint().to_owned(),
            name: "desk".into(),
            address: String::new(),
            role: Role::Master,
        })
        .unwrap();
    // The remote's desktop, never the developer's: what it shows and says is the test's.
    let desktop = std::sync::Arc::new(lbm_agent::desktop::FakeDesktop::default());
    desktop.show(vec![solid([0x10, 0x20, 0x30])]);
    let shown = desktop.clone();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let peers = lbm_agent::peers::Service::new(
        config.clone(),
        lbm_agent::peers::Setup {
            bind: "127.0.0.1".parse().unwrap(),
            input_endpoint: None,
            options: lbm_net::link::Options::default(),
            clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
        },
    );
    tokio::spawn(async move {
        let world = Screens {
            state: Enabled(None),
            rebuilds: 0,
        };
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_api(calls)
            .with_peers(peers)
            .with_desktop(desktop);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    assert_eq!(shown.watching(), 0, "the mode is off: nothing is watched");
    let on = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": true, "Port": 0, "Name": "tower" }))
        .await["Result"]
        .clone();
    let address = on["ListenAddress"].as_str().unwrap().to_owned();
    frontend.ask(json!({ "Method": "Refresh" })).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        shown.watching(),
        0,
        "the mode is on but no master is linked: nothing is watched"
    );

    let link_up = || async {
        lbm_net::link::connect(
            address.as_str(),
            &master,
            Hello::new("desk", "0", Role::Master),
            lbm_net::link::Options::default(),
        )
        .await
        .unwrap()
        .accept()
        .await
        .unwrap()
    };
    let mut link = link_up().await;
    let (first, wallpapers) = next_wallpapers(&mut link).await;
    assert_eq!(wallpapers, [colour_of("DELA0B1_ABC123", "#102030")]);
    watched(&shown, 1).await;

    // The user changes the wallpaper, and the desktop says so the way desktops do: more
    // than once. One reading, one message.
    shown.show(vec![solid([0x40, 0x50, 0x60])]);
    let asked = shown.asked();
    for _ in 0..5 {
        shown.change();
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(shown.asked(), asked, "read before the burst went quiet");
    let (next, wallpapers) = next_wallpapers(&mut link).await;
    assert!(next > first, "{next} after {first}");
    assert_eq!(wallpapers, [colour_of("DELA0B1_ABC123", "#405060")]);
    assert!(
        no_wallpapers(&mut link, LONGEST).await,
        "one burst, two messages"
    );
    assert_eq!(shown.asked(), asked + 1, "one burst, one reading");

    // An event that changed nothing the master would see (the same picture set again,
    // another setting of the same file): read, since only the reading can tell, and not
    // sent — the master's revision does not move.
    shown.change();
    assert!(
        no_wallpapers(&mut link, QUIET * 3).await,
        "the same wallpapers were sent again"
    );
    assert_eq!(shown.asked(), asked + 2);

    // The master leaves: the watch goes with it, and what the desktop says is nobody's.
    drop(link);
    watched(&shown, 0).await;
    let asked = shown.asked();
    shown.change();
    tokio::time::sleep(QUIET * 2).await;
    assert_eq!(shown.asked(), asked, "read with no master to send to");

    // It comes back: watched again, once. And turning the mode off lets go of it.
    let mut link = link_up().await;
    watched(&shown, 1).await;
    shown.show(vec![solid([0x70, 0x80, 0x90])]);
    shown.change();
    loop {
        // What was last read comes first at link-up; the change follows.
        let (_, wallpapers) = next_wallpapers(&mut link).await;
        if wallpapers == [colour_of("DELA0B1_ABC123", "#708090")] {
            break;
        }
    }
    frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": false, "Port": 0, "Name": "tower" }))
        .await;
    watched(&shown, 0).await;
}

/// The next `Overlay` the laptop is sent, skipping everything else.
async fn next_overlay(
    link: &mut lbm_net::link::Link,
    within: Duration,
) -> Option<(u64, Vec<Value>)> {
    tokio::time::timeout(within, async {
        loop {
            match link.next().await {
                Some(lbm_net::link::Event::Control(lbm_net::wire::Control::Overlay {
                    revision,
                    bands,
                })) => return (revision, bands),
                Some(_) => {}
                None => panic!("the link ended"),
            }
        }
    })
    .await
    .ok()
}

#[tokio::test]
async fn the_rulers_a_frontend_shows_on_a_remote_go_down_when_the_frontend_goes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start_as(
        &tmp,
        lbm_agent::discovery::Discovery::Fallback,
        None,
        &[lbm_net::wire::OVERLAY_CAPABILITY],
    )
    .await;
    let laptop_id = master.laptop_id();
    let hello = master.frontend.ask(json!({ "Method": "Hello" })).await;
    assert!(hello["Result"]["Capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("PeerOverlay")));
    let mut link = master.link_the_laptop().await;
    let band = json!({"monitor": "eDP-1@laptop", "connector": "eDP-1"});
    let asked = master
        .frontend
        .ask(json!({ "Method": "PeerOverlay", "MachineId": laptop_id, "Bands": [band] }))
        .await;
    assert_eq!(asked["Result"], Value::Null, "{asked}");
    let (shown, bands) = next_overlay(&mut link, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(bands, [band], "passed on unread: the remote checks them");

    // The window closes without taking them down: the agent does.
    drop(master.frontend);
    let (down, bands) = next_overlay(&mut link, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(down > shown && bands.is_empty(), "{down} {bands:?}");
}

#[tokio::test]
async fn a_window_taking_its_rulers_down_leaves_those_another_window_wrote_since() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start_as(
        &tmp,
        lbm_agent::discovery::Discovery::Fallback,
        None,
        &[lbm_net::wire::OVERLAY_CAPABILITY],
    )
    .await;
    let laptop_id = master.laptop_id();
    let mut link = master.link_the_laptop().await;
    let mut other = Frontend::connect(&master.api_endpoint).await;
    let rulers = |by: &str| json!({ "Method": "PeerOverlay", "MachineId": laptop_id, "Bands": [{"monitor": by}] });
    let none = json!({ "Method": "PeerOverlay", "MachineId": laptop_id, "Bands": [] });

    // A shows its rulers, then B shows its own on the same remote.
    master.frontend.ask(rulers("a")).await;
    next_overlay(&mut link, Duration::from_secs(5))
        .await
        .unwrap();
    other.ask(rulers("b")).await;
    let (_, bands) = next_overlay(&mut link, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(bands, [json!({"monitor": "b"})]);
    // A turns its rulers off, then goes: B's stay.
    let answer = master.frontend.ask(none.clone()).await;
    assert_eq!(answer["Result"], Value::Null, "{answer}");
    drop(master.frontend);
    assert_eq!(
        next_overlay(&mut link, Duration::from_millis(500)).await,
        None,
        "B's rulers are B's to take down"
    );
    // B turns them off: down.
    other.ask(none).await;
    let (_, bands) = next_overlay(&mut link, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(bands.is_empty());
}

#[tokio::test]
async fn rulers_for_a_remote_too_old_to_draw_them_are_not_sent_and_the_user_is_told() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Fallback).await;
    let laptop_id = master.laptop_id();
    let mut link = master.link_the_laptop().await;
    let asked = master
        .frontend
        .ask(
            json!({ "Method": "PeerOverlay", "MachineId": laptop_id, "Bands": [{"monitor": "x"}] }),
        )
        .await;
    assert_eq!(asked["Result"], Value::Null, "{asked}");
    let told = loop {
        let state = master.frontend.ask(json!({ "Method": "Peers" })).await;
        if let Some(error) = state["Result"]["Peers"][0]["OverlayError"].as_str() {
            break error.to_owned();
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert!(told.contains("cannot show rulers"), "{told}");
    assert_eq!(
        next_overlay(&mut link, Duration::from_millis(300)).await,
        None
    );
    // Taking them down clears what the user was told.
    master
        .frontend
        .ask(json!({ "Method": "PeerOverlay", "MachineId": laptop_id, "Bands": [] }))
        .await;
    let state = master.frontend.ask(json!({ "Method": "Peers" })).await;
    assert!(
        state["Result"]["Peers"][0]["OverlayError"].is_null(),
        "{state}"
    );
}

#[tokio::test]
async fn a_master_keeps_its_remotes_wallpapers_and_serves_them_apart_from_the_peers_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Fallback).await;
    let laptop_id = master.laptop_id();
    let hello = master.frontend.ask(json!({ "Method": "Hello" })).await;
    assert!(hello["Result"]["Capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("PeerWallpapers")));
    let link = master.link_the_laptop().await;
    let none = master
        .frontend
        .ask(json!({ "Method": "PeerWallpapers", "MachineId": laptop_id }))
        .await;
    assert_eq!(
        none["Result"],
        json!({ "MachineId": laptop_id, "Revision": 0, "Wallpapers": [] }),
        "nothing sent yet is no error"
    );

    let picture = lbm_ipc::peer::PeerWallpaper {
        display: "eDP-1".into(),
        image: Some("/9j/4AAQSkZJRg==".into()),
        color: None,
    };
    let send = |revision: u64, wallpapers: Vec<lbm_ipc::peer::PeerWallpaper>| {
        let sender = link.sender();
        async move {
            sender
                .control(&lbm_net::wire::Control::Wallpapers {
                    revision,
                    wallpapers,
                })
                .await
                .unwrap();
        }
    };
    send(1, vec![picture.clone()]).await;
    // The state says there is something new, and only that: the pictures are asked for.
    let stamp = loop {
        let frame = master.frontend.receive().await;
        if frame["Event"] != "Peers" {
            continue;
        }
        let entry = &frame["Peers"]["Peers"][0];
        let stamp = entry["WallpaperRevision"].as_u64().unwrap_or(0);
        if stamp > 0 {
            assert!(
                !frame.to_string().contains("4AAQ"),
                "no picture in the event: {frame}"
            );
            break stamp;
        }
    };
    let served = master
        .frontend
        .ask(json!({ "Method": "PeerWallpapers", "MachineId": laptop_id }))
        .await;
    assert_eq!(served["Result"]["Revision"], stamp);
    assert_eq!(
        served["Result"]["Wallpapers"],
        json!([{ "Display": "eDP-1", "Image": "/9j/4AAQSkZJRg==" }])
    );

    // The same pictures again change nothing; an older revision is ignored; other ones
    // are a new stamp.
    send(2, vec![picture.clone()]).await;
    send(1, vec![colour_of("eDP-1", "#000000")]).await;
    send(3, vec![colour_of("eDP-1", "#203040")]).await;
    let newer = loop {
        let state = master.frontend.ask(json!({ "Method": "Peers" })).await;
        let now = state["Result"]["Peers"][0]["WallpaperRevision"]
            .as_u64()
            .unwrap();
        if now != stamp {
            break now;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(newer, stamp + 1, "one new stamp, for the one change");
    let served = master
        .frontend
        .ask(json!({ "Method": "PeerWallpapers", "MachineId": laptop_id }))
        .await;
    assert_eq!(
        served["Result"]["Wallpapers"],
        json!([{ "Display": "eDP-1", "Color": "#203040" }])
    );

    let unknown = master
        .frontend
        .ask(json!({ "Method": "PeerWallpapers", "MachineId": "nobody" }))
        .await;
    assert!(unknown["Error"].is_string(), "{unknown}");
}

/// One monitor whose screens, as the multi-PC mode reports them, change with every
/// rebuild: what a display change looks like from the remote's side.
struct Screens {
    state: Enabled,
    rebuilds: i32,
}

impl World for Screens {
    fn display_signature(&mut self) -> String {
        "one-monitor".into()
    }
    fn rebuild_layout(&mut self) {
        self.rebuilds += 1;
        self.state.rebuild_layout();
    }
    fn layout(&self) -> Option<LayoutState> {
        self.state.layout()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.state.set_enabled(enabled);
    }
}

impl AgentWorld for Screens {
    fn zones(&self) -> Option<(String, bool)> {
        self.state.zones()
    }
    fn desktop(&self) -> Option<lbm_ipc::protocol::Desktop> {
        self.state.desktop()
    }
    fn save_enabled(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn save_layout(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn layout_id(&self) -> Option<String> {
        self.state.layout_id()
    }
    fn peer_displays(&mut self) -> Vec<lbm_ipc::peer::PeerDisplay> {
        vec![lbm_ipc::peer::PeerDisplay {
            id: "DELA0B1_ABC123".into(),
            connector: "DP-1".into(),
            name: "DELL".into(),
            edid: None,
            x: 0,
            y: 0,
            width: 1920 + self.rebuilds,
            height: 1080,
            scale: 1.0,
            effective_dpi: 96.0,
            orientation: 0,
            frequency: 60.0,
            primary: true,
            attached: true,
            width_mm: 527.0,
            height_mm: 296.0,
            input_source: None,
        }]
    }
}

/// The widths of the screens in the next `Displays` a master receives.
async fn displayed_widths(link: &mut lbm_net::link::Link) -> Vec<i32> {
    use lbm_net::link::Event;
    use lbm_net::wire::Control;
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), link.next())
            .await
            .expect("an event within 5 s");
        match event {
            Some(Event::Control(Control::Displays { displays, .. })) => {
                return displays.iter().map(|d| d.width).collect()
            }
            Some(Event::Closed(reason)) => panic!("the link ended: {reason:?}"),
            None => panic!("the link ended"),
            Some(_) => {}
        }
    }
}

#[tokio::test]
async fn a_frontend_drives_the_multi_pc_mode_and_a_master_sees_the_screens_follow_a_rebuild() {
    use lbm_net::wire::{Hello, Role};

    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let config = dir.path().join("config");
    // A master the user paired earlier.
    let master = lbm_net::identity::Identity::load_or_create(&dir.path().join("master")).unwrap();
    lbm_net::peers::PeerStore::load(&config)
        .insert(lbm_net::peers::PairedPeer {
            machine_id: master.fingerprint().to_owned(),
            name: "desk".into(),
            address: String::new(),
            role: Role::Master,
        })
        .unwrap();

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let peers = lbm_agent::peers::Service::new(
        config.clone(),
        lbm_agent::peers::Setup {
            bind: "127.0.0.1".parse().unwrap(),
            input_endpoint: None,
            options: lbm_net::link::Options::default(),
            clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
        },
    );
    tokio::spawn(async move {
        let world = Screens {
            state: Enabled(None),
            rebuilds: 0,
        };
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_api(calls)
            .with_peers(peers);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });

    let mut frontend = Frontend::connect(&api_endpoint).await;
    let hello = frontend.ask(json!({ "Method": "Hello" })).await;
    assert_eq!(hello["Result"]["Protocol"], 3, "additive: no new version");
    assert!(hello["Result"]["Capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("MultiPc")));
    frontend.ask(json!({ "Method": "Subscribe" })).await;

    // Off by default: no listener, no identity, the paired master listed.
    let off = frontend.ask(json!({ "Method": "Peers" })).await["Result"].clone();
    assert_eq!(off["Enabled"], false);
    assert_eq!(off["ListenAddress"], Value::Null);
    assert_eq!(off["MachineId"], Value::Null);
    assert_eq!(off["Peers"][0]["MachineId"], master.fingerprint());
    assert_eq!(off["Peers"][0]["State"], "Paired");
    assert_eq!(off["Peers"][0]["Role"], "Master");
    assert_eq!(off["Peers"][0]["Connected"], false);
    let refused = frontend
        .ask(json!({ "Method": "PeerConnect", "Address": "127.0.0.1:1" }))
        .await;
    assert!(refused["Error"].as_str().unwrap().contains("off"));

    let on = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": true, "Port": 0, "Name": "tower" }))
        .await["Result"]
        .clone();
    assert_eq!(on["Enabled"], true);
    assert_eq!(on["Name"], "tower");
    assert_eq!(on["MachineId"].as_str().unwrap().len(), 64);
    let address = on["ListenAddress"].as_str().unwrap().to_owned();
    assert!(config.join("multi-pc.json").exists());

    // The master connects: accepted from the store, told the screens, seen linked.
    let mut link = lbm_net::link::connect(
        address.as_str(),
        &master,
        Hello::new("desk", "0", Role::Master),
        lbm_net::link::Options::default(),
    )
    .await
    .unwrap()
    .accept()
    .await
    .unwrap();
    // Built once at boot, so the screens are the first rebuild's.
    while displayed_widths(&mut link).await != [1921] {}
    loop {
        let frame = frontend.receive().await;
        if frame["Event"] == "Peers" && frame["Peers"]["Peers"][0]["Connected"] == true {
            assert_eq!(frame["Peers"]["Peers"][0]["Role"], "Master");
            break;
        }
    }

    // A rebuild is a display change: the master hears of it.
    frontend.ask(json!({ "Method": "Refresh" })).await;
    assert_eq!(displayed_widths(&mut link).await, [1922]);

    // Forgetting the master ends its link.
    let forgotten = frontend
        .ask(json!({ "Method": "PeerForget", "MachineId": master.fingerprint() }))
        .await;
    assert_eq!(forgotten["Result"], Value::Null, "{forgotten}");
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match link.next().await {
                Some(lbm_net::link::Event::Closed(reason)) => return reason,
                None => panic!("closed without a reason"),
                Some(_) => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(
        matches!(ended, lbm_net::link::CloseReason::Bye(_)),
        "{ended:?}"
    );
    let peers = frontend.ask(json!({ "Method": "Peers" })).await;
    assert_eq!(peers["Result"]["Peers"], json!([]));
    let unknown = frontend
        .ask(json!({ "Method": "PeerAccept", "MachineId": "nobody" }))
        .await;
    assert!(unknown["Error"].is_string(), "{unknown}");

    let off = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": false }))
        .await["Result"]
        .clone();
    assert_eq!(off["Enabled"], false);
    assert_eq!(off["ListenAddress"], Value::Null);
    assert_eq!(off["Name"], "tower", "kept for the next time");
}

//==================//
// A master's layout //
//==================//

/// A 128-byte EDID block of a Dell U2720Q (DEL 4065) with `serial_string` as its serial
/// descriptor. `interface` is the low nibble of byte 20 (5: DisplayPort, 2: HDMI), which
/// differs between two inputs of one monitor, as the checksum does; the panel key does
/// not.
fn dell_edid(serial_string: &str, interface: u8) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
    // "DEL": D=4, E=5, L=12 in five-bit letters.
    let letters = (4u16 << 10) | (5 << 5) | 12;
    b[8] = (letters >> 8) as u8;
    b[9] = letters as u8;
    b[10] = 0x65;
    b[11] = 0x40;
    b[12..16].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
    b[16] = 12;
    b[17] = 30;
    b[18] = 1;
    b[19] = 4;
    b[20] = 0xA0 | interface;
    b[21] = 60;
    b[22] = 34;
    // First detailed timing: 597 x 336 mm.
    b[54] = 0x01;
    b[66] = (597 & 0xFF) as u8;
    b[67] = (336 & 0xFF) as u8;
    b[68] = (((597 >> 8) << 4) | (336 >> 8)) as u8;
    let descriptor = |b: &mut Vec<u8>, at: usize, tag: u8, text: &str| {
        b[at..at + 5].copy_from_slice(&[0, 0, 0, tag, 0]);
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0x0A);
        bytes.resize(13, 0x20);
        b[at + 5..at + 18].copy_from_slice(&bytes);
    };
    descriptor(&mut b, 72, 0xFF, serial_string);
    descriptor(&mut b, 90, 0xFC, "DELL U2720Q");
    let sum: u32 = b[..127].iter().map(|&x| u32::from(x)).sum();
    b[127] = ((256 - sum % 256) % 256) as u8;
    b
}

/// The panel key both machines compute for that Dell.
const SHARED_DELL: &str = "DEL4065_ABC123";

/// This machine with the Dell on DisplayPort, alone: the outputs a test discovery gives.
fn dell_here() -> &'static [lbm_layout::linux::LinuxMonitor] {
    let edid = lbm_display::edid::parse("card0-DP-1", &dell_edid("ABC123", 5)).to_linux();
    Box::leak(Box::new([lbm_layout::linux::LinuxMonitor {
        connector_name: "DP-1".to_owned(),
        logical_x: 0.0,
        logical_y: 0.0,
        logical_width: 2560.0,
        logical_height: 1440.0,
        pixel_width: 2560,
        pixel_height: 1440,
        scale: 1.0,
        width_mm: 597.0,
        height_mm: 336.0,
        primary: true,
        enabled: true,
        orientation: 0,
        frequency: 60,
        edid: Some(edid),
    }]))
}

/// One screen of the laptop, as it reports it.
fn laptop_screen(id: &str, edid: Option<Vec<u8>>, x: i32) -> lbm_ipc::peer::PeerDisplay {
    use base64::Engine as _;
    lbm_ipc::peer::PeerDisplay {
        id: id.to_owned(),
        connector: id.to_owned(),
        name: id.to_owned(),
        edid: edid.map(|e| base64::engine::general_purpose::STANDARD.encode(e)),
        x,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.0,
        effective_dpi: 96.0,
        orientation: 0,
        frequency: 60.0,
        primary: x == 0,
        attached: true,
        width_mm: 344.0,
        height_mm: 194.0,
        input_source: None,
    }
}

/// What a frontend does to hold the layout a master holds: this machine's displays,
/// then each linked remote's merged in machine-id order, the id of the whole set, and
/// only then the store — the order the agent's rebuild follows.
fn master_layout(
    dir: &std::path::Path,
    outputs: &[lbm_layout::linux::LinuxMonitor],
    remotes: &[(&str, &str, Vec<lbm_ipc::peer::PeerDisplay>)],
) -> lbm_layout::model::Layout {
    let mut layout = lbm_layout::model::Layout::new(lbm_layout::model::LayoutOptions::default());
    let mut persistence = persistence(dir);
    lbm_layout::linux::populate(&mut layout, outputs, |l| {
        for (machine_id, name, displays) in remotes {
            lbm_display::peer::merge_peer(l, machine_id, name, displays);
        }
        l.id = l.compute_id();
        persistence.load(l)
    })
    .unwrap();
    layout
}

/// The last `Load` the hook was handed: its zones and its desktop.
fn last_load(fake: &FakeHook) -> (String, Option<lbm_ipc::protocol::Desktop>) {
    fake.received()
        .iter()
        .rev()
        .find_map(|c| match c {
            Command::Load { zones, desktop, .. } => Some((zones.clone(), *desktop)),
            _ => None,
        })
        .expect("a Load")
}

/// A master agent over `discovery` and a store in `dir`, its hook the fake one, and a
/// laptop the user paired earlier as its remote, listening on loopback with nothing of
/// an agent: what it says on its link is the test's.
struct Master {
    frontend: Frontend,
    /// Where another frontend connects.
    api_endpoint: String,
    fake: FakeHook,
    laptop: lbm_net::identity::Identity,
    listener: lbm_net::link::Listener,
    /// The layout of this machine alone, running.
    alone: Value,
    /// The monitors' DDC/CI as the agent sees it: no real monitor is ever reached.
    ddc: std::sync::Arc<lbm_agent::vcp::FakeDdc>,
}

impl Master {
    async fn start(tmp: &tempfile::TempDir, discovery: lbm_agent::discovery::Discovery) -> Master {
        Master::start_with(tmp, discovery, None).await
    }

    /// As [`Master::start`], the hook's input endpoint at `input_endpoint`.
    async fn start_with(
        tmp: &tempfile::TempDir,
        discovery: lbm_agent::discovery::Discovery,
        input_endpoint: Option<String>,
    ) -> Master {
        Master::start_as(tmp, discovery, input_endpoint, &[]).await
    }

    /// As [`Master::start_with`], the laptop announcing `capabilities`.
    async fn start_as(
        tmp: &tempfile::TempDir,
        discovery: lbm_agent::discovery::Discovery,
        input_endpoint: Option<String>,
        capabilities: &[&str],
    ) -> Master {
        use lbm_net::wire::{Hello, Role};

        let dir = tmp.path();
        let (hook_endpoint, api_endpoint) = endpoints(tmp);
        let fake = FakeHook::bind(&hook_endpoint).unwrap();
        let config = dir.join("config");
        let laptop = lbm_net::identity::Identity::load_or_create(&dir.join("laptop")).unwrap();
        let listener = lbm_net::link::listen(
            "127.0.0.1:0",
            &laptop,
            Hello {
                capabilities: capabilities.iter().map(|c| c.to_string()).collect(),
                ..Hello::new("laptop", "0", Role::Remote)
            },
            lbm_net::link::Options::default(),
        )
        .await
        .unwrap();
        lbm_net::peers::PeerStore::load(&config)
            .insert(lbm_net::peers::PairedPeer {
                machine_id: laptop.fingerprint().to_owned(),
                name: "laptop".into(),
                address: listener.local_addr().to_string(),
                role: Role::Remote,
            })
            .unwrap();

        let (hook, signals) = HookClient::spawn(hook_endpoint);
        let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
        let (calls, listening) = listen(&api_endpoint);
        let peers = lbm_agent::peers::Service::new(
            config,
            lbm_agent::peers::Setup {
                bind: "127.0.0.1".parse().unwrap(),
                input_endpoint,
                options: lbm_net::link::Options::default(),
                clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
            },
        );
        let world = lbm_agent::world::SystemWorld::new(discovery, persistence(dir));
        let ddc = std::sync::Arc::new(lbm_agent::vcp::FakeDdc::default());
        let bus = ddc.clone();
        tokio::spawn(async move {
            let _listening = listening;
            let mut agent = Agent::new(world, Timings::default(), hook, inputs)
                .with_api(calls)
                .with_peers(peers)
                .with_ddc(bus);
            agent.run(signals, inputs_rx, std::future::pending()).await;
        });

        let mut frontend = Frontend::connect(&api_endpoint).await;
        frontend.ask(json!({ "Method": "Subscribe" })).await;
        frontend.ask(json!({ "Method": "Start" })).await;
        let alone = frontend.state(|s| s["Engine"] == "Running").await;
        Master {
            frontend,
            api_endpoint,
            fake,
            laptop,
            listener,
            alone,
            ddc,
        }
    }

    fn laptop_id(&self) -> String {
        self.laptop.fingerprint().to_owned()
    }

    /// Turns the mode on and waits for the agent to call the laptop back: the link,
    /// from the laptop's side.
    async fn link_the_laptop(&mut self) -> lbm_net::link::Link {
        let on = self
            .frontend
            .ask(json!({ "Method": "SetMultiPc", "Enabled": true, "Port": 0 }))
            .await;
        assert_eq!(on["Result"]["Enabled"], true, "{on}");
        let pending = tokio::time::timeout(Duration::from_secs(15), self.listener.next())
            .await
            .expect("the agent calls its paired remote back")
            .unwrap()
            .unwrap();
        pending.accept().await.unwrap()
    }

    /// The laptop reports `screens`; once the agent rebuilt on them (another layout id
    /// than `before`, or a new revision of it), the user starts the engine on that
    /// layout — a new set of monitors is a new profile, off until enabled.
    async fn laptop_shows(
        &mut self,
        link: &lbm_net::link::Link,
        revision: u64,
        screens: Vec<lbm_ipc::peer::PeerDisplay>,
    ) -> Value {
        let before = self.alone["LayoutRevision"].as_u64().unwrap();
        link.sender()
            .control(&lbm_net::wire::Control::Displays {
                revision,
                displays: screens,
            })
            .await
            .unwrap();
        let rebuilt = self
            .frontend
            .state(|s| s["LayoutRevision"].as_u64().unwrap() > before)
            .await;
        // The engine state is the hook's word, which lags: what says the user's Start
        // went through is the Load it sends.
        let handed = loads(&self.fake);
        self.frontend.ask(json!({ "Method": "Start" })).await;
        load_after(&self.fake, handed).await;
        // Stopped on the new layout (off until enabled), then running it.
        self.frontend.state(|s| s["Engine"] == "Running").await;
        rebuilt
    }
}

/// The `Load` after the first `before`, once the hook has it.
async fn load_after(
    fake: &FakeHook,
    before: usize,
) -> (String, Option<lbm_ipc::protocol::Desktop>) {
    for _ in 0..500 {
        if loads(fake) > before {
            return last_load(fake);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no Load after the first {before}");
}

#[tokio::test]
async fn a_remotes_screens_reach_the_hook_and_leave_with_it_and_the_desktop_stays_this_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Fallback).await;
    let laptop_id = master.laptop_id();
    let (zones, desktop) = last_load(&master.fake);
    assert!(!zones.contains("Host="), "{zones}");
    let here = desktop.expect("the desktop travels with the layout");
    let alone_id = master.alone["LayoutId"].as_str().unwrap().to_owned();

    // The laptop links and reports a screen of its own: its zone is in what the hook
    // runs, marked as the laptop's; the desktop the hook spans is still this one.
    let link = master.link_the_laptop().await;
    let together = master
        .laptop_shows(&link, 1, vec![laptop_screen("eDP-1", None, 0)])
        .await;
    assert_ne!(together["LayoutId"], alone_id.as_str(), "a monitor more");
    let (zones, desktop) = last_load(&master.fake);
    assert!(zones.contains(&format!("Host=\"{laptop_id}\"")), "{zones}");
    assert_eq!(desktop, Some(here));
    assert!(master.fake.hooked());

    // The laptop leaves, and stays away: back to this machine's layout, enabled
    // before, so the hook is handed it again at once — with no zone of the laptop's.
    let loads_before = loads(&master.fake);
    drop(master.listener);
    link.sender().close("going home").await;
    drop(link);
    let back = master
        .frontend
        .state(|s| s["LayoutId"] == alone_id.as_str())
        .await;
    assert_eq!(back["Enabled"], true);
    let (zones, desktop) = load_after(&master.fake, loads_before).await;
    assert!(!zones.contains("Host="), "{zones}");
    assert_eq!(desktop, Some(here));
}

/// The layout is rebuilt for a remote gone only after the display-change debounce. A
/// Start meanwhile — the user's, the watchdog's — must not hand the hook the layout built
/// while that remote was selected: its zones would lead to a machine the stream no longer
/// goes to. The rebuild on its way hands the hook the right one.
#[tokio::test]
async fn a_start_before_the_rebuild_for_a_departed_remote_never_hands_the_hook_its_zones() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Fallback).await;
    let laptop_id = master.laptop_id();
    let alone_id = master.alone["LayoutId"].as_str().unwrap().to_owned();
    let link = master.link_the_laptop().await;
    master
        .laptop_shows(&link, 1, vec![laptop_screen("eDP-1", None, 0)])
        .await;
    assert!(last_load(&master.fake).0.contains("Host="));

    let loads_before = loads(&master.fake);
    drop(master.listener);
    link.sender().close("going home").await;
    drop(link);
    // The agent has heard the laptop go; its rebuild waits for the debounce.
    loop {
        let frame = master.frontend.receive().await;
        if frame["Event"] == "Peers" && frame["Peers"]["SelectedRemote"].is_null() {
            break;
        }
    }
    master.frontend.ask(json!({ "Method": "Start" })).await;
    master
        .frontend
        .state(|s| s["LayoutId"] == alone_id.as_str())
        .await;
    load_after(&master.fake, loads_before).await;
    let handed: Vec<String> = master.fake.received()[..]
        .iter()
        .filter_map(|c| match c {
            Command::Load { zones, .. } => Some(zones.clone()),
            _ => None,
        })
        .skip(loads_before)
        .collect();
    assert!(
        handed.iter().all(|zones| !zones.contains(&laptop_id)),
        "{handed:?}"
    );
}

/// The hook makes the remote's zones enterable while the agent holds a connection to its
/// input endpoint, and a batch carries no destination: the agent connects only while
/// every zone the hook holds, and every one on its way, leads to the selected remote and
/// nowhere else — as the hook confirmed it, not as the agent hopes.
#[cfg(unix)]
#[tokio::test]
async fn the_master_relays_its_hooks_input_only_while_the_hook_has_confirmed_zones_for_the_selected_remote(
) {
    use lbm_ipc::input::read_batch;
    let tmp = tempfile::tempdir().unwrap();
    let input_path = tmp.path().join("hook-input.sock");
    let input = tokio::net::UnixListener::bind(&input_path).unwrap();
    let accept = || async {
        tokio::time::timeout(Duration::from_secs(15), input.accept())
            .await
            .expect("the agent connects to the hook's input endpoint")
            .unwrap()
            .0
    };
    let nobody_connects = || async {
        tokio::time::timeout(Duration::from_millis(500), input.accept())
            .await
            .is_err()
    };
    let mut master = Master::start_with(
        &tmp,
        lbm_agent::discovery::Discovery::Fallback,
        Some(input_path.to_string_lossy().into_owned()),
    )
    .await;
    let laptop_id = master.laptop_id();
    let link = master.link_the_laptop().await;
    master
        .laptop_shows(&link, 1, vec![laptop_screen("eDP-1", None, 0)])
        .await;
    let (zones, _) = last_load(&master.fake);
    let laptops = format!("Host=\"{laptop_id}\"");
    assert!(zones.contains(&laptops), "{zones}");
    let mut relaying = accept().await;

    // Another client hands the hook a layout with a zone of a machine that is not the
    // selected remote: the agent cannot vouch for what the hook holds now, lets go of
    // its input (the hook brings the pointer home) and does not come back.
    let stale = zones.replace(&laptops, r#"Host="another-machine""#);
    let mut other = tokio::net::UnixStream::connect(endpoints(&tmp).0)
        .await
        .unwrap();
    write_frame(
        &mut other,
        &lbm_ipc::protocol::frame(&[
            Command::Load {
                zones: stale,
                desktop: None,
                id: None,
            },
            Command::Run,
        ]),
    )
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(15), read_batch(&mut relaying))
            .await
            .expect("the agent lets go of the hook's input")
            .is_err()
    );
    assert!(nobody_connects().await);

    // The user's Start hands the hook the laptop's layout again. It is on its way, not
    // confirmed: still no connection, until the hook says it loaded it.
    master.fake.hold();
    let handed = loads(&master.fake);
    master.frontend.ask(json!({ "Method": "Start" })).await;
    load_after(&master.fake, handed).await;
    assert!(nobody_connects().await);
    master.fake.release();
    let _relaying = accept().await;
}

#[tokio::test]
async fn a_remote_monitors_placement_is_saved_and_rebuilt_from_the_store() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Fallback).await;
    let laptop_id = master.laptop_id();
    let screens = vec![laptop_screen("eDP-1", None, 0)];
    let link = master.link_the_laptop().await;
    let together = master.laptop_shows(&link, 1, screens.clone()).await;
    let layout_id = together["LayoutId"].as_str().unwrap().to_owned();

    // The frontend builds the agent's layout by the same steps, and moves the laptop's
    // screen to the left of this machine's.
    let remotes = [(laptop_id.as_str(), "laptop", screens)];
    let mut edited = master_layout(tmp.path(), &[], &remotes);
    assert_eq!(edited.id, layout_id, "the same layout as the agent's");
    let panel = lbm_layout::remote::remote_source_id(&laptop_id, "eDP-1");
    edited.set_location(&panel, lbm_layout::geo::Point::new(-400.0, 25.0));
    edited.set_placed(&panel, true);
    let moved = edited.monitor(&panel).unwrap().location();
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&edited)).unwrap();
    let saved = master
        .frontend
        .ask(json!({ "Method": "SaveLayout", "LayoutId": layout_id, "Document": document }))
        .await;
    assert_eq!(saved["Result"], Value::Null, "{saved}");

    // Rebuilt from the displays and the store: the laptop's screen is where it was
    // put, in millimetres, and the hook runs exactly what the frontend reads back.
    let revision = together["LayoutRevision"].as_u64().unwrap();
    master.frontend.ask(json!({ "Method": "Refresh" })).await;
    master
        .frontend
        .state(|s| s["LayoutRevision"].as_u64().unwrap() > revision && s["Engine"] == "Running")
        .await;
    let reread = master_layout(tmp.path(), &[], &remotes);
    assert_eq!(reread.monitor(&panel).unwrap().location(), moved);
    assert!(reread.monitor(&panel).unwrap().placed);
    let expected = lbm_layout::zoning::compute_zones(&reread).serialize();
    let mut ran = String::new();
    for _ in 0..500 {
        ran = last_load(&master.fake).0;
        if ran == expected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(ran, expected);
}

#[tokio::test]
async fn choosing_the_machine_a_shared_panel_shows_hands_its_zone_to_that_machine() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Given(dell_here())).await;
    let laptop_id = master.laptop_id();
    let (_, here) = last_load(&master.fake);
    let alone_id = master.alone["LayoutId"].as_str().unwrap().to_owned();

    // The laptop has its own panel, and the Dell on HDMI: the Dell is one monitor with
    // two sources, this machine's shown first.
    let screens = vec![
        laptop_screen("eDP-1", None, -1920),
        laptop_screen("HDMI-1", Some(dell_edid("ABC123", 2)), 0),
    ];
    let link = master.link_the_laptop().await;
    let together = master.laptop_shows(&link, 1, screens.clone()).await;
    let layout_id = together["LayoutId"].as_str().unwrap().to_owned();
    let dell_there = lbm_layout::remote::remote_source_id(&laptop_id, "HDMI-1");
    // How many screens the engine routes to the laptop. Every zone the document
    // describes: a parsed layout keeps the other machines' out of `main_zones` until a
    // hook's input endpoint has a client (`set_remote_enterable`, lot 6).
    let on_the_laptop = |zones: &str| -> usize {
        let mut layout = lbm_zones::ZonesLayout::from_xml(zones).unwrap();
        // What the hook's router sees once this agent is connected to its input stream.
        layout.set_remote_enterable(true);
        layout
            .all_zones
            .iter()
            .filter(|id| layout.arena[**id].host.as_deref() == Some(laptop_id.as_str()))
            .count()
    };
    let (zones, _) = last_load(&master.fake);
    assert_eq!(
        on_the_laptop(&zones),
        1,
        "the Dell shown here first: {zones}"
    );

    // The user shows the laptop on the Dell: its zone goes to the laptop, at once,
    // and the choice is in the profile.
    let loads_before = loads(&master.fake);
    let chosen = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": dell_there,
        }))
        .await;
    assert_eq!(chosen["Result"], Value::Null, "{chosen}");
    // A running engine is told.
    let (zones, desktop) = load_after(&master.fake, loads_before).await;
    assert_eq!(on_the_laptop(&zones), 2, "{zones}");
    assert_eq!(desktop, here, "the Dell is still an output of this desktop");
    let remotes = [(laptop_id.as_str(), "laptop", screens)];
    let reread = master_layout(tmp.path(), dell_here(), &remotes);
    assert_eq!(
        reread
            .monitor(SHARED_DELL)
            .unwrap()
            .active_source
            .as_deref(),
        Some(dell_there.as_str())
    );

    // And back.
    let loads_before = loads(&master.fake);
    let back = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": SHARED_DELL,
        }))
        .await;
    assert_eq!(back["Result"], Value::Null, "{back}");
    assert_eq!(
        on_the_laptop(&load_after(&master.fake, loads_before).await.0),
        1
    );

    // Neither a source of another monitor nor a layout that is not the current one.
    let panel = lbm_layout::remote::remote_source_id(&laptop_id, "eDP-1");
    let foreign = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": panel,
        }))
        .await;
    assert!(foreign["Error"].is_string(), "{foreign}");
    let stale = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": alone_id,
            "Monitor": SHARED_DELL,
            "Source": SHARED_DELL,
        }))
        .await;
    assert!(stale["Error"].is_string(), "{stale}");

    // A document made without the laptop is not for this layout, even when the ids
    // agree — as they would, had the laptop shared the Dell alone.
    let mut alone = master_layout(tmp.path(), dell_here(), &[]);
    alone.id = layout_id.clone();
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&alone)).unwrap();
    let refused = master
        .frontend
        .ask(json!({ "Method": "SaveLayout", "LayoutId": layout_id, "Document": document }))
        .await;
    assert!(
        refused["Error"]
            .as_str()
            .unwrap_or_default()
            .contains("other machines"),
        "{refused}"
    );
    drop(link);
}

#[tokio::test]
async fn a_remote_that_shares_only_a_panel_keeps_the_layout_id_and_still_rebuilds() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Given(dell_here())).await;
    let laptop_id = master.laptop_id();
    let alone_id = master.alone["LayoutId"].as_str().unwrap().to_owned();
    let before = master.alone["LayoutRevision"].as_u64().unwrap();

    // Only the Dell, on its HDMI input: no monitor more, the same id and profile, so
    // the engine stays on — yet the layout is rebuilt with the laptop's source in it.
    let link = master.link_the_laptop().await;
    link.sender()
        .control(&lbm_net::wire::Control::Displays {
            revision: 1,
            displays: vec![laptop_screen("HDMI-1", Some(dell_edid("ABC123", 2)), 0)],
        })
        .await
        .unwrap();
    let together = master
        .frontend
        .state(|s| s["LayoutRevision"].as_u64().unwrap() > before)
        .await;
    assert_eq!(together["LayoutId"], alone_id.as_str());
    assert_ne!(
        together["DisplaySignature"],
        master.alone["DisplaySignature"]
    );
    let dell_there = lbm_layout::remote::remote_source_id(&laptop_id, "HDMI-1");
    let chosen = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": alone_id,
            "Monitor": SHARED_DELL,
            "Source": dell_there,
        }))
        .await;
    assert_eq!(chosen["Result"], Value::Null, "{chosen}");
    drop(link);
}

/// Waits for `what` to hold, a few seconds at most.
async fn eventually(what: impl Fn() -> bool, said: &str) {
    for _ in 0..500 {
        if what() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never: {said}");
}

/// The next control message on `link` that `want` takes, skipping the others.
async fn next_control_where(
    link: &mut lbm_net::link::Link,
    want: impl Fn(&lbm_net::wire::Control) -> bool,
) -> lbm_net::wire::Control {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match link.next().await {
                Some(lbm_net::link::Event::Control(message)) if want(&message) => return message,
                Some(lbm_net::link::Event::Control(_) | lbm_net::link::Event::Input(_)) => {}
                other => panic!("the link ended: {other:?}"),
            }
        }
    })
    .await
    .expect("the control message in time")
}

/// Turns VCP control on in the layout of this machine alone, as the user does in
/// Settings: nothing is read from or written to a monitor before.
async fn enable_vcp_control(
    frontend: &mut Frontend,
    dir: &std::path::Path,
    outputs: &[lbm_layout::linux::LinuxMonitor],
    layout_id: &str,
) {
    let mut layout = master_layout(dir, outputs, &[]);
    assert_eq!(layout.id, layout_id);
    layout.edit_options(|o| o.vcp_control = true);
    let document = serde_json::to_value(lbm_store::LayoutDocument::of(&layout)).unwrap();
    let saved = frontend
        .ask(json!({ "Method": "SaveLayout", "LayoutId": layout_id, "Document": document }))
        .await;
    assert_eq!(saved["Result"], Value::Null, "{saved}");
}

const ON_DP: u16 = 0x0f;
const ON_HDMI: u16 = 0x11;

fn input_write(value: u16) -> lbm_agent::vcp::DdcCall {
    lbm_agent::vcp::DdcCall::Write {
        connector: "DP-1".into(),
        code: 0x60,
        value,
    }
}

#[tokio::test]
async fn choosing_the_machine_a_shared_panel_shows_switches_its_input_and_nothing_else_does() {
    let tmp = tempfile::tempdir().unwrap();
    let mut master = Master::start(&tmp, lbm_agent::discovery::Discovery::Given(dell_here())).await;
    let laptop_id = master.laptop_id();
    let layout_id = master.alone["LayoutId"].as_str().unwrap().to_owned();
    // The Dell shows this machine, on DisplayPort.
    master.ddc.set("DP-1", 0x60, ON_DP);
    enable_vcp_control(&mut master.frontend, tmp.path(), dell_here(), &layout_id).await;

    // The laptop has the Dell on HDMI, and has read nothing of it yet. The display
    // change has this machine read its own input, the panel showing it.
    let mut link = master.link_the_laptop().await;
    let dell_on_hdmi = |input_source| lbm_ipc::peer::PeerDisplay {
        input_source,
        ..laptop_screen("HDMI-1", Some(dell_edid("ABC123", 2)), 0)
    };
    link.sender()
        .control(&lbm_net::wire::Control::Displays {
            revision: 1,
            displays: vec![dell_on_hdmi(None)],
        })
        .await
        .unwrap();
    let remotes = |input| [(laptop_id.as_str(), "laptop", vec![dell_on_hdmi(input)])];
    let stored_input = |source: &str| {
        master_layout(tmp.path(), dell_here(), &remotes(None))
            .source(source)
            .and_then(|s| s.source.input_source)
    };
    eventually(
        || stored_input(SHARED_DELL) == Some(ON_DP),
        "DisplayPort learnt",
    )
    .await;
    let dell_there = lbm_layout::remote::remote_source_id(&laptop_id, "HDMI-1");
    assert_eq!(stored_input(&dell_there), None);

    // The laptop chosen while its input is unknown: the routing follows, and nothing
    // is written — the hardware KVM case.
    let chosen = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": dell_there,
        }))
        .await;
    assert_eq!(chosen["Result"], Value::Null, "{chosen}");

    // Now shown, the laptop reads HDMI and says so: a display change here, which this
    // machine learns from, and which writes nothing.
    link.sender()
        .control(&lbm_net::wire::Control::Displays {
            revision: 2,
            displays: vec![dell_on_hdmi(Some(ON_HDMI))],
        })
        .await
        .unwrap();
    eventually(|| stored_input(&dell_there) == Some(ON_HDMI), "HDMI learnt").await;
    assert_eq!(
        stored_input(SHARED_DELL),
        Some(ON_DP),
        "this machine's own kept"
    );
    assert!(master.ddc.writes().is_empty(), "{:?}", master.ddc.calls());

    // Back to this machine: the Dell shows HDMI, so DisplayPort is written, here.
    master.ddc.set("DP-1", 0x60, ON_HDMI);
    let back = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": SHARED_DELL,
        }))
        .await;
    assert_eq!(back["Result"], Value::Null, "{back}");
    eventually(
        || master.ddc.writes() == [input_write(ON_DP)],
        "DisplayPort written",
    )
    .await;

    // To the laptop again, the Dell silent on the cable it no longer shows: the laptop
    // is asked to switch it, by its own name for it.
    master.ddc.silence("DP-1");
    let there = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": dell_there,
        }))
        .await;
    assert_eq!(there["Result"], Value::Null, "{there}");
    let asked = next_control_where(&mut link, |c| {
        matches!(c, lbm_net::wire::Control::SelectInput { .. })
    })
    .await;
    let lbm_net::wire::Control::SelectInput {
        display,
        value,
        request,
    } = asked
    else {
        unreachable!()
    };
    assert_eq!((display.as_str(), value), ("HDMI-1", ON_HDMI));
    assert!(request.is_some(), "numbered, for the answer to name it");
    assert_eq!(
        master.ddc.writes(),
        [input_write(ON_DP)],
        "one write, the one chosen"
    );

    // The laptop could not either: the window is told which panel, and why on both
    // machines. An answer to another request first, which is no news.
    let answer = |request, error: Option<&str>| lbm_net::wire::Control::InputSwitched {
        display: "HDMI-1".into(),
        value: ON_HDMI,
        request,
        error: error.map(str::to_owned),
    };
    let stale = request.map(|r| r + 100);
    link.sender()
        .control(&answer(stale, Some("not this one")))
        .await
        .unwrap();
    link.sender()
        .control(&answer(request, Some("ddcutil: code -3021")))
        .await
        .unwrap();
    let state = master
        .frontend
        .state(|s| s["InputSwitchFailures"].is_array())
        .await;
    let failures = state["InputSwitchFailures"].as_array().unwrap();
    assert_eq!(failures.len(), 1, "{state}");
    assert_eq!(failures[0]["Monitor"], SHARED_DELL);
    assert_eq!(failures[0]["Source"], dell_there.as_str());
    assert_eq!(failures[0]["Value"], ON_HDMI);
    assert_eq!(failures[0]["Remote"], laptop_id.as_str());
    assert_eq!(failures[0]["RemoteError"], "ddcutil: code -3021");
    assert!(
        !failures[0]["Error"].as_str().unwrap().is_empty(),
        "why this machine could not: {state}"
    );

    // Back to this machine, the Dell still silent here: the laptop is asked, does it,
    // and the failure goes.
    let back = master
        .frontend
        .ask(json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": SHARED_DELL,
        }))
        .await;
    assert_eq!(back["Result"], Value::Null, "{back}");
    let lbm_net::wire::Control::SelectInput {
        value,
        request: again,
        ..
    } = next_control_where(&mut link, |c| {
        matches!(c, lbm_net::wire::Control::SelectInput { .. })
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(value, ON_DP);
    assert_ne!(again, request, "a new number for a new request");
    link.sender()
        .control(&lbm_net::wire::Control::InputSwitched {
            display: "HDMI-1".into(),
            value: ON_DP,
            request: again,
            error: None,
        })
        .await
        .unwrap();
    master
        .frontend
        .state(|s| s.get("InputSwitchFailures").is_none())
        .await;

    // The laptop again, and while that is asked of the laptop the user changes their
    // mind: the retry is accepted, and this machine's own go at it is held. The laptop's
    // failure for the earlier choice, arriving meanwhile, is not the retry's.
    let choose = |source: String| {
        json!({
            "Method": "SetActiveSource",
            "LayoutId": layout_id,
            "Monitor": SHARED_DELL,
            "Source": source,
        })
    };
    let selected =
        |c: &lbm_net::wire::Control| matches!(c, lbm_net::wire::Control::SelectInput { .. });
    let there = master.frontend.ask(choose(dell_there.clone())).await;
    assert_eq!(there["Result"], Value::Null, "{there}");
    let lbm_net::wire::Control::SelectInput {
        request: earlier, ..
    } = next_control_where(&mut link, selected).await
    else {
        unreachable!()
    };
    let gate = master.ddc.hold("DP-1");
    let back = master.frontend.ask(choose(SHARED_DELL.to_owned())).await;
    assert_eq!(back["Result"], Value::Null, "{back}");
    link.sender()
        .control(&lbm_net::wire::Control::InputSwitched {
            display: "HDMI-1".into(),
            value: ON_HDMI,
            request: earlier,
            error: Some("busy".into()),
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let now = master.frontend.ask(json!({ "Method": "Subscribe" })).await;
    assert!(
        now["Result"].get("InputSwitchFailures").is_none(),
        "an earlier choice's failure shown for the retry: {now}"
    );
    // Let go, the retry is asked of the laptop under its own number, and settled by
    // its own answer.
    gate.add_permits(16);
    let lbm_net::wire::Control::SelectInput {
        request: retry,
        value,
        ..
    } = next_control_where(&mut link, selected).await
    else {
        unreachable!()
    };
    assert_eq!(value, ON_DP);
    assert_ne!(retry, earlier);
    link.sender()
        .control(&lbm_net::wire::Control::InputSwitched {
            display: "HDMI-1".into(),
            value: ON_DP,
            request: retry,
            error: None,
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let now = master.frontend.ask(json!({ "Method": "Subscribe" })).await;
    assert!(now["Result"].get("InputSwitchFailures").is_none(), "{now}");
}

#[tokio::test]
async fn a_remote_tells_its_master_what_selects_it_and_switches_when_its_master_asks() {
    use lbm_net::wire::{Control, Hello, Role};

    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let (hook_endpoint, api_endpoint) = endpoints(&tmp);
    let _fake = FakeHook::bind(&hook_endpoint).unwrap();
    let config = dir.join("config");
    let desk = lbm_net::identity::Identity::load_or_create(&dir.join("desk")).unwrap();
    lbm_net::peers::PeerStore::load(&config)
        .insert(lbm_net::peers::PairedPeer {
            machine_id: desk.fingerprint().to_owned(),
            name: "desk".into(),
            address: String::new(),
            role: Role::Master,
        })
        .unwrap();
    let ddc = std::sync::Arc::new(lbm_agent::vcp::FakeDdc::default());
    // The Dell shows this machine, on HDMI.
    ddc.set("DP-1", 0x60, ON_HDMI);

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, listening) = listen(&api_endpoint);
    let peers = lbm_agent::peers::Service::new(
        config,
        lbm_agent::peers::Setup {
            bind: "127.0.0.1".parse().unwrap(),
            input_endpoint: None,
            options: lbm_net::link::Options::default(),
            clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
        },
    );
    let world = lbm_agent::world::SystemWorld::new(
        lbm_agent::discovery::Discovery::Given(dell_here()),
        persistence(dir),
    );
    let bus = ddc.clone();
    tokio::spawn(async move {
        let _listening = listening;
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_api(calls)
            .with_peers(peers)
            .with_ddc(bus);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let mut frontend = Frontend::connect(&api_endpoint).await;
    let state = frontend.ask(json!({ "Method": "Subscribe" })).await["Result"].clone();
    let layout_id = state["LayoutId"].as_str().unwrap().to_owned();
    enable_vcp_control(&mut frontend, dir, dell_here(), &layout_id).await;
    let on = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": true, "Port": 0 }))
        .await;
    let address = on["Result"]["ListenAddress"].as_str().unwrap().to_owned();

    // Its master links: it is told the input that selects this machine on the Dell.
    let mut link = lbm_net::link::connect(
        address.as_str(),
        &desk,
        Hello::new("desk", "0", Role::Master),
        lbm_net::link::Options::default(),
    )
    .await
    .unwrap()
    .accept()
    .await
    .unwrap();
    let reports = |value| {
        move |c: &Control| {
            matches!(c, Control::Displays { displays, .. }
                if displays.iter().any(|d| d.id == SHARED_DELL && d.input_source == Some(value)))
        }
    };
    next_control_where(&mut link, reports(ON_HDMI)).await;

    // A display change, the Dell on DisplayPort now: read again and told, never written.
    ddc.set("DP-1", 0x60, ON_DP);
    frontend.ask(json!({ "Method": "Refresh" })).await;
    next_control_where(&mut link, reports(ON_DP)).await;
    assert!(ddc.writes().is_empty(), "{:?}", ddc.calls());

    // The master asks for a display this machine does not have: nothing. Then for the
    // Dell: switched.
    for (display, value) in [("NOT-HERE", ON_HDMI), (SHARED_DELL, ON_HDMI)] {
        link.sender()
            .control(&Control::SelectInput {
                display: display.into(),
                value,
                request: None,
            })
            .await
            .unwrap();
    }
    eventually(|| ddc.writes() == [input_write(ON_HDMI)], "HDMI written").await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(ddc.writes(), [input_write(ON_HDMI)]);
}

#[test]
fn a_panel_switched_to_a_machine_not_showing_it_yet_keeps_the_choice_until_it_does() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let laptop_id = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
    let dell_on_hdmi = |attached| lbm_ipc::peer::PeerDisplay {
        attached,
        ..laptop_screen("HDMI-1", Some(dell_edid("ABC123", 2)), 0)
    };
    let laptop = |attached| {
        vec![lbm_agent::peers::RemoteScreens {
            machine_id: laptop_id.into(),
            name: "laptop".into(),
            displays: vec![dell_on_hdmi(attached)],
        }]
    };
    let dell_there = lbm_layout::remote::remote_source_id(laptop_id, "HDMI-1");
    let mut world = lbm_agent::world::SystemWorld::new(
        lbm_agent::discovery::Discovery::Given(dell_here()),
        persistence(dir),
    );
    let shown = |world: &lbm_agent::world::SystemWorld<_, _>| {
        world
            .layout()
            .unwrap()
            .monitor(SHARED_DELL)
            .unwrap()
            .active_source
            .clone()
            .unwrap()
    };

    // The Dell shows this machine; the laptop has lost it.
    world.set_remote_screens(laptop(false));
    world.rebuild_layout();
    let layout_id = world.layout_id().unwrap();
    assert_eq!(shown(&world), SHARED_DELL);
    // Nothing known of the laptop's input: it cannot be chosen, nothing could bring
    // the panel to it.
    assert!(world
        .set_active_source(&layout_id, SHARED_DELL, &dell_there)
        .is_err());

    // Its input learnt earlier, it can: the switch is what brings the panel there.
    let mut remembered = master_layout(
        dir,
        dell_here(),
        &[(laptop_id, "laptop", vec![dell_on_hdmi(false)])],
    );
    remembered.set_source_input(&dell_there, Some(ON_HDMI));
    world
        .edit(&layout_id, &lbm_store::LayoutDocument::of(&remembered))
        .unwrap();
    world.save_layout().unwrap();
    let switch = world
        .set_active_source(&layout_id, SHARED_DELL, &dell_there)
        .unwrap()
        .expect("a switch");
    assert_eq!(switch.value, ON_HDMI);
    assert_eq!(
        switch.remote.map(|r| r.display).as_deref(),
        Some("HDMI-1"),
        "the laptop can be asked"
    );

    // A display change before the laptop has the panel back — this machine losing it,
    // say — does not take the choice back.
    world.rebuild_layout();
    assert_eq!(shown(&world), dell_there);
    // Nobody could switch it after all: back to the machine that has it.
    assert!(world.switch_failed(SHARED_DELL, &dell_there));
    assert_eq!(shown(&world), SHARED_DELL);

    // Chosen again, and the laptop has the panel: from then on the rule keeps it, both
    // machines attached, through any display change, and nothing is waited for.
    world
        .set_active_source(&layout_id, SHARED_DELL, &dell_there)
        .unwrap();
    world.set_remote_screens(laptop(true));
    world.rebuild_layout();
    assert_eq!(shown(&world), dell_there);
    world.rebuild_layout();
    assert_eq!(shown(&world), dell_there);
    assert!(!world.switch_failed(SHARED_DELL, &dell_there));
    assert_eq!(shown(&world), dell_there);
}

/// The pointer lock: the hook is told its shortcut only while the mode is on, and the
/// state says when the hook locked the pointer, until the hook goes down.
#[tokio::test]
async fn the_hook_watches_for_the_pointer_lock_only_while_the_mode_is_on() {
    let dir = tempfile::tempdir().unwrap();
    let (hook_endpoint, api_endpoint) = endpoints(&dir);
    let fake = FakeHook::bind(&hook_endpoint).unwrap();
    let config = dir.path().join("config");

    let (hook, signals) = HookClient::spawn(hook_endpoint);
    let (inputs, inputs_rx) = tokio::sync::mpsc::unbounded_channel();
    let (calls, _listener) = listen(&api_endpoint);
    let peers = lbm_agent::peers::Service::new(
        config.clone(),
        lbm_agent::peers::Setup {
            bind: "127.0.0.1".parse().unwrap(),
            input_endpoint: None,
            options: lbm_net::link::Options::default(),
            clipboard: lbm_agent::clipboard::memory::Memory::new().factory(),
        },
    );
    tokio::spawn(async move {
        let world = Screens {
            state: Enabled(None),
            rebuilds: 0,
        };
        let mut agent = Agent::new(world, Timings::default(), hook, inputs)
            .with_api(calls)
            .with_peers(peers);
        agent.run(signals, inputs_rx, std::future::pending()).await;
    });
    let told = |want: &'static str| {
        let fake = &fake;
        async move {
            tokio::time::timeout(Duration::from_secs(5), async {
                while fake.pointer_lock() != want {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!("the hook was told {want:?}, not {:?}", fake.pointer_lock())
            });
        }
    };

    let mut frontend = Frontend::connect(&api_endpoint).await;
    frontend.ask(json!({ "Method": "Subscribe" })).await;
    frontend.state(|s| s["HookConnected"] == true).await;

    // Set while the mode is off: kept, and nothing is watched for.
    let off = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": false,
                     "PointerLockShortcut": "Ctrl+Alt+L" }))
        .await["Result"]
        .clone();
    assert_eq!(off["PointerLockShortcut"], "Ctrl+Alt+L");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(fake.pointer_lock(), "");

    let on = frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": true, "Port": 0 }))
        .await["Result"]
        .clone();
    assert_eq!(on["PointerLocked"], false);
    told("Ctrl+Alt+L").await;

    fake.broadcast(&Event::PointerLocked { locked: true });
    frontend.state(|s| s["PointerLocked"] == true).await;
    // The hook's routing goes down, and its lock with it.
    fake.broadcast(&Event::Stopped);
    frontend.state(|s| s["PointerLocked"] == false).await;

    frontend
        .ask(json!({ "Method": "SetMultiPc", "Enabled": false }))
        .await;
    told("").await;
}
