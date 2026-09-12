//! The reconciler: what the agent does about what it is told.
//!
//! It answers the two questions the C# UI answered in four places — *does this display
//! change deserve a layout rebuild?* (`DisplayChangeCoordinator`) and *should the mouse
//! engine be hooked right now, and is it?* (`EngineController`, and the routing of
//! daemon events in `MainService`) — as one state machine. The rules are the C# ones,
//! and the C# tests of those classes are its specification, the #607 fix included.
//!
//! It performs nothing itself. It reads and rebuilds the layout through [`World`], and
//! everything else it wants done comes out as [`Effect`]s, in order — including the
//! timers it needs, which come back as [`Input::Wake`]. The runtime owns the clock and
//! the I/O, the reconciler owns the decisions; tests drive it with a virtual clock.
//!
//! # One event at a time
//!
//! In C# several async paths each captured the zones of their own moment and raced to
//! the daemon, which could then hook a desktop that had just gone away (#607, which
//! `LatestRequestGate` fixes in #609). Here one input is handled to completion before
//! the next, and [`Effect::Start`] carries no zones: the runtime computes them from the
//! current layout when it sends. A stale Start cannot exist.

use std::time::Duration;

/// How long a display-change burst may take, and how hard the post-resume watchdog
/// insists: C#'s `DisplayChangeTimings` and `ResumeWatchdogTimings`, measured defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timings {
    /// Quiet window absorbing a burst of display notifications.
    pub debounce: Duration,
    /// Spacing between settle re-reads of the display signature.
    pub stability_step: Duration,
    /// Cap on the settle loop, so a flapping configuration cannot hang it.
    pub stability_max_steps: u32,
    /// Spacing between the watchdog's engine-state checks.
    pub watchdog_step: Duration,
    /// Consecutive Running checks that count as converged.
    pub watchdog_stable_steps: u32,
    /// Cap on re-Starts, so a paused (excluded application) engine cannot loop forever.
    pub watchdog_max_restarts: u32,
    /// Hard stop against a pathological flapping wake.
    pub watchdog_max_steps: u32,
}

impl Default for Timings {
    fn default() -> Self {
        Timings {
            debounce: Duration::from_millis(300),
            stability_step: Duration::from_millis(100),
            stability_max_steps: 8,
            watchdog_step: Duration::from_millis(500),
            watchdog_stable_steps: 3,
            watchdog_max_restarts: 6,
            watchdog_max_steps: 60,
        }
    }
}

/// What the reconciler needs to know of the current layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutState {
    /// The user's choice for this layout (`Options.Enabled`, stored per layout).
    pub enabled: bool,
    /// A foreign layout (an import, a file): simulated, never adopted.
    pub is_virtual: bool,
    /// Nothing edited since the last save.
    pub saved: bool,
}

/// The world the reconciler reads and changes synchronously.
pub trait World {
    /// A cheap fingerprint of the display configuration (C# `DisplaySignature`).
    fn display_signature(&mut self) -> String;

    /// Builds a fresh layout from the displays and the store (C# `UpdateLayout`).
    fn rebuild_layout(&mut self);

    /// The current layout; `None` before the first build.
    fn layout(&self) -> Option<LayoutState>;

    /// Records the user's choice in the current layout (`Options.Enabled = enabled`).
    fn set_enabled(&mut self, enabled: bool);

    /// Drops the layout being previewed, if any: what the hook is handed next is the
    /// current layout again.
    fn end_preview(&mut self) {}
}

/// What the hook reports, as far as reconciling goes (the wire events of
/// `lbm_ipc::client::DaemonEvent`, plus the client's own `Connected`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HookEvent {
    /// The connection to the hook was (re)established.
    Connected,
    Running,
    Stopped,
    Paused,
    Dead,
    SettingsChanged,
    DisplayChanged,
    DesktopChanged,
    /// The foreground process, for the exclusion editor.
    FocusChanged(String),
    Suspended,
    Resumed,
    Loaded,
    LoadFailed,
    Probed,
    Rescued,
    ShortcutUnavailable,
    /// The hook declined a Run (#609): the engine stays stopped, and Stopped is what
    /// the reconciler reasons on.
    RunRefused,
}

/// The hook's state as its events tell it (C#: `State`, the events up to `Dead`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EngineState {
    Running,
    Stopped,
    Paused,
    #[default]
    Dead,
}

/// A timer the reconciler asked for, coming back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// A step of the display-change flow of this generation.
    Display(u64),
    /// A step of the resume watchdog of this generation.
    Watchdog(u64),
}

