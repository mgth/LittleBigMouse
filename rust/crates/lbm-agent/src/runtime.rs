//! The agent's event loop: every input — from the hook, the display poll, the timers
//! the reconciler asked for — goes through the reconciler, one at a time, and its
//! effects are carried out here, in order.

use std::future::Future;

use lbm_ipc::protocol::{self, Command, Event};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::api::{self, Call, Request, Snapshot};
use crate::hook::{HookClient, HookSignal};
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
        // The handshake never comes through here: the connection answers it and
        // reports what it learned as `HookSignal::Greeted`. An event this version does
        // not know is one it has nothing to do about.
        Event::Hello { .. } | Event::Unknown => HookEvent::Unknown,
    }
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
    /// Whether the hook was last told it belongs to this agent; `None` on a fresh
    /// connection, so a hook that just adopted us is always told.
    bound: Option<bool>,
    /// The exclusion list the hook was last handed; `None` on a fresh connection.
    excluded: Option<Vec<String>>,
    /// What the desktop was last told to show; `None`: unknown, so the next apply goes.
    wallpaper_on_screen: Option<String>,
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
            bound: None,
            excluded: None,
            wallpaper_on_screen: None,
        }
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
            enabled: layout.map(|l| l.enabled),
            saved: layout.map(|l| l.saved),
            load_at_startup: self.world.app_options().map(|(startup, _)| startup),
            hide_tray_icon: self.world.app_options().map(|(_, hidden)| hidden),
            previewing: self.reconciler.previewing(),
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
            })),
            Request::Snapshot => serde_json::to_value(self.snapshot()).map_err(|e| e.to_string()),
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
                    let screens = self.world.wallpaper();
                    self.paint_desktop(screens);
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
                    let screens = self.world.wallpaper();
                    self.paint_desktop(screens);
                    serde_json::Value::Null
                }),
            Request::ApplyTopology {
                layout_id,
                document,
                adjust_scale,
            } => self
                .world
                .apply_topology(&layout_id, &document, adjust_scale)
                .map(|()| {
                    // The topology changed under everyone: the layout has to be rebuilt
                    // from what the system now says, exactly as a hotplug would.
                    let _ = self.inputs.send(Input::DisplayChanged);
                    serde_json::Value::Null
                }),
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
            } => self
                .world
                .save_options(options.as_ref(), excluded.as_deref(), load_at_startup)
                .map(|()| {
                    self.tell_shortcut();
                    self.tell_binding();
                    self.tell_excluded();
                    serde_json::Value::Null
                }),
            Request::SaveWallpaper {
                layout_id,
                settings,
            } => self
                .world
                .save_wallpaper(&layout_id, *settings)
                .map(|screens| {
                    // An edit is the one apply that must not be skipped as "already
                    // shown": the user changed something and is looking at the screen.
                    self.wallpaper_on_screen = None;
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
            Request::SeenProcesses => Ok(serde_json::json!(self.seen.list())),
        };
        client.send(api::answer(id, result));
        if let Some(report) = report {
            self.forward("Probed", &report);
        }
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

    /// C# `SendShortcutAsync`, when the options change it: the hook registers the
    /// rescue shortcut at once and says if it cannot. Windows only, as in C#: nothing
    /// registers one elsewhere (and every Load carries it anyway).
    /// Show `screens`, unless the desktop is already showing exactly that.
    ///
    /// The call reaches the desktop environment, which can take its time (or hang), so it
    /// goes to a task of its own: nothing the agent does may wait on a wallpaper. Nothing
    /// waits for the answer either — a desktop that refused is a background that stayed
    /// as it was, and the next apply tries again.
    fn paint_desktop(&mut self, screens: Vec<crate::desktop::ScreenWallpaper>) {
        if screens.is_empty() {
            return;
        }
        let signature = crate::wallpaper::signature(&screens);
        if self.wallpaper_on_screen.as_deref() == Some(signature.as_str()) {
            return;
        }
        self.wallpaper_on_screen = Some(signature);
        tokio::spawn(async move {
            crate::desktop::apply(&screens).await;
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
        loop {
            self.publish();
            if self.quitting {
                // The Quit must reach the hook before the process goes.
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), self.hook.flush())
                    .await;
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
                signal = signals.recv() => match signal {
                    Some(HookSignal::Connected) => {
                        eprintln!("[lbm-agent] hook connected");
                        self.forward("Connected", "");
                        self.hook_connected = true;
                        self.on_the_wire = None;
                        self.bound = None;
                        self.excluded = None;
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
                        Input::Hook(HookEvent::Greeted(layout))
                    }
                    Some(HookSignal::Message(message)) => {
                        // C#: EventTrace.
                        eprintln!("[lbm-agent] hook: {}", message.name());
                        self.forward(message.name(), &message.payload());
                        Input::Hook(hook_event(message))
                    }
                    // C#: the client synthesizes Dead when the connection drops.
                    Some(HookSignal::Lost) => {
                        eprintln!("[lbm-agent] hook connection lost");
                        self.forward("Dead", "");
                        self.hook_connected = false;
                        self.on_the_wire = None;
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
        let effects = self.reconciler.handle(input, &mut self.world);
        for effect in effects {
            self.apply(effect);
        }
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
                let screens = self.world.wallpaper();
                self.paint_desktop(screens);
            }
            Effect::ProcessSeen(process) => {
                if self.seen.add(&process) {
                    eprintln!("[lbm-agent] seen: {process}");
                }
            }
        }
    }
}
