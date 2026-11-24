//! The agent's event loop: every input — from the hook, the display poll, the timers
//! the reconciler asked for — goes through the reconciler, one at a time, and its
//! effects are carried out here, in order.

use std::future::Future;

use lbm_ipc::protocol::{self, Command, Event};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::api::{self, Call, Request, Snapshot};
use crate::hook::{HookClient, HookSignal};
use crate::input_switch::{self, InputRead, InputSwitch, Outcome};
use crate::peers::{Answer, Notice, PeerRequest, PeerState};
use crate::reconcile::{Effect, EngineState, HookEvent, Input, Reconciler, Timings};
use crate::supervise::{HookLauncher, Launch};
use crate::world::AgentWorld;

/// What the system is doing (see `sleep`, on Linux).
#[derive(Debug)]
pub enum SleepSignal {
    /// About to sleep: take the hook down, then answer (the machine waits for it).
    Starting(tokio::sync::oneshot::Sender<()>),
    /// Awake again.
    Ended,
}

/// A hook event as the reconciler knows it. The two vocabularies are deliberately not
/// the same one: the reconciler also reasons about a hook that is not there
/// (`Connected`, `Dead`), which is not something a hook can say.
fn hook_event(message: Event) -> HookEvent {
    match message {
        Event::Running => HookEvent::Running,
        Event::Stopped => HookEvent::Stopped,
        Event::Paused => HookEvent::Paused,
        Event::Dead => HookEvent::Dead,
        Event::SettingsChanged => HookEvent::SettingsChanged,
        Event::DisplayChanged => HookEvent::DisplayChanged,
        Event::DesktopChanged => HookEvent::DesktopChanged,
        Event::FocusChanged { process } => HookEvent::FocusChanged(process),
        Event::Suspended => HookEvent::Suspended,
        Event::Resumed => HookEvent::Resumed,
        Event::Loaded { .. } => HookEvent::Loaded,
        Event::LoadFailed => HookEvent::LoadFailed,
        Event::Rescued => HookEvent::Rescued,
        Event::ShortcutUnavailable { .. } => HookEvent::ShortcutUnavailable,
        Event::ShortcutAvailable { .. } => HookEvent::ShortcutAvailable,
        // The handshake never comes through here: the connection answers it and
        // reports what it learned as `HookSignal::Greeted`. An event this version does
        // not know is one it has nothing to do about.
        Event::Hello { .. } | Event::Unknown => HookEvent::Unknown,
    }
}

/// What the monitor work done away from the loop (`input_switch`) reports back to it.
enum VcpEvent {
    /// The inputs read after the rebuild `rebuild`.
    Read {
        rebuild: u64,
        results: Vec<(InputRead, Result<u16, String>)>,
    },
    /// A switch this machine attempted.
    Switched {
        switch: Box<InputSwitch>,
        outcome: Outcome,
    },
    /// A switch this machine's master `from` asked for, and whether it wrote.
    Selected {
        from: String,
        display: String,
        value: u16,
        result: Result<bool, String>,
    },
}

/// The agent: the reconciler, the world it acts on, the hook it drives.
pub struct Agent<W> {
    reconciler: Reconciler,
    world: W,
    hook: HookClient,
    inputs: UnboundedSender<Input>,
    launcher: Option<HookLauncher>,
    /// The frontends' requests, when the agent serves them.
    calls: Option<UnboundedReceiver<Call>>,
    subscribers: Vec<api::Client>,
    /// The connection a live preview belongs to, while one is running.
    previewing_for: Option<api::Client>,
    /// What the subscribers last saw.
    published: Option<Snapshot>,
    hook_connected: bool,
    /// A frontend asked the agent to leave.
    quitting: bool,
    seen: api::SeenProcesses,
    /// System sleep, where the platform reports it to the agent (Linux: logind).
    sleep: Option<UnboundedReceiver<SleepSignal>>,
    /// The zones the hook was last handed; `None`: unknown.
    on_the_wire: Option<String>,
    /// The rescue shortcut the hook was last told (Windows).
    shortcut: Option<String>,
    /// The shortcut the hook last said it could not arm, on this connection.
    shortcut_unavailable: Option<String>,
    /// Whether the hook was last told it belongs to this agent; `None` on a fresh
    /// connection, so a hook that just adopted us is always told.
    bound: Option<bool>,
    /// The exclusion list the hook was last handed; `None` on a fresh connection.
    excluded: Option<Vec<String>>,
    /// Retire prepared work when a newer request or rebuilt layout replaces it.
    wallpaper_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    wallpaper_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    /// Where the wallpaper is painted: nowhere ([`NoDesktop`](crate::desktop::NoDesktop))
    /// unless `main` installs the session's (`with_desktop`). A test cannot reach the
    /// developer's desktop by forgetting a double.
    desktop: std::sync::Arc<dyn crate::desktop::Desktop>,
    /// Counts the reads of this machine's wallpapers for its masters: only the latest
    /// one's pictures are sent.
    wallpaper_reads: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Where the desktop's watch says the wallpapers may have changed, and the watch
    /// itself: held only while a master is linked (`peers_changed`), so a machine nobody
    /// looks at watches nothing.
    wallpaper_changes: crate::wallpaper_watch::Changes,
    wallpaper_changes_rx: Option<UnboundedReceiver<()>>,
    wallpaper_watch: Option<crate::wallpaper_watch::Watching>,
    /// The property panel's monitor control, when this agent offers it
    /// (`with_monitor_control`); the lock is also the one DDC/CI bus of this machine, so
    /// the input switching holds it too, with or without a panel to serve.
    vcp: std::sync::Arc<tokio::sync::Mutex<Option<crate::vcp::Service>>>,
    /// The network TV remotes, when this agent has a configuration to read them from
    /// (`with_remote_config`): none by default, so a test never loads the user's pairings.
    remote: Option<std::sync::Arc<tokio::sync::Mutex<crate::remote::Service>>>,
    /// The multi-PC mode, when this agent offers it.
    peers: Option<crate::peers::Service>,
    /// The monitors' input, read and written for the multi-PC mode (`input_switch`).
    /// [`NoDdc`](crate::vcp::NoDdc) unless `main` or a test hands one over.
    ddc: std::sync::Arc<dyn crate::vcp::Ddc>,
    vcp_events: UnboundedSender<VcpEvent>,
    vcp_events_rx: Option<UnboundedReceiver<VcpEvent>>,
    /// This machine's monitors' inputs, as last read, by monitor id: what a master is
    /// told selects this machine on each.
    readings: std::collections::HashMap<String, u16>,
    /// Why a monitor's input could not be read, as last said: said again only when it
    /// changes, not at every display event.
    read_failures: std::collections::HashMap<String, String>,
    /// The input switches that failed, for the frontends (`InputSwitchFailures`).
    input_failures: input_switch::Failures,
    /// Whether a master is linked: only then are the panels this machine alone has
    /// read, to tell it what selects this machine on them.
    has_master: bool,
    /// The selected remote of the last `Peers` state, with that state's number for the
    /// choice (`PeersState::selection`); `None` before any. The zones this agent hands
    /// the hook never lead to another remote ([`Agent::zones_fit`]).
    selection_seen: Option<(u64, Option<String>)>,
    /// The other machines the zones the hook holds lead to; `None` when that is not
    /// known (no hook, a hook holding a layout this agent cannot name, a `Loaded` this
    /// agent did not ask for).
    hook_holds: Option<std::collections::BTreeSet<String>>,
    /// The same for every `Load` sent and not yet answered, oldest first: the hook answers
    /// each with `Loaded` or `LoadFailed`, in order.
    loads_in_flight: std::collections::VecDeque<std::collections::BTreeSet<String>>,
    /// What the multi-PC manager was last told of the hook's zones.
    told_hook_ready: Option<Option<u64>>,
}