/// What the reconciler is told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// The agent starts: build the first layout (C#: `MainService.UpdateLayout` at
    /// startup). Nothing is sent: the hook is handed the layout when it asks for one
    /// (`Connected`, then `Stopped`) — and a hook that survived the previous agent
    /// (D5) answers `Running` instead, so it keeps its layout and is not re-grabbed.
    Boot,
    /// The platform saw a display change (the hook's are [`HookEvent`]s).
    DisplayChanged,
    /// The hook reported something.
    Hook(HookEvent),
    /// The tray's Refresh: rebuild what the automatic detection missed (#443).
    Refresh,
    /// The user's Start. `keep_layout`: from the editor's "apply and start", which also
    /// keeps the geometry being edited; the tray has no editor behind it.
    UserStart { keep_layout: bool },
    /// The user's Stop.
    UserStop,
    /// A frontend's live preview: the world holds the layout being edited, the hook is
    /// handed it as it stands, nothing is recorded (C#: `LiveLayoutUpdater`).
    Preview,
    /// The preview is over: the hook goes back to the current layout.
    EndPreview,
    /// A timer the reconciler asked for.
    Wake(Wake),
}

/// What the reconciler wants done, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Hand the current layout to the hook and hook it: the topology prologue, then
    /// Load and Run — Load only for a virtual layout. The zones are computed at send.
    Start,
    /// Hand the layout being previewed to the hook: Load and Run, without the prologue
    /// and without persisting anything (C#: `SendLiveAsync`).
    Preview,
    /// Unhook.
    Stop,
    /// Persist the current layout's Enabled alone (C# `SaveEnabled`).
    SaveEnabled,
    /// Persist the whole current layout (C# `Save`).
    SaveLayout,
    /// Come back with `wake` after `after`.
    WakeAfter { wake: Wake, after: Duration },
    /// A process was seen in the foreground (for the exclusion editor).
    ProcessSeen(String),
    /// Put the desktop background back on the layout now held. Emitted with every
    /// rebuild: the span was cut for the desktop that just went away.
    Wallpaper,
}

/// Where the display-change flow stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DisplayFlow {
    Idle,
    /// Waiting out the burst.
    Debouncing,
    /// Re-reading until two consecutive signatures agree; `steps` re-reads done.
    Settling {
        signature: String,
        steps: u32,
    },
}

/// The resume watchdog of one generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Watchdog {
    active: bool,
    restarts: u32,
    quiet: u32,
    steps: u32,
}

/// The agent's decisions (see the module documentation).
#[derive(Clone, Debug)]
pub struct Reconciler {
    timings: Timings,
    /// The display is off: the hook unhooked itself, nothing is read or rebuilt.
    suspended: bool,
    engine: EngineState,
    /// Between a (re)connection and the first Running: a Stopped then means "I have no
    /// layout", not the user's Stop.
    just_connected: bool,
    display_generation: u64,
    display: DisplayFlow,
    /// Display flows after which a resume starts its watchdog: each C# `Resumed`
    /// handler awaits its own `NotifyAsync`, which returns when that flow is done or
    /// superseded, then starts a watchdog (a newer one superseding it).
    resume_after: Vec<u64>,
    last_built_signature: String,
    rebuild_count: u64,
    watchdog_generation: u64,
    watchdog: Watchdog,
    /// A frontend's edit is what the hook runs.
    previewing: bool,
}

impl Reconciler {
    pub fn new(timings: Timings) -> Self {
        Reconciler {
            timings,
            suspended: false,
            engine: EngineState::Dead,
            just_connected: false,
            display_generation: 0,
            display: DisplayFlow::Idle,
            resume_after: Vec::new(),
            last_built_signature: String::new(),
            rebuild_count: 0,
            watchdog_generation: 0,
            watchdog: Watchdog::default(),
            previewing: false,
        }
    }

    /// A frontend's live preview is what the hook runs.
    pub fn previewing(&self) -> bool {
        self.previewing
    }

    /// The display is off.
    pub fn suspended(&self) -> bool {
        self.suspended
    }

    /// The hook's state as its events tell it.
    pub fn engine(&self) -> EngineState {
        self.engine
    }

    /// The signature the current layout was built from (`""` before the first build).
    pub fn last_built_signature(&self) -> &str {
        &self.last_built_signature
    }

    /// Layout rebuilds so far.
    pub fn rebuild_count(&self) -> u64 {
        self.rebuild_count
    }

    /// The resume watchdog owns reconciliation.
    pub fn watchdog_active(&self) -> bool {
        self.watchdog.active
    }

