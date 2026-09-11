//! The agent's event loop: every input — from the hook, the display poll, the timers
//! the reconciler asked for — goes through the reconciler, one at a time, and its
//! effects are carried out here, in order.

use std::future::Future;

use lbm_ipc::client::{self, DaemonEvent, DaemonMessage};
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

/// A daemon event as the reconciler knows it.
fn hook_event(message: DaemonMessage) -> HookEvent {
    match message.event {
        DaemonEvent::Running => HookEvent::Running,
        DaemonEvent::Stopped => HookEvent::Stopped,
        DaemonEvent::Paused => HookEvent::Paused,
        DaemonEvent::Dead => HookEvent::Dead,
        DaemonEvent::SettingsChanged => HookEvent::SettingsChanged,
        DaemonEvent::DisplayChanged => HookEvent::DisplayChanged,
        DaemonEvent::DesktopChanged => HookEvent::DesktopChanged,
        DaemonEvent::FocusChanged => HookEvent::FocusChanged(message.payload),
        DaemonEvent::Suspended => HookEvent::Suspended,
        DaemonEvent::Resumed => HookEvent::Resumed,
        DaemonEvent::Loaded => HookEvent::Loaded,
        DaemonEvent::LoadFailed => HookEvent::LoadFailed,
        DaemonEvent::Probed => HookEvent::Probed,
        DaemonEvent::Rescued => HookEvent::Rescued,
        DaemonEvent::ShortcutUnavailable => HookEvent::ShortcutUnavailable,
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
            published: None,
            hook_connected: false,
            quitting: false,
            seen: api::SeenProcesses::default(),
            sleep: None,
            on_the_wire: None,
            shortcut: None,
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
        self.hook.send(client::messages(&[client::stop()]));
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
            previewing: self.reconciler.previewing(),
        }
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

    /// One frontend request.
    fn call(&mut self, call: Call) {
        let Call {
            id,
            request,
            client,
        } = call;
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
                Some(document) => self
                    .edit(layout_id.as_deref(), &document)
                    .map(|()| self.handle(Input::UserStart { keep_layout: true })),
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
                .map(|()| serde_json::Value::Null),
            Request::Preview {
                layout_id,
                document,
            } => self.world.set_preview(&layout_id, &document).map(|()| {
                self.handle(Input::Preview);
                serde_json::Value::Null
            }),
            Request::EndPreview => {
                self.handle(Input::EndPreview);
                Ok(serde_json::Value::Null)
            }
            Request::SaveOptions { options, excluded } => self
                .world
                .save_options(options.as_ref(), excluded.as_deref())
                .map(|()| {
                    self.tell_shortcut();
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
                self.hook.send(client::messages(&[client::quit()]));
                self.quitting = true;
                Ok(serde_json::Value::Null)
            }
            // A command, not a question: the report comes back as a Probed event.
            Request::Probe if self.hook_connected => {
                self.hook.send(client::messages(&[client::probe()]));
                Ok(serde_json::Value::Null)
            }
            Request::Probe => Err("no hook is connected".to_owned()),
            Request::SeenProcesses => Ok(serde_json::json!(self.seen.list())),
        };
        client.send(api::answer(id, result));
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
        self.hook
            .send(client::messages(&[client::shortcut(&shortcut)]));
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
                        if let Some(launcher) = &mut self.launcher {
                            launcher.on_connected();
                        }
                        Input::Hook(HookEvent::Connected)
                    }
                    Some(HookSignal::Message(message)) => {
                        // C#: DaemonEventTrace.
                        eprintln!("[lbm-agent] hook: {:?}", message.event);
                        self.forward(api::hook_event_name(message.event), &message.payload);
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
                    let load = client::load(&zones);
                    // A foreign layout is simulated: loaded, never run.
                    let frame = if foreign {
                        client::messages(&[load])
                    } else {
                        client::messages(&[load, client::run()])
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
                self.hook
                    .send(client::messages(&[client::load(&zones), client::run()]));
                self.on_the_wire = Some(zones);
            }
            Effect::Stop => {
                eprintln!("[lbm-agent] -> Stop");
                self.hook.send(client::messages(&[client::stop()]));
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
            Effect::ProcessSeen(process) => {
                if self.seen.add(&process) {
                    eprintln!("[lbm-agent] seen: {process}");
                }
            }
        }
    }
}