impl<W: AgentWorld> Agent<W> {
    /// `inputs` is where timers (and any other source) send their inputs back; its
    /// receiving end is what [`run`](Self::run) reads.
    pub fn new(
        world: W,
        timings: Timings,
        hook: HookClient,
        inputs: UnboundedSender<Input>,
    ) -> Self {
        let (vcp_events, vcp_events_rx) = tokio::sync::mpsc::unbounded_channel();
        let (wallpaper_changes, wallpaper_changes_rx) = crate::wallpaper_watch::channel();
        Agent {
            reconciler: Reconciler::new(timings),
            world,
            hook,
            inputs,
            launcher: None,
            calls: None,
            subscribers: Vec::new(),
            previewing_for: None,
            published: None,
            hook_connected: false,
            quitting: false,
            seen: api::SeenProcesses::default(),
            sleep: None,
            on_the_wire: None,
            shortcut: None,
            shortcut_unavailable: None,
            bound: None,
            excluded: None,
            wallpaper_generation: Default::default(),
            wallpaper_lock: Default::default(),
            desktop: std::sync::Arc::new(crate::desktop::NoDesktop),
            wallpaper_reads: Default::default(),
            wallpaper_changes,
            wallpaper_changes_rx: Some(wallpaper_changes_rx),
            wallpaper_watch: None,
            vcp: Default::default(),
            remote: None,
            peers: None,
            ddc: std::sync::Arc::new(crate::vcp::NoDdc),
            vcp_events,
            vcp_events_rx: Some(vcp_events_rx),
            readings: Default::default(),
            read_failures: Default::default(),
            input_failures: Default::default(),
            has_master: false,
            selection_seen: None,
            hook_holds: None,
            loads_in_flight: Default::default(),
            told_hook_ready: None,
        }
    }

    /// Reads and writes the monitors' input through `ddc`: the system's DDC/CI in `main`
    /// ([`SystemDdc`](crate::vcp::SystemDdc)), the tests' recording double elsewhere.
    pub fn with_ddc(mut self, ddc: std::sync::Arc<dyn crate::vcp::Ddc>) -> Self {
        self.ddc = ddc;
        self
    }

    /// Paints the wallpaper on `desktop`: the session's in `main`
    /// ([`SystemDesktop`](crate::desktop::SystemDesktop)), a recording double in a test.
    pub fn with_desktop(mut self, desktop: std::sync::Arc<dyn crate::desktop::Desktop>) -> Self {
        self.desktop = desktop;
        self
    }

    /// Serves the property panel's monitor controls through `service` (`ddcutil`, dxva2).
    pub fn with_monitor_control(mut self, service: crate::vcp::Service) -> Self {
        self.vcp = std::sync::Arc::new(tokio::sync::Mutex::new(Some(service)));
        self
    }

    /// Offers the multi-PC mode (see [`crate::peers`]).
    pub fn with_peers(mut self, peers: crate::peers::Service) -> Self {
        self.peers = Some(peers);
        self
    }

    /// Follows system sleep from `signals` (Linux: `sleep::watch`).
    pub fn with_sleep(mut self, signals: UnboundedReceiver<SleepSignal>) -> Self {
        self.sleep = Some(signals);
        self
    }