    /// Handles one input to completion; returns what to do, in order.
    pub fn handle(&mut self, input: Input, world: &mut impl World) -> Vec<Effect> {
        let mut out = Vec::new();
        match input {
            Input::Boot => {
                self.rebuild(world, &mut out);
            }
            Input::DisplayChanged => {
                self.display_changed(&mut out);
            }
            Input::Hook(event) => self.hook_event(event, world, &mut out),
            Input::Refresh => self.refresh(world, &mut out),
            Input::UserStart { keep_layout } => self.user_start(keep_layout, world, &mut out),
            Input::UserStop => self.user_stop(world, &mut out),
            Input::Preview => self.preview(world, &mut out),
            Input::EndPreview => self.end_preview(world, &mut out),
            Input::Wake(Wake::Display(generation)) => {
                self.display_step(generation, world, &mut out)
            }
            Input::Wake(Wake::Watchdog(generation)) => {
                self.watchdog_step(generation, world, &mut out)
            }
        }
        out
    }

    //==================//
    // Hook events      //
    //==================//

    /// C# `MainService.DaemonEventReceivedAsync`: flags first (they gate what follows),
    /// then what the event means.
    fn hook_event(&mut self, event: HookEvent, world: &mut impl World, out: &mut Vec<Effect>) {
        match event {
            HookEvent::Running => self.engine = EngineState::Running,
            HookEvent::Stopped => self.engine = EngineState::Stopped,
            HookEvent::Paused => self.engine = EngineState::Paused,
            HookEvent::Dead => self.engine = EngineState::Dead,
            _ => {}
        }
        match event {
            HookEvent::Suspended => self.suspended = true,
            HookEvent::Resumed => self.suspended = false,
            HookEvent::Connected => self.just_connected = true,
            HookEvent::Running => self.just_connected = false,
            _ => {}
        }

        match event {
            // A hook reporting itself stopped right after connecting has no layout yet:
            // that is not the user's Stop, it is a hook waiting to be told what to do.
            HookEvent::Stopped => {
                if self.just_connected && world.layout().is_some_and(|l| l.enabled) {
                    self.just_connected = false;
                    out.push(Effect::Start);
                }
            }
            HookEvent::SettingsChanged | HookEvent::DesktopChanged | HookEvent::DisplayChanged => {
                self.display_changed(out);
            }
            // The display is back: rebuild only if the configuration changed while it was
            // off, then keep re-hooking through the post-resume re-enumeration storm.
            HookEvent::Resumed => {
                if self.display_changed(out) {
                    self.resume_after.push(self.display_generation);
                } else {
                    self.start_watchdog(world, out);
                }
            }
            HookEvent::FocusChanged(process) => out.push(Effect::ProcessSeen(process)),
            // The hook is gone: the one that comes back is handed the current layout.
            HookEvent::Dead => self.stop_previewing(world),
            // The panic shortcut interrupted a preview: the experiment is thrown away and
            // the engine goes back on the current layout (C#: `AbandonPreviewAsync`).
            HookEvent::Rescued if self.previewing => {
                self.stop_previewing(world);
                self.reconcile_fresh_layout(world, out);
            }
            // Consumed by the frontend (badge, status, rescue); nothing to reconcile.
            _ => {}
        }
    }

    //==================//
    // Display changes  //
    //==================//

    /// C# `DisplayChangeCoordinator.NotifyAsync`, first half: a new generation, which
    /// makes any flow in progress stale, and the debounce. Returns whether a flow
    /// started (not while the display is off: nothing at all then, the hook's Resumed
    /// reconciles once there is a desktop).
    fn display_changed(&mut self, out: &mut Vec<Effect>) -> bool {
        if self.suspended {
            return false;
        }
        self.display_generation += 1;
        self.display = DisplayFlow::Debouncing;
        out.push(Effect::WakeAfter {
            wake: Wake::Display(self.display_generation),
            after: self.timings.debounce,
        });
        true
    }

