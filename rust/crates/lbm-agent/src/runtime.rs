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
        }
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
            Request::Start { keep_layout } => {
                self.handle(Input::UserStart { keep_layout });
                Ok(serde_json::Value::Null)
            }
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
        };
        client.send(api::answer(id, result));
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
                signal = signals.recv() => match signal {
                    Some(HookSignal::Connected) => {
                        eprintln!("[lbm-agent] hook connected");
                        self.hook_connected = true;
                        if let Some(launcher) = &mut self.launcher {
                            launcher.on_connected();
                        }
                        Input::Hook(HookEvent::Connected)
                    }
                    Some(HookSignal::Message(message)) => {
                        // C#: DaemonEventTrace.
                        eprintln!("[lbm-agent] hook: {:?}", message.event);
                        Input::Hook(hook_event(message))
                    }
                    // C#: the client synthesizes Dead when the connection drops.
                    Some(HookSignal::Lost) => {
                        eprintln!("[lbm-agent] hook connection lost");
                        self.hook_connected = false;
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
                }
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
            Effect::ProcessSeen(process) => eprintln!("[lbm-agent] focused: {process}"),
        }
    }
}