    /// The system is about to sleep. What the Windows hook does on its own when the
    /// display goes off: the hook is taken down (the frame written before the machine
    /// is let go), and the agent stands down until the wake (`Suspended`).
    async fn sleep_starting(&mut self) {
        eprintln!("[lbm-agent] the system is going to sleep: -> Stop");
        self.hook.send(protocol::frame(&[Command::Stop]));
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), self.hook.flush()).await;
        self.handle(Input::Hook(HookEvent::Suspended));
    }

    /// Drives the network TV remotes paired in `path` (`samsung-tizen.json`, …).
    pub fn with_remote_config(mut self, path: std::path::PathBuf) -> Self {
        self.remote = Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            crate::remote::Service::new(path),
        )));
        self
    }

    /// Serves the frontends' requests coming out of `calls` (see [`api`]).
    pub fn with_api(mut self, calls: UnboundedReceiver<Call>) -> Self {
        self.calls = Some(calls);
        self
    }

    /// What a frontend sees of the agent now.
    pub fn snapshot(&self) -> Snapshot {
        let layout = crate::reconcile::World::layout(&self.world);
        Snapshot {
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
            hook_connected: self.hook_connected,
            engine: match self.reconciler.engine() {
                EngineState::Running => "Running",
                EngineState::Stopped => "Stopped",
                EngineState::Paused => "Paused",
                EngineState::Dead => "Dead",
            }
            .to_owned(),
            suspended: self.reconciler.suspended(),
            layout_id: self.world.layout_id(),
            layout_revision: self.reconciler.rebuild_count(),
            display_signature: self.reconciler.last_built_signature().to_owned(),
            enabled: layout.map(|l| l.enabled),
            saved: layout.map(|l| l.saved),
            load_at_startup: self.world.app_options().map(|(startup, _)| startup),
            hide_tray_icon: self.world.app_options().map(|(_, hidden)| hidden),
            previewing: self.reconciler.previewing(),
            shortcut_unavailable: unavailable_now(
                self.shortcut_unavailable.as_deref(),
                self.world.rescue_shortcut().as_deref(),
            ),
            input_switch_failures: self
                .input_failures
                .current(|monitor| self.world.has_monitor(monitor)),
        }
    }

    /// Sweeps the current layout's edges. `None` when there is no layout to sweep.
    /// The report goes out as a `Probed` event — the shape the hook used to answer in,
    /// so the frontends did not have to learn a second one.
    fn probe(&mut self) -> Option<String> {
        let (zones, _) = self.world.zones()?;
        lbm_engine::probe::probe_xml(&zones)
    }

    /// Sends the state to the subscribers when it changed since they last saw it.
    fn publish(&mut self) {
        let snapshot = self.snapshot();
        if self.published.as_ref() == Some(&snapshot) {
            return;
        }
        let event = api::state_event(&snapshot);
        self.subscribers.retain(|s| s.send(event.clone()));
        self.published = Some(snapshot);
    }

    /// One frontend request, or one frontend leaving.
    fn call(&mut self, call: Call) {
        let (id, request, client) = match call {
            Call::Asked {
                id,
                request,
                client,
            } => (id, request, client),
            Call::Gone(client) => return self.left(&client),
        };
        // Held until the answer has gone out: a frontend that asks and then listens
        // must not have the report arrive before the acknowledgement it is waiting on.
        let mut report = None;
        let result = match request {
            Request::Hello { .. } => Ok(serde_json::json!({
                "Agent": "lbm-agent",
                "Version": env!("CARGO_PKG_VERSION"),
                "Protocol": api::PROTOCOL,
                "Capabilities": api::CAPABILITIES,
            })),
            Request::Snapshot => serde_json::to_value(self.snapshot()).map_err(|e| e.to_string()),
            Request::WallpaperSettings {
                layout_id,
                layout_revision,
            } => {
                if layout_revision != self.reconciler.rebuild_count() {
                    Err("The displays changed; reload the wallpaper editor.".into())
                } else {
                    match self.world.wallpaper_settings(&layout_id) {
                        Err(e) => Err(e),
                        Ok(settings) => {
                            let desktop = self.desktop.clone();
                            tokio::spawn(async move {
                                let supported = tokio::time::timeout(
                                    std::time::Duration::from_secs(5),
                                    desktop.is_supported(),
                                )
                                .await
                                .unwrap_or(false);
                                client.send(api::answer(
                                    id,
                                    Ok(serde_json::json!({
                                        "Settings": settings,
                                        "Supported": supported
                                    })),
                                ));
                            });
                            return;
                        }
                    }
                }
            }
            Request::ApplyWallpaper {
                layout_id,
                layout_revision,
                settings,
            } => {
                if layout_revision != self.reconciler.rebuild_count()
                    || self.snapshot().suspended
                    || self.snapshot().previewing
                {
                    Err("The displays changed or the desktop is suspended; reload the wallpaper editor.".into())
                } else {
                    match self.world.wallpaper_job(&layout_id, *settings) {
                        Err(e) => Err(e),
                        Ok(job) => {
                            use std::sync::atomic::Ordering;
                            let generation = self.wallpaper_generation.clone();
                            let expected = generation.fetch_add(1, Ordering::SeqCst) + 1;
                            let lock = self.wallpaper_lock.clone();
                            let desktop = self.desktop.clone();
                            tokio::spawn(async move {
                                let _guard = lock.lock().await;
                                let cache = job.images.clone();
                                let store = job.store.clone();
                                let mut applied_successfully = false;
                                let result = async {
                                    let supported = tokio::time::timeout(
                                        std::time::Duration::from_secs(5),
                                        desktop.is_supported(),
                                    ).await.unwrap_or(false);
                                    if !supported {
                                        return Err("This desktop has no supported wallpaper service.".into());
                                    }
                                    let g = generation.clone();
                                    let screens = tokio::task::spawn_blocking(move || job.prepare(g, expected))
                                        .await.map_err(|e| e.to_string())??;
                                    let applied = if generation.load(Ordering::SeqCst) != expected {
                                        Err("Settings saved, but the displays changed before application.".into())
                                    } else {
                                        desktop.apply(screens).await
                                    };
                                    applied_successfully = applied.is_ok();
                                    Ok(serde_json::json!({
                                        "Saved": true,
                                        "Applied": applied.is_ok(),
                                        "Error": applied.err()
                                    }))
                                }.await;
                                client.send(api::answer(id, result));
                                if applied_successfully {
                                    crate::wallpaper_cache::cleanup(cache, store, desktop).await;
                                }
                            });
                            return;
                        }
                    }
                }
            }
            Request::Remote {
                layout_id,
                layout_revision,
                monitor,
                command,
            } => {
                let state = self.snapshot();
                if state.layout_id.as_deref() != Some(&layout_id)
                    || state.layout_revision != layout_revision
                    || state.suspended
                {
                    Err("The display layout changed; reload before using the remote.".into())
                } else if let Err(error) = self.world.vcp_target(&monitor) {
                    Err(error)
                } else if let Some(remote) = self.remote.clone() {
                    match remote.try_lock_owned() {
                        Err(_) => Err(
                            "A remote command is already running. Try again when it finishes."
                                .into(),
                        ),
                        Ok(mut remote) => {
                            tokio::spawn(async move {
                                let result = remote.execute(monitor, command).await.and_then(|s| {
                                    serde_json::to_value(s).map_err(|e| e.to_string())
                                });
                                client.send(api::answer(id, result));
                            });
                            return;
                        }
                    }
                } else {
                    Err("This agent drives no network remote.".into())
                }
            }
            Request::Vcp {
                layout_id,
                layout_revision,
                monitor,
                write,
            } => {
                let state = self.snapshot();
                if state.layout_id.as_deref() != Some(&layout_id)
                    || state.layout_revision != layout_revision
                    || state.suspended
                {
                    Err("The displays changed or are suspended; reload before controlling the monitor.".into())
                } else {
                    match self.world.vcp_target(&monitor) {
                        Err(error) => Err(error),
                        Ok(target) => match self.vcp.clone().try_lock_owned() {
                            Err(_) => Err(
                                "A monitor request is already running. Try again when it finishes."
                                    .into(),
                            ),
                            Ok(mut service) => {
                                tokio::spawn(async move {
                                    let result = match service.as_mut() {
                                        Some(service) => service.execute(target, write).await,
                                        None => {
                                            Err("This agent offers no monitor control.".to_owned())
                                        }
                                    }
                                    .and_then(|state| {
                                        serde_json::to_value(state).map_err(|e| e.to_string())
                                    });
                                    client.send(api::answer(id, result));
                                });
                                return;
                            }
                        },
                    }
                }
            }
            Request::DisplayOutputs => self
                .world
                .display_outputs()
                .and_then(|outputs| serde_json::to_value(outputs).map_err(|e| e.to_string())),
            Request::ChangeOutput {
                layout_id,
                layout_revision,
                signature,
                connector,
                change,
            } => {
                let state = self.snapshot();
                if state.layout_id.as_deref() != Some(&layout_id)
                    || state.layout_revision != layout_revision
                {
                    Err("the displays changed; reload the layout before changing an output".into())
                } else if state.enabled != Some(false)
                    || state.previewing
                    || state.suspended
                    || matches!(
                        self.reconciler.engine(),
                        EngineState::Running | EngineState::Paused
                    )
                {
                    Err("stop the engine and end the preview before changing an output".into())
                } else if self.world.layout().is_none_or(|l| l.is_virtual) {
                    Err("a foreign layout cannot change system outputs".into())
                } else {
                    let result = self.world.change_output(&connector, change, &signature);
                    // Even a failed tool may have partially changed the desktop.
                    let _ = self.inputs.send(Input::DisplayChanged);
                    result.and_then(|outputs| {
                        serde_json::to_value(outputs).map_err(|e| e.to_string())
                    })
                }
            }
            Request::Subscribe => {
                let snapshot = serde_json::to_value(self.snapshot()).map_err(|e| e.to_string());
                self.subscribers.push(client.clone());
                snapshot
            }
            // "Apply and start" sends the edit along: it becomes the current layout first.
            Request::Start {
                keep_layout,
                layout_id,
                document,
            } => match document {
                Some(document) => self.edit(layout_id.as_deref(), &document).map(|()| {
                    // "Apply and start" saves the edit too, so the screens may have moved.
                    self.handle(Input::UserStart { keep_layout: true });
                    self.repaint_desktop();
                }),
                None => {
                    self.handle(Input::UserStart { keep_layout });
                    Ok(())
                }
            }
            .map(|()| serde_json::Value::Null),
            // The agent is the only writer: the frontend sends what it would have saved.
            Request::SaveLayout {
                layout_id,
                document,
            } => self
                .edit(Some(&layout_id), &document)
                .and_then(|()| self.world.save_layout().map_err(|e| e.to_string()))
                .map(|()| {
                    // The screens moved: a span cut for where they were is wrong now.
                    // C# re-sliced on Saved for the same reason.
                    self.repaint_desktop();
                    serde_json::Value::Null
                }),
            Request::ApplyTopology {
                layout_id,
                layout_revision,
                document,
                adjust_scale,
                dry_run,
            } => {
                use lbm_display::linux::topology::How;
                let how = if dry_run { How::DryRun } else { How::ForReal };
                let state = self.snapshot();
                if self.world.topology_requires_stopped_engine()
                    && (state.layout_id.as_deref() != Some(&layout_id)
                        || layout_revision != Some(state.layout_revision))
                {
                    Err("the displays changed or the layout revision is missing; reload before applying Windows display positions".into())
                } else if self.world.topology_requires_stopped_engine()
                    && (state.enabled != Some(false)
                        || state.previewing
                        || state.suspended
                        || matches!(
                            self.reconciler.engine(),
                            EngineState::Running | EngineState::Paused
                        ))
                {
                    Err("stop the engine and end the preview before applying Windows display positions".into())
                } else {
                    let result = self
                        .world
                        .apply_topology(&layout_id, &document, adjust_scale, how)
                        .map(|commands| {
                            for line in &commands {
                                eprintln!(
                                    "[lbm-agent] {} {line}",
                                    if dry_run { "would run:" } else { "ran:" }
                                );
                            }
                            // Self-describing: the answer says which it was, so a frontend
                            // does not have to remember what it asked for to label it.
                            serde_json::json!({ "Commands": commands, "DryRun": dry_run })
                        });
                    // A driver may partially apply before reporting failure. Dry runs must
                    // never rebuild: that would discard an edit for a change not performed.
                    if !dry_run {
                        let _ = self.inputs.send(Input::DisplayChanged);
                    }
                    result
                }
            }
            Request::Preview {
                layout_id,
                document,
            } => self.world.set_preview(&layout_id, &document).map(|()| {
                // Whoever asked now owns it, and it ends when their connection does.
                self.previewing_for = Some(client.clone());
                self.handle(Input::Preview);
                serde_json::Value::Null
            }),
            Request::EndPreview => {
                self.previewing_for = None;
                self.handle(Input::EndPreview);
                Ok(serde_json::Value::Null)
            }
            Request::SaveOptions {
                options,
                excluded,
                load_at_startup,
            } => {
                let saved = self.world.save_options(
                    options.as_deref(),
                    excluded.as_deref(),
                    load_at_startup,
                );
                // Told whatever the answer: a session autostart that could not be set
                // up refuses the request, yet the other options were applied and stored
                // — and the hook must run on what the store says. Each one is sent only
                // if it changed, so a request refused outright sends nothing.
                self.tell_shortcut();
                self.tell_binding();
                self.tell_excluded();
                self.reload_changed_zones();
                saved.map(|()| serde_json::Value::Null)
            }
            Request::SaveWallpaper {
                layout_id,
                settings,
            } => self
                .world
                .save_wallpaper(&layout_id, *settings)
                .map(|screens| {
                    self.paint_desktop(screens);
                    serde_json::Value::Null
                }),
            Request::Stop => {
                self.handle(Input::UserStop);
                Ok(serde_json::Value::Null)
            }
            Request::Refresh => {
                self.handle(Input::Refresh);
                Ok(serde_json::Value::Null)
            }
            Request::Quit => {
                // The tray's Quit (C#: QuitAsync): the hook leaves, then the agent.
                eprintln!("[lbm-agent] -> Quit");
                self.hook.send(protocol::frame(&[Command::Quit]));
                self.quitting = true;
                Ok(serde_json::Value::Null)
            }
            // A command, not a question: the report comes back as a Probed event.
            //
            // Answered here rather than by the hook: the report is a pure function of
            // the zones, which the agent holds — so it can be asked for a layout no
            // hook has, or has yet, which is exactly the foreign-layout case the
            // editor uses it for.
            Request::Probe => match self.probe() {
                Some(sweep) => {
                    report = Some(sweep);
                    Ok(serde_json::Value::Null)
                }
                None => Err("no layout to probe".to_owned()),
            },
            Request::ProbeLayout {
                layout_id,
                layout_revision,
                document,
            } => {
                if layout_revision != self.reconciler.rebuild_count() {
                    Err("the displays changed; refresh the layout before probing".into())
                } else {
                    self.world
                        .probe_zones(&layout_id, &document)
                        .and_then(|xml| crate::probe::report(layout_id, layout_revision, &xml))
                        .and_then(|report| serde_json::to_value(report).map_err(|e| e.to_string()))
                }
            }
            Request::SeenProcesses => Ok(serde_json::json!(self.seen.list())),
            Request::Peers => return self.ask_peers(id, client, PeerRequest::State),
            Request::SetMultiPc {
                enabled,
                port,
                name,
            } => {
                return self.ask_peers(
                    id,
                    client,
                    PeerRequest::Set {
                        enabled,
                        port,
                        name,
                    },
                )
            }
            Request::PeerConnect { address } => {
                return self.ask_peers(id, client, PeerRequest::Connect(address))
            }
            Request::PeerAccept { machine_id } => {
                return self.ask_peers(id, client, PeerRequest::Accept(machine_id))
            }
            Request::PeerRefuse { machine_id } => {
                return self.ask_peers(id, client, PeerRequest::Refuse(machine_id))
            }
            Request::PeerForget { machine_id } => {
                return self.ask_peers(id, client, PeerRequest::Forget(machine_id))
            }
            Request::PeerWallpapers { machine_id } => {
                return self.ask_peers(id, client, PeerRequest::Wallpapers(machine_id))
            }
            Request::SetActiveSource {
                layout_id,
                monitor,
                source,
            } => self
                .world
                .set_active_source(&layout_id, &monitor, &source)
                .map(|switch| {
                    eprintln!("[lbm-agent] {monitor} now shows {source}");
                    // A panel wired to both machines on two inputs is switched too, away
                    // from the loop: the answer never waits on a monitor. Without a known
                    // input value only the routing follows the choice, which is all a
                    // hardware KVM needs.
                    if let Some(switch) = switch {
                        self.switch_input(switch);
                    }
                    // The zone of the panel has changed hands: a running engine must
                    // route it to the machine now shown.
                    self.reload_changed_zones();
                    serde_json::Value::Null
                }),
        };
        client.send(api::answer(id, result));
        if let Some(report) = report {
            self.forward("Probed", &report);
        }
    }

    /// A multi-PC request. What needs the network is answered from a task of its own,
    /// as the TV remote's and the VCP's are: the loop never waits on another machine.
    fn ask_peers(&mut self, id: u64, client: api::Client, request: PeerRequest) {
        let turning_on = matches!(request, PeerRequest::Set { enabled: true, .. });
        let Some(peers) = &mut self.peers else {
            client.send(api::answer(
                id,
                Err("this agent does not offer the multi-PC mode".into()),
            ));
            return;
        };
        match peers.request(request) {
            Answer::Now(result) => {
                client.send(api::answer(id, result));
            }
            Answer::Later(result) => {
                tokio::spawn(async move {
                    client.send(api::answer(id, result.await));
                });
            }
        }
        // A manager just started has no screens to tell its masters yet.
        if turning_on {
            self.tell_peers_the_displays();
        }
    }

    /// This machine's screens to the multi-PC manager, while the mode is on: they are
    /// what a master is told this remote shows. Read again from the system only then,
    /// so that an agent with the mode off pays nothing for it.
    fn tell_peers_the_displays(&mut self) {
        if !self.peers.as_ref().is_some_and(|p| p.enabled()) {
            return;
        }
        let mut displays = self.world.peer_displays();
        for display in &mut displays {
            display.input_source = self
                .world
                .display_monitor(&display.id)
                .and_then(|monitor| self.readings.get(&monitor))
                .copied();
        }
        if let Some(peers) = &mut self.peers {
            peers.local_displays(displays);
        }
    }

    /// Reads the input of the panels this machine shows (`input_switch::reads`), away
    /// from the loop. Asked after a rebuild — a display event — and when a master links;
    /// never on a timer.
    fn read_inputs(&mut self) {
        if !self.peers.as_ref().is_some_and(|p| p.enabled()) {
            return;
        }
        let reads = self.world.input_reads(self.has_master);
        if reads.is_empty() {
            if !self.readings.is_empty() {
                self.readings.clear();
                self.tell_peers_the_displays();
            }
            return;
        }
        let rebuild = self.reconciler.rebuild_count();
        let (ddc, bus, events) = (self.ddc.clone(), self.vcp.clone(), self.vcp_events.clone());
        tokio::spawn(async move {
            // One request at a time on the monitors, the property panel's included.
            let _bus = bus.lock_owned().await;
            let mut results = Vec::with_capacity(reads.len());
            for read in reads {
                let result = ddc.read(read.target.clone(), crate::vcp::INPUT).await;
                results.push((read, result));
            }
            let _ = events.send(VcpEvent::Read { rebuild, results });
        });
    }

    /// What the reads found: remembered on the sources shown, and told to the masters.
    fn inputs_read(&mut self, rebuild: u64, results: Vec<(InputRead, Result<u16, String>)>) {
        // A newer rebuild has read, or is reading, the panels as they are now.
        if rebuild != self.reconciler.rebuild_count() {
            return;
        }
        let mut learnt = Vec::new();
        let mut readings = std::collections::HashMap::new();
        for (read, result) in results {
            match result {
                Ok(value) => {
                    self.read_failures.remove(&read.monitor);
                    readings.insert(read.monitor.clone(), value);
                    learnt.push((read, value));
                }
                // A monitor without DDC/CI, behind a KVM, or a machine without ddcutil:
                // its input stays unknown, which only means it is never switched.
                Err(error) => {
                    if self.read_failures.get(&read.monitor) != Some(&error) {
                        eprintln!(
                            "[lbm-agent] the input of {} could not be read: {error}",
                            read.monitor
                        );
                        self.read_failures.insert(read.monitor.clone(), error);
                    }
                }
            }
        }
        for (read, value) in &learnt {
            if self.readings.get(&read.monitor) != Some(value) {
                eprintln!("[lbm-agent] {} shows input {value:#04x}", read.monitor);
            }
        }
        self.world.learn_inputs(&learnt);
        if readings != self.readings {
            self.readings = readings;
            self.tell_peers_the_displays();
        }
    }

    /// Switches a panel's input for the user's choice, away from the loop.
    fn switch_input(&mut self, switch: InputSwitch) {
        eprintln!(
            "[lbm-agent] switching {} to input {:#04x}",
            switch.monitor, switch.value
        );
        let (ddc, bus, events) = (self.ddc.clone(), self.vcp.clone(), self.vcp_events.clone());
        tokio::spawn(async move {
            let mut service = bus.lock_owned().await;
            let outcome = input_switch::execute(&*ddc, &switch).await;
            // Whatever the property panel read of that monitor may be wrong now.
            if let Some(service) = service.as_mut() {
                service.forget();
            }
            let _ = events.send(VcpEvent::Switched {
                switch: Box::new(switch),
                outcome,
            });
        });
    }

    fn input_switched(&mut self, switch: InputSwitch, outcome: Outcome) {
        self.input_failures.switched(&switch, &outcome);
        let (monitor, value) = (&switch.monitor, switch.value);
        match outcome {
            Outcome::AlreadyShown => {
                eprintln!("[lbm-agent] {monitor} already shows input {value:#04x}")
            }
            Outcome::Written => eprintln!("[lbm-agent] {monitor} switched to input {value:#04x}"),
            Outcome::Ask(ask, why) => {
                eprintln!(
                    "[lbm-agent] {monitor} cannot be switched from here ({why}): asking {}",
                    ask.machine_id
                );
                let sent = self
                    .peers
                    .as_mut()
                    .is_some_and(|p| p.select_input(&ask.machine_id, &ask.display, value));
                if !sent {
                    self.switch_given_up(&switch, &format!("{why}; the multi-PC mode is off"));
                }
            }
            Outcome::Failed(why) => self.switch_given_up(&switch, &why),
        }
    }

    /// Nobody could switch the panel: said, and a choice that only the switch could
    /// have made good is given up.
    fn switch_given_up(&mut self, switch: &InputSwitch, why: &str) {
        eprintln!(
            "[lbm-agent] {} could not be switched to input {:#04x}: {why}",
            switch.monitor, switch.value
        );
        self.input_failures.failed(switch, why);
        self.abandon(switch);
    }

    /// A choice that only the switch could have made good is given up.
    fn abandon(&mut self, switch: &InputSwitch) {
        if self.world.switch_failed(&switch.monitor, &switch.source) {
            self.reload_changed_zones();
        }
    }

    /// This machine's master asks for one of its monitors to be switched.
    fn select_input(&mut self, from: String, display: String, value: u16) {
        let target = match self.world.select_input_target(&display) {
            Ok(target) => target,
            Err(error) => {
                eprintln!("[lbm-agent] {from} asked to switch {display}: {error}");
                self.tell_master_switched(&from, &display, value, Some(error));
                return;
            }
        };
        let (ddc, bus, events) = (self.ddc.clone(), self.vcp.clone(), self.vcp_events.clone());
        tokio::spawn(async move {
            let mut service = bus.lock_owned().await;
            let result = input_switch::switch_locally(&*ddc, target, value).await;
            if let Some(service) = service.as_mut() {
                service.forget();
            }
            let _ = events.send(VcpEvent::Selected {
                from,
                display,
                value,
                result,
            });
        });
    }

    /// How a switch this machine's master asked for went: said, and told to the master,
    /// whose user is the one waiting on it.
    fn selected(
        &mut self,
        from: String,
        display: String,
        value: u16,
        result: Result<bool, String>,
    ) {
        let error = match result {
            Ok(true) => {
                eprintln!("[lbm-agent] {display} switched to input {value:#04x}");
                None
            }
            Ok(false) => {
                eprintln!("[lbm-agent] {display} already shows input {value:#04x}");
                None
            }
            Err(error) => {
                eprintln!(
                    "[lbm-agent] {display} could not be switched to input {value:#04x}: {error}"
                );
                Some(error)
            }
        };
        self.tell_master_switched(&from, &display, value, error);
    }

    fn tell_master_switched(
        &mut self,
        from: &str,
        display: &str,
        value: u16,
        error: Option<String>,
    ) {
        if let Some(peers) = &mut self.peers {
            peers.input_switched(from, display, value, error);
        }
    }

    /// What a remote of this machine answered about a switch this machine asked of it.
    fn remote_switched(
        &mut self,
        from: String,
        display: String,
        value: u16,
        error: Option<String>,
    ) {
        let failed = error.clone();
        match self.input_failures.answered(&from, &display, value, error) {
            Some(switch) => {
                eprintln!(
                    "[lbm-agent] {} could not be switched to input {value:#04x}: {from} could not either: {}",
                    switch.monitor,
                    failed.unwrap_or_default()
                );
                self.abandon(&switch);
            }
            None if failed.is_none() => {
                eprintln!("[lbm-agent] {from} switched {display} to input {value:#04x}")
            }
            None => {}
        }
    }

    /// What the multi-PC manager reports changed: a machine paired, linked, left, or —
    /// for a remote of this machine — reported its screens.
    ///
    /// The screens of the remotes of this machine are part of its layout: a remote
    /// linking, leaving or changing its screens is a display change, and goes the way
    /// of one — the debounce and the settle loop, then the rebuild, the profile of the
    /// new set of monitors, and the hook handed the result if the user wants it hooked.
    /// The same path as a local change, so that a link flapping is absorbed as a
    /// compositor's burst of notifications is.
    fn peers_changed(&mut self, state: crate::peers::PeersState) {
        let event = api::peers_event(&state);
        self.subscribers.retain(|s| s.send(event.clone()));
        let has_master = state.enabled
            && state.peers.iter().any(|p| {
                p.state == PeerState::Paired
                    && p.role == lbm_net::wire::Role::Master
                    && p.peer.connected
            });
        let master_linked = has_master && !self.has_master;
        self.has_master = has_master;
        self.selection_seen = Some((
            state.selection,
            state.selected_remote.clone().filter(|_| state.enabled),
        ));
        if self.world.set_remote_screens(state.remote_screens()) {
            eprintln!("[lbm-agent] the remote machines' screens changed");
            self.handle(Input::DisplayChanged);
        }
        self.tell_peers_the_hook_zones();
        // A master that links needs what selects this machine on its panels, which a
        // machine without a master does not read — and what its screens show.
        if master_linked {
            self.read_inputs();
            self.read_wallpapers();
        }
        self.watch_wallpapers();
    }

    /// Holds a watch on the desktop's wallpapers exactly while a master is linked: the
    /// only one who would see a change. Taken down with the last master, and with the
    /// mode — `has_master` is false then.
    fn watch_wallpapers(&mut self) {
        if !self.has_master {
            if self.wallpaper_watch.take().is_some() {
                eprintln!(
                    "[lbm-agent] multi-PC: no master left: the wallpapers are no longer watched"
                );
            }
        } else if self.wallpaper_watch.is_none() {
            self.wallpaper_watch = self.desktop.watch(self.wallpaper_changes.clone());
            if self.wallpaper_watch.is_some() {
                eprintln!("[lbm-agent] multi-PC: watching the wallpapers for the master");
            }
        }
    }

    /// Whether zones leading to `hosts` may be handed to the hook now: only if they lead to
    /// no machine but the selected remote, as the agent last heard of it. The layout is
    /// rebuilt for a new choice only after the display-change debounce; a `Load` of the
    /// layout built for the previous one (a user's Start, the watchdog) meanwhile would
    /// put that remote's zones back under a connection relaying to another. The rebuild
    /// that is on its way sends the right ones.
    fn zones_fit(&self, hosts: &std::collections::BTreeSet<String>) -> bool {
        let selected = self
            .selection_seen
            .as_ref()
            .and_then(|(_, selected)| selected.as_ref());
        hosts.iter().all(|host| Some(host) == selected)
    }

    /// Tells the multi-PC manager whether the hook's zones are fit for the selected remote
    /// (`peers::Service::hook_ready`): every zone it holds and every one on its way leads
    /// to no other machine. Until then the manager keeps no connection to the hook's input
    /// endpoint, and the borders to the other machines are walls.
    fn tell_peers_the_hook_zones(&mut self) {
        let ready = self.selection_seen.as_ref().and_then(|(selection, _)| {
            let fit = self.hook_holds.as_ref().is_some_and(|h| self.zones_fit(h))
                && self.loads_in_flight.iter().all(|h| self.zones_fit(h));
            fit.then_some(*selection)
        });
        if self.told_hook_ready == Some(ready) {
            return;
        }
        if let Some(peers) = &mut self.peers {
            peers.hook_ready(ready);
            self.told_hook_ready = Some(ready);
        }
    }

    /// The zones the hook holds are unknown from now on (a new connection, or none).
    fn hook_zones_unknown(&mut self) {
        self.hook_holds = None;
        self.loads_in_flight.clear();
        self.tell_peers_the_hook_zones();
    }

    /// Reads what this machine's screens show and hands it, drawn small, to the masters
    /// it is a remote of (docs/multi-pc.md, "Wallpapers"). Away from the loop: the
    /// desktop can take its time and the drawing is work. Asked after a rebuild — a
    /// display event —, when a master links, and when the desktop says its wallpapers
    /// changed (`wallpaper_watch`); never on a timer, so on a desktop that cannot say, a
    /// wallpaper changed with no display event reaches the master at the next one.
    ///
    /// Only while a master is linked: a machine nobody watches reads nothing.
    fn read_wallpapers(&mut self) {
        if !self.has_master {
            return;
        }
        let Some(sink) = self.peers.as_ref().and_then(|p| p.wallpaper_sink()) else {
            return;
        };
        let displays = self.world.peer_displays();
        let desktop = self.desktop.clone();
        use std::sync::atomic::Ordering;
        let generation = self.wallpaper_reads.clone();
        let expected = generation.fetch_add(1, Ordering::SeqCst) + 1;
        tokio::spawn(async move {
            let shown = match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                desktop.shown(),
            )
            .await
            {
                Ok(Ok(shown)) => shown,
                Ok(Err(error)) => {
                    eprintln!("[lbm-agent] multi-PC: the wallpapers cannot be read: {error}");
                    return;
                }
                Err(_) => {
                    eprintln!("[lbm-agent] multi-PC: the desktop did not say what it shows");
                    return;
                }
            };
            let drawn = tokio::task::spawn_blocking(move || {
                crate::peers::wallpapers::thumbnails(&displays, &shown)
            })
            .await;
            // A newer read is under way, or done: this one is about screens that were.
            if generation.load(Ordering::SeqCst) != expected {
                return;
            }
            match drawn {
                Ok(wallpapers) => sink.send(wallpapers),
                Err(error) => eprintln!("[lbm-agent] multi-PC: drawing the wallpapers: {error}"),
            }
        });
    }

    /// A frontend's connection ended.
    ///
    /// **A live preview belongs to the connection that asked for it.** It is a `Load`
    /// *and* a `Run`, so a frontend that goes away without ending it — killed, crashed,
    /// its machine asleep — would otherwise leave the engine driving an arrangement
    /// nobody saved and nothing on screen to stop it. A clean exit sends `EndPreview` and
    /// this finds nothing left to do; this is for every other way of leaving.
    ///
    /// Only the owner's departure counts. Another frontend closing its window, or the
    /// tray reconnecting, must not end somebody else's preview.
    fn left(&mut self, client: &api::Client) {
        self.subscribers.retain(|s| !s.is(client));
        if self.previewing_for.as_ref().is_some_and(|o| o.is(client)) {
            eprintln!("[lbm-agent] the frontend previewing has gone: ending the preview");
            self.previewing_for = None;
            self.handle(Input::EndPreview);
        }
    }

    /// Applies a frontend's edit to the current layout.
    fn edit(
        &mut self,
        layout_id: Option<&str>,
        document: &lbm_store::LayoutDocument,
    ) -> Result<(), String> {
        let layout_id = layout_id.ok_or("a document comes with the LayoutId it edits")?;
        self.world.edit(layout_id, document)
    }

    /// Saved settings are rendered in a worker, including after layout Save/rebuild.
    fn repaint_desktop(&mut self) {
        let Some(job) = self.world.saved_wallpaper_job() else {
            let screens = self.world.wallpaper();
            self.paint_desktop(screens);
            return;
        };
        use std::sync::atomic::Ordering;
        let generation = self.wallpaper_generation.clone();
        let expected = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let lock = self.wallpaper_lock.clone();
        let desktop = self.desktop.clone();
        tokio::spawn(async move {
            let _guard = lock.lock().await;
            if generation.load(Ordering::SeqCst) != expected {
                return;
            }
            let cache = job.images.clone();
            let store = job.store.clone();
            let prepared = tokio::task::spawn_blocking(move || {
                crate::wallpaper_job::screens(&job.layout, &job.settings, &job.images)
            })
            .await;
            match prepared {
                Ok(Ok(screens)) if generation.load(Ordering::SeqCst) == expected => {
                    if desktop.apply(screens).await.is_ok() {
                        crate::wallpaper_cache::cleanup(cache, store, desktop).await;
                    }
                }
                Ok(Err(error)) => eprintln!("[lbm-agent] wallpaper: {error}"),
                Err(error) => eprintln!("[lbm-agent] wallpaper worker: {error}"),
                _ => {}
            }
        });
    }

    /// Serialize background repainting with editor applications; obsolete work is skipped.
    ///
    /// The call reaches the desktop environment, which can take its time (or hang), so it
    /// goes to a task of its own: nothing the agent does may wait on a wallpaper. Nothing
    /// waits for the answer either — a desktop that refused is a background that stayed
    /// as it was, and the next apply tries again.
    fn paint_desktop(&mut self, screens: Vec<crate::desktop::ScreenWallpaper>) {
        let generation = self.wallpaper_generation.clone();
        let expected = generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        if screens.is_empty() {
            return;
        }
        let pending = match crate::wallpaper_cache::pin(&screens) {
            Ok(pending) => pending,
            Err(e) => {
                eprintln!("[lbm-agent] wallpaper: {e}");
                return;
            }
        };
        let lock = self.wallpaper_lock.clone();
        let desktop = self.desktop.clone();
        tokio::spawn(async move {
            let _pending = pending;
            let _guard = lock.lock().await;
            if generation.load(std::sync::atomic::Ordering::SeqCst) != expected {
                return;
            }
            let _ = desktop.apply(screens).await;
        });
    }

    /// Tells the hook whether it belongs to this agent, when that is known and is not
    /// what it was last told.
    ///
    /// Sent on every connection rather than carried by the layout: a hook this agent
    /// has just taken over from another one holds whatever that other one asked for,
    /// and a hook that outlives an agent (D5) must not inherit a binding to a process
    /// that is gone. The cache is cleared on connecting, so "not what it was told" is
    /// always true for a hook that has not been told yet.
    /// Hands the hook the list it stands aside for, when it is known and is not what it
    /// was last handed.
    ///
    /// The decision stays in the hook: it is the only one that can ask who is in front
    /// at the instant it is about to grab, and asking then is what fixed #541. What
    /// moves here is only the list, which the agent owns because the agent is what
    /// writes `Excluded.txt` and what the user edits it through.
    ///
    /// Sent before any layout, so the first `Run` of a connection is decided against
    /// the user's list and not against an empty one.
    /// Some options travel inside the layout the hook holds (the touchscreen ones), and
    /// so does which machine a shared panel shows: a running engine only gets them with
    /// a new Load. Nothing is sent when the zones did not change, and a stopped engine
    /// picks them up at its next Start.
    fn reload_changed_zones(&mut self) {
        if !matches!(
            self.reconciler.engine(),
            EngineState::Running | EngineState::Paused
        ) {
            return;
        }
        let changed = self
            .world
            .zones()
            .is_some_and(|(zones, _)| self.on_the_wire.as_ref() != Some(&zones));
        if changed {
            self.apply(Effect::Preview);
        }
    }

    fn tell_excluded(&mut self) {
        let Some(excluded) = self.world.excluded() else {
            return;
        };
        if self.excluded.as_ref() == Some(&excluded) {
            return;
        }
        eprintln!("[lbm-agent] -> Excluded ({} entries)", excluded.len());
        self.hook.send(protocol::frame(&[Command::Excluded {
            processes: excluded.clone(),
        }]));
        self.excluded = Some(excluded);
    }

    fn tell_binding(&mut self) {
        let Some(bound) = self.world.bound_to_agent() else {
            return;
        };
        if self.bound == Some(bound) {
            return;
        }
        eprintln!("[lbm-agent] -> BindToAgent({bound})");
        self.hook
            .send(protocol::frame(&[Command::BindToAgent { bound }]));
        self.bound = Some(bound);
    }

    fn tell_shortcut(&mut self) {
        if !cfg!(windows) {
            return;
        }
        let Some(shortcut) = self.world.rescue_shortcut() else {
            return;
        };
        if shortcut.trim().is_empty() || self.shortcut.as_ref() == Some(&shortcut) {
            return;
        }
        self.hook.send(protocol::frame(&[Command::Shortcut {
            text: shortcut.clone(),
        }]));
        self.shortcut = Some(shortcut);
        // The hook starts over on it, and says so only if it fails again.
        self.shortcut_unavailable = None;
    }

    /// Forwards what the hook said to the subscribers (see [`api`]).
    fn forward(&mut self, name: &str, payload: &str) {
        if self.subscribers.is_empty() {
            return;
        }
        let event = api::hook_event(name, payload);
        self.subscribers.retain(|s| s.send(event.clone()));
    }

    /// Launches a hook when none answers (D5); without one, the agent waits for a hook.
    pub fn with_launcher(mut self, launcher: HookLauncher) -> Self {
        self.launcher = Some(launcher);
        self
    }

    pub fn world(&self) -> &W {
        &self.world
    }

    /// Boots (the first layout), then handles inputs until `shutdown` completes or the
    /// hook connection and every input source are gone.
    pub async fn run(
        &mut self,
        mut signals: UnboundedReceiver<HookSignal>,
        mut inputs: UnboundedReceiver<Input>,
        shutdown: impl Future<Output = ()>,
    ) {
        // A previous run died with the outputs gapped: put them back before anything is
        // built on top (C#: LinuxDisplayController's constructor).
        if self.world.recover_stale() {
            eprintln!("[lbm-agent] restored the outputs a previous run left gapped");
            let _ = self.inputs.send(Input::DisplayChanged);
        }
        self.handle(Input::Boot);
        tokio::pin!(shutdown);
        let mut calls = self.calls.take();
        let mut sleep = self.sleep.take();
        let mut peer_notices = self.peers.as_mut().and_then(|p| p.take_notices());
        let mut vcp_events = self.vcp_events_rx.take();
        let mut wallpaper_changes = self.wallpaper_changes_rx.take();
        // The events of one wallpaper change, read once. No event, no deadline.
        let mut wallpaper_burst = crate::wallpaper_watch::Burst::default();
        loop {
            self.publish();
            if self.quitting {
                // The Quit must reach the hook before the process goes.
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), self.hook.flush())
                    .await;
                // The other machines are told rather than left to notice the silence.
                if let Some(peers) = &mut self.peers {
                    peers.stop().await;
                }
                return;
            }
            let input = tokio::select! {
                _ = &mut shutdown => return,
                call = async {
                    match &mut calls {
                        Some(calls) => calls.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match call {
                        Some(call) => self.call(call),
                        // The endpoint is gone: serve no more, but keep running.
                        None => calls = None,
                    }
                    continue;
                },
                signal = async {
                    match &mut sleep {
                        Some(sleep) => sleep.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match signal {
                        Some(SleepSignal::Starting(done)) => {
                            self.sleep_starting().await;
                            let _ = done.send(());
                        }
                        Some(SleepSignal::Ended) => {
                            eprintln!("[lbm-agent] the system woke up");
                            self.handle(Input::Hook(HookEvent::Resumed));
                        }
                        None => sleep = None,
                    }
                    continue;
                },
                notice = async {
                    match &mut peer_notices {
                        Some(notices) => notices.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match notice {
                        Some(Notice::Peers(state)) => self.peers_changed(state),
                        Some(Notice::SelectInput { from, display, value }) => {
                            self.select_input(from, display, value)
                        }
                        Some(Notice::InputSwitched { from, display, value, error }) => {
                            self.remote_switched(from, display, value, error)
                        }
                        None => peer_notices = None,
                    }
                    continue;
                },
                event = async {
                    match &mut vcp_events {
                        Some(events) => events.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match event {
                        Some(VcpEvent::Read { rebuild, results }) => {
                            self.inputs_read(rebuild, results)
                        }
                        Some(VcpEvent::Switched { switch, outcome }) => {
                            self.input_switched(*switch, outcome)
                        }
                        Some(VcpEvent::Selected { from, display, value, result }) => {
                            self.selected(from, display, value, result)
                        }
                        None => vcp_events = None,
                    }
                    continue;
                },
                change = async {
                    match &mut wallpaper_changes {
                        Some(changes) => changes.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match change {
                        // What a watch said before it was let go of is nobody's any more.
                        Some(()) if self.wallpaper_watch.is_some() => {
                            wallpaper_burst.event(tokio::time::Instant::now())
                        }
                        Some(()) => {}
                        None => wallpaper_changes = None,
                    }
                    continue;
                },
                _ = async {
                    match wallpaper_burst.due() {
                        Some(due) => tokio::time::sleep_until(due).await,
                        None => std::future::pending().await,
                    }
                } => {
                    wallpaper_burst.clear();
                    self.read_wallpapers();
                    continue;
                },
                signal = signals.recv() => match signal {
                    Some(HookSignal::Connected) => {
                        eprintln!("[lbm-agent] hook connected");
                        self.forward("Connected", "");
                        self.hook_connected = true;
                        // A hook repeats this to whoever starts listening to it.
                        self.shortcut_unavailable = None;
                        self.on_the_wire = None;
                        self.bound = None;
                        self.excluded = None;
                        self.hook_zones_unknown();
                        if let Some(launcher) = &mut self.launcher {
                            launcher.on_connected();
                        }
                        Input::Hook(HookEvent::Connected)
                    }
                    Some(HookSignal::Greeted { layout }) => {
                        eprintln!(
                            "[lbm-agent] hook greeting: layout {}",
                            if layout.is_empty() { "none" } else { &layout }
                        );
                        // Now that it has said it is one of ours, say what we want of
                        // it — before any layout, so a hook told to be bound is bound
                        // for the whole of this connection and not only once it runs.
                        self.tell_binding();
                        self.tell_excluded();
                        self.hook_greeted_with(&layout);
                        Input::Hook(HookEvent::Greeted(layout))
                    }
                    Some(HookSignal::Message(message)) => {
                        // C#: EventTrace.
                        eprintln!("[lbm-agent] hook: {}", message.name());
                        match &message {
                            Event::ShortcutUnavailable { shortcut } => {
                                self.shortcut_unavailable = Some(shortcut.clone());
                            }
                            Event::ShortcutAvailable { shortcut }
                                if self.shortcut_unavailable.as_ref() == Some(shortcut) =>
                            {
                                self.shortcut_unavailable = None;
                            }
                            // The answer to the oldest Load on its way; one this agent did
                            // not send leaves the hook holding what it cannot name.
                            Event::Loaded { .. } => {
                                self.hook_holds = self.loads_in_flight.pop_front();
                                self.tell_peers_the_hook_zones();
                            }
                            // A document the hook could not read: it keeps what it held.
                            Event::LoadFailed => {
                                if self.loads_in_flight.pop_front().is_none() {
                                    self.hook_holds = None;
                                }
                                self.tell_peers_the_hook_zones();
                            }
                            _ => {}
                        }
                        self.forward(message.name(), &message.payload());
                        Input::Hook(hook_event(message))
                    }
                    // C#: the client synthesizes Dead when the connection drops.
                    Some(HookSignal::Lost) => {
                        eprintln!("[lbm-agent] hook connection lost");
                        self.forward("Dead", "");
                        self.hook_connected = false;
                        self.shortcut_unavailable = None;
                        self.on_the_wire = None;
                        self.hook_zones_unknown();
                        Input::Hook(HookEvent::Dead)
                    }
                    Some(HookSignal::Unreachable) => {
                        self.unreachable();
                        continue;
                    }
                    None => return,
                },
                input = inputs.recv() => match input {
                    Some(input) => input,
                    None => return,
                },
            };
            self.handle(input);
        }
    }

    /// What a hook that just greeted this agent holds: nothing, the zones this agent would
    /// hand it now (the same document, by its fingerprint), or a layout it cannot name —
    /// one a previous life of the agent sent, perhaps with a remote's zones that are not
    /// the selected one's. That one is unknown, and fit for nothing until replaced.
    fn hook_greeted_with(&mut self, layout: &str) {
        self.hook_holds = if layout.is_empty() {
            Some(Default::default())
        } else {
            self.world
                .zones()
                .filter(|(zones, _)| protocol::fingerprint(zones) == layout)
                .map(|(zones, _)| zone_hosts(&zones))
        };
        self.tell_peers_the_hook_zones();
    }

    /// No hook answers: launch one, if this agent launches hooks.
    fn unreachable(&mut self) {
        let Some(launcher) = &mut self.launcher else {
            eprintln!("[lbm-agent] no hook answers");
            return;
        };
        match launcher.on_unreachable(std::time::Instant::now()) {
            Launch::Started(pid) => {
                eprintln!(
                    "[lbm-agent] started {} ({pid})",
                    launcher.program().display()
                )
            }
            Launch::Starting => eprintln!("[lbm-agent] the hook started is not answering yet"),
            Launch::AlreadyRunning => {
                eprintln!("[lbm-agent] a hook runs but does not answer at the endpoint")
            }
            Launch::Waiting => {}
            Launch::Failed(error) => eprintln!(
                "[lbm-agent] could not start {}: {error}",
                launcher.program().display()
            ),
        }
    }

    /// One input through the reconciler, then its effects.
    pub fn handle(&mut self, input: Input) {
        let rebuilds = self.reconciler.rebuild_count();
        let effects = self.reconciler.handle(input, &mut self.world);
        for effect in effects {
            self.apply(effect);
        }
        // Every rebuild is this machine's display change, decided by the reconciler:
        // the masters it is a remote of hear of it from here, with no watcher of its own.
        if self.reconciler.rebuild_count() != rebuilds {
            self.tell_peers_the_displays();
            // The panels' inputs as they are after this display change; what is read
            // only ever reaches the layout and the masters, never a monitor.
            self.read_inputs();
            self.read_wallpapers();
        }
    }

    /// Whether `zones` may be handed to the hook ([`Agent::zones_fit`]), and if so,
    /// counted as on their way until the hook answers.
    fn loading(&mut self, zones: &str) -> bool {
        let hosts = zone_hosts(zones);
        if !self.zones_fit(&hosts) {
            eprintln!(
                "[lbm-agent] the layout still leads to a machine no longer selected: \
                 the rebuild on its way is sent instead"
            );
            return false;
        }
        self.loads_in_flight.push_back(hosts);
        self.tell_peers_the_hook_zones();
        true
    }

    fn apply(&mut self, effect: Effect) {
        match effect {
            Effect::Start => {
                // The zones of now: never those of the moment the Start was decided.
                if let Some((zones, foreign)) = self.world.zones() {
                    // The topology prologue — never for a foreign layout, which must not
                    // move local outputs. When it moved them, these zones describe a
                    // desktop that is going away: drop the send, the display change
                    // rebuilds and starts again in the new geometry (C#: StartAsync).
                    if !foreign && self.world.prepare_for_engine() {
                        eprintln!("[lbm-agent] outputs moved for the engine: waiting for them");
                        let _ = self.inputs.send(Input::DisplayChanged);
                        return;
                    }
                    if !self.loading(&zones) {
                        return;
                    }
                    let load = Command::Load {
                        zones: zones.clone(),
                        desktop: self.world.desktop(),
                    };
                    // A foreign layout is simulated: loaded, never run.
                    let frame = if foreign {
                        protocol::frame(&[load])
                    } else {
                        protocol::frame(&[load, Command::Run])
                    };
                    eprintln!(
                        "[lbm-agent] -> {} ({} zones)",
                        if foreign { "Load" } else { "Load+Run" },
                        zones.matches("<Zone ").count()
                    );
                    self.hook.send(frame);
                    self.on_the_wire = Some(zones);
                }
            }
            Effect::Preview => {
                let Some((zones, false)) = self.world.zones() else {
                    return;
                };
                // C# `LiveLayoutUpdater`: the hook is never made to swap a layout for an
                // identical one — unless it is down, when the preview is what puts it up.
                if self.reconciler.engine() == EngineState::Running
                    && self.on_the_wire.as_ref() == Some(&zones)
                {
                    return;
                }
                if !self.loading(&zones) {
                    return;
                }
                eprintln!(
                    "[lbm-agent] -> Preview ({} zones)",
                    zones.matches("<Zone ").count()
                );
                self.hook.send(protocol::frame(&[
                    Command::Load {
                        zones: zones.clone(),
                        desktop: self.world.desktop(),
                    },
                    Command::Run,
                ]));
                self.on_the_wire = Some(zones);
            }
            Effect::Stop => {
                eprintln!("[lbm-agent] -> Stop");
                self.hook.send(protocol::frame(&[Command::Stop]));
                // Nobody to deliver it to: end the hook this agent launched instead (C#:
                // a lost IPC Stop falls back to stopping the process).
                if !self.hook_connected {
                    if let Some(launcher) = &mut self.launcher {
                        if launcher.stop_launched() {
                            eprintln!("[lbm-agent] the hook could not be told to stop: ended it");
                        }
                    }
                }
                // The epilogue: the outputs go back where they were (C#: StopAsync).
                if self.world.restore_after_engine() {
                    let _ = self.inputs.send(Input::DisplayChanged);
                }
            }
            Effect::SaveEnabled => {
                if let Err(error) = self.world.save_enabled() {
                    eprintln!("[lbm-agent] could not save Enabled: {error}");
                }
            }
            Effect::SaveLayout => {
                if let Err(error) = self.world.save_layout() {
                    eprintln!("[lbm-agent] could not save the layout: {error}");
                }
            }
            Effect::WakeAfter { wake, after } => {
                let inputs = self.inputs.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(after).await;
                    let _ = inputs.send(Input::Wake(wake));
                });
            }
            Effect::Wallpaper => {
                self.repaint_desktop();
            }
            Effect::ProcessSeen(process) => {
                if self.seen.add(&process) {
                    eprintln!("[lbm-agent] seen: {process}");
                }
            }
        }
    }
}