    /// The rest of `NotifyAsync`, one timer at a time: the settle loop (rebuild only
    /// once two consecutive signatures agree, within a cap), then the idempotence guard
    /// (a configuration identical to the built one is re-hooked, not rebuilt: rebuilding
    /// for nothing is what filled gigabytes over a storm, #412).
    fn display_step(&mut self, generation: u64, world: &mut impl World, out: &mut Vec<Effect>) {
        if generation != self.display_generation {
            // Superseded: the newer flow does the work. A resume waiting on this one
            // goes on, as the C# `await NotifyAsync()` returned.
            self.resumed_flow_done(generation, world, out);
            return;
        }
        let settled = match std::mem::replace(&mut self.display, DisplayFlow::Idle) {
            DisplayFlow::Idle => return,
            DisplayFlow::Debouncing => {
                let signature = world.display_signature();
                if self.timings.stability_max_steps == 0 {
                    signature
                } else {
                    self.settle(signature, 0, out);
                    return;
                }
            }
            DisplayFlow::Settling { signature, steps } => {
                let next = world.display_signature();
                let steps = steps + 1;
                if next == signature || steps >= self.timings.stability_max_steps {
                    next
                } else {
                    self.settle(next, steps, out);
                    return;
                }
            }
        };

        // The display may have gone off meanwhile: the hook unhooked, wait for Resumed.
        if !self.suspended {
            if settled == self.last_built_signature {
                // The settle loop says the configuration STOPPED changing, not that it
                // differs from what is built: the hook unhooked itself over the change,
                // so put it back.
                self.ensure_hooked(world, out);
            } else {
                // The edit was of the desktop that just went away.
                self.stop_previewing(world);
                self.rebuild(world, out);
                self.last_built_signature = settled;
                self.reconcile_fresh_layout(world, out);
            }
        }
        self.resumed_flow_done(generation, world, out);
    }

    fn settle(&mut self, signature: String, steps: u32, out: &mut Vec<Effect>) {
        self.display = DisplayFlow::Settling { signature, steps };
        out.push(Effect::WakeAfter {
            wake: Wake::Display(self.display_generation),
            after: self.timings.stability_step,
        });
    }

    fn resumed_flow_done(
        &mut self,
        generation: u64,
        world: &mut impl World,
        out: &mut Vec<Effect>,
    ) {
        if let Some(i) = self.resume_after.iter().position(|g| *g == generation) {
            self.resume_after.remove(i);
            self.start_watchdog(world, out);
        }
    }

    /// Rebuild the layout from the displays, and say the desktop has to be repainted:
    /// the span was cut for screens that are no longer where they were. One place, so a
    /// rebuild added later cannot forget it.
    fn rebuild(&mut self, world: &mut impl World, out: &mut Vec<Effect>) {
        world.rebuild_layout();
        self.rebuild_count += 1;
        out.push(Effect::Wallpaper);
    }

    /// C# `DisplayChangeCoordinator.RefreshAsync`: the rebuild the automatic detection
    /// missed (#443), past the debounce, the settle loop and the guard — which it then
    /// realigns, so the next display event does not rebuild again on its account.
    fn refresh(&mut self, world: &mut impl World, out: &mut Vec<Effect>) {
        if self.suspended {
            return;
        }
        self.stop_previewing(world);
        self.rebuild(world, out);
        self.last_built_signature = world.display_signature();
        self.reconcile_fresh_layout(world, out);
    }

    //==================//
    // Engine           //
    //==================//

    fn enabled(world: &impl World) -> bool {
        world.layout().is_some_and(|l| l.enabled)
    }

    /// C# `EngineController.StartAsync`: nothing before the first layout.
    fn start(world: &impl World, out: &mut Vec<Effect>) {
        if world.layout().is_some() {
            out.push(Effect::Start);
        }
    }

    /// C# `ReconcileFreshLayoutAsync` (#609): a fresh layout hooks if the user wants it
    /// hooked — and makes sure nothing stays hooked if not. Enabled is stored per layout,
    /// so a display change can land on a layout the user turned off; leaving the hook
    /// alone then would leave it running the layout of the desktop that just went away
    /// (#607).
    fn reconcile_fresh_layout(&mut self, world: &impl World, out: &mut Vec<Effect>) {
        match world.layout() {
            None => {}
            Some(layout) if layout.enabled => out.push(Effect::Start),
            Some(_) => out.push(Effect::Stop),
        }
    }

    /// C# `EnsureHookedAsync`: re-hook without rebuilding, if the engine should run and
    /// does not. Safe with an excluded application focused: the hook's Run no-ops while
    /// paused, so this never forces the hook on over an exclusion.
    fn ensure_hooked(&mut self, world: &impl World, out: &mut Vec<Effect>) {
        if self.watchdog.active
            || self.suspended
            || !Self::enabled(world)
            || self.engine == EngineState::Running
        {
            return;
        }
        Self::start(world, out);
    }

