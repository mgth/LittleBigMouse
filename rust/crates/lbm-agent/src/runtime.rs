//! The agent's event loop: every input — from the hook, the display poll, the timers
//! the reconciler asked for — goes through the reconciler, one at a time, and its
//! effects are carried out here, in order.

use std::future::Future;

use lbm_ipc::client::{self, DaemonEvent, DaemonMessage};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::hook::{HookClient, HookSignal};
use crate::reconcile::{Effect, HookEvent, Input, Reconciler, Timings};
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
        }
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
        loop {
            let input = tokio::select! {
                _ = &mut shutdown => return,
                signal = signals.recv() => match signal {
                    Some(HookSignal::Connected) => {
                        eprintln!("[lbm-agent] hook connected");
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