/// The other machines the zones of a document lead to (their `Host`), read as the hook
/// reads them. A document that does not read leads nowhere: the hook refuses it.
fn zone_hosts(zones: &str) -> std::collections::BTreeSet<String> {
    lbm_zones::ZonesLayout::from_xml(zones)
        .map(|layout| {
            layout
                .all_zones
                .iter()
                .filter_map(|&zone| layout.arena[zone].host.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// What the hook said about the shortcut, if it is about the one wanted now: a verdict on
/// a combination since replaced says nothing of this one. With no layout yet, the hook is
/// on its own and what it said stands. An empty option means the default, for the hook too.
fn unavailable_now(said: Option<&str>, current: Option<&str>) -> Option<String> {
    let said = said?;
    let current = current.map(|text| {
        if text.trim().is_empty() {
            lbm_ipc::shortcut::DEFAULT
        } else {
            text
        }
    });
    current
        .is_none_or(|current| current == said)
        .then(|| said.to_owned())
}

#[cfg(test)]
mod tests {
    use super::unavailable_now;

    #[test]
    fn a_verdict_only_speaks_for_the_shortcut_it_was_about() {
        let said = Some("Ctrl+Alt+F9");
        assert_eq!(unavailable_now(said, Some("Ctrl+Alt+F9")).as_deref(), said);
        assert_eq!(unavailable_now(said, Some("Ctrl+Alt+F8")), None);
        assert_eq!(unavailable_now(said, None).as_deref(), said);
        assert_eq!(unavailable_now(None, Some("Ctrl+Alt+F9")), None);
    }

    #[test]
    fn an_empty_option_is_the_default_shortcut() {
        let default = lbm_ipc::shortcut::DEFAULT;
        assert_eq!(
            unavailable_now(Some(default), Some("  ")).as_deref(),
            Some(default)
        );
    }
}