    /// C# `StartFromUserAsync`: the user's Start records itself before hooking — the
    /// recovery guards all read Enabled, a start that does not record itself is one
    /// they keep undoing. A foreign layout is simulated, never adopted: nothing is
    /// recorded, so no recovery path can mistake it for something the user asked to run.
    fn user_start(&mut self, keep_layout: bool, world: &mut impl World, out: &mut Vec<Effect>) {
        // What is started is the current layout (an editor's edit is applied to it first).
        self.stop_previewing(world);
        let Some(layout) = world.layout() else { return };
        if layout.is_virtual {
            out.push(Effect::Start);
            return;
        }
        world.set_enabled(true);
        // A full save writes Enabled too: one write, not two, and only for an edit.
        out.push(if keep_layout && !layout.saved {
            Effect::SaveLayout
        } else {
            Effect::SaveEnabled
        });
        out.push(Effect::Start);
    }

    /// C# `StopFromUserAsync`: recorded, then unhooked — the unhook always, a layout or
    /// not.
    fn user_stop(&mut self, world: &mut impl World, out: &mut Vec<Effect>) {
        // Stopping outranks previewing (C#: `StopAsync` turns live update off first).
        self.stop_previewing(world);
        if world.layout().is_some() {
            world.set_enabled(false);
            out.push(Effect::SaveEnabled);
        }
        out.push(Effect::Stop);
    }

    //==================//
    // Live preview     //
    //==================//

    /// A preview tick. Not while the display is off (the hook unhooked itself), nor for
    /// a foreign layout, which the hook refuses to hook (C#: `SendLiveAsync` sends
    /// nothing then). Turning it on over a stopped engine is a legitimate way to start:
    /// Enabled is not read, and not recorded.
    fn preview(&mut self, world: &mut impl World, out: &mut Vec<Effect>) {
        if self.suspended || world.layout().is_none_or(|l| l.is_virtual) {
            self.stop_previewing(world);
            return;
        }
        self.previewing = true;
        out.push(Effect::Preview);
    }

    /// The frontend ends its preview. C# left the daemon on the last previewed geometry,
    /// saved or not; here a hook still up goes back on the current layout — or down, if
    /// the user does not want this layout hooked. A hook already down stays down.
    fn end_preview(&mut self, world: &mut impl World, out: &mut Vec<Effect>) {
        if !self.previewing {
            world.end_preview();
            return;
        }
        self.stop_previewing(world);
        if matches!(self.engine, EngineState::Running | EngineState::Paused) {
            self.reconcile_fresh_layout(world, out);
        }
    }

    /// The preview ends, without a word to the hook: what replaces it follows.
    fn stop_previewing(&mut self, world: &mut impl World) {
        self.previewing = false;
        world.end_preview();
    }

    //==================//
    // Resume watchdog  //
    //==================//

    /// C# `EnsureRunningAfterResumeAsync`: keep re-asserting Start across the display
    /// re-enumeration storm that follows a wake — one Start reliably loses a race with a
    /// late display change that unhooks the hook again — until it sticks, bounded so a
    /// legitimately paused engine cannot loop for the session. A newer resume
    /// supersedes an older watchdog.
    fn start_watchdog(&mut self, world: &impl World, out: &mut Vec<Effect>) {
        if !Self::enabled(world) {
            return;
        }
        self.watchdog_generation += 1;
        self.watchdog = Watchdog {
            active: true,
            ..Default::default()
        };
        // A Start this same event just sent (the settled display's re-hook, a fresh
        // layout's hand-over) is given one step to take: the hook cannot have answered
        // yet, and checking now would send it again at once. (C# does: its watchdog's
        // first check runs before the hook's Running can reach it.)
        if out.contains(&Effect::Start) {
            out.push(Effect::WakeAfter {
                wake: Wake::Watchdog(self.watchdog_generation),
                after: self.timings.watchdog_step,
            });
        } else {
            self.watchdog_step(self.watchdog_generation, world, out);
        }
    }

    fn watchdog_step(&mut self, generation: u64, world: &impl World, out: &mut Vec<Effect>) {
        if generation != self.watchdog_generation || !self.watchdog.active {
            return;
        }
        let w = &mut self.watchdog;
        let t = &self.timings;
        let done = if w.steps >= t.watchdog_max_steps || self.suspended || !Self::enabled(world) {
            true
        } else if self.engine == EngineState::Running {
            w.quiet += 1;
            w.quiet >= t.watchdog_stable_steps
        } else if w.restarts >= t.watchdog_max_restarts {
            // Given up: the engine is legitimately paused (an excluded application).
            true
        } else {
            w.quiet = 0;
            w.restarts += 1;
            Self::start(world, out);
            false
        };
        if done {
            w.active = false;
            return;
        }
        w.steps += 1;
        out.push(Effect::WakeAfter {
            wake: Wake::Watchdog(generation),
            after: t.watchdog_step,
        });
    }
}
