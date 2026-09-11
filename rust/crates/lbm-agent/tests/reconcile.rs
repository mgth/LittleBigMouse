//! The reconciler against its specification: the C# tests of
//! `DisplayChangeCoordinator` and `EngineController` (`LittleBigMouse.Ui.Avalonia.Tests`),
//! one Rust test per C# test, on a virtual clock instead of shrunk real delays.
//!
//! The C# classes were tested in isolation, each with fakes for the other; here they
//! are one state machine, so where a C# test counted calls into its neighbour (a
//! "reconcile" of the coordinator is the engine's re-hook), the Rust test checks what
//! that call does.

use std::time::Duration;

use lbm_agent::reconcile::{
    Effect, HookEvent, Input, LayoutState, Reconciler, Timings, Wake, World,
};

/// C#'s `Fast` timings: the debounce and the settle steps of `DisplayChangeCoordinatorTests`
/// (cap 4), the watchdog of `EngineControllerTests` (3 stable steps, 6 restarts, 60 steps).
fn fast() -> Timings {
    Timings {
        debounce: Duration::from_millis(1),
        stability_step: Duration::from_millis(1),
        stability_max_steps: 4,
        watchdog_step: Duration::from_millis(1),
        watchdog_stable_steps: 3,
        watchdog_max_restarts: 6,
        watchdog_max_steps: 60,
    }
}

/// The display configuration and the layout, as the reconciler sees them.
struct FakeWorld {
    signature: Box<dyn FnMut() -> String>,
    signature_reads: u32,
    rebuilds: u32,
    layout: Option<LayoutState>,
}

impl World for FakeWorld {
    fn display_signature(&mut self) -> String {
        self.signature_reads += 1;
        (self.signature)()
    }

    fn rebuild_layout(&mut self) {
        self.rebuilds += 1;
    }

    fn layout(&self) -> Option<LayoutState> {
        self.layout
    }

    fn set_enabled(&mut self, enabled: bool) {
        if let Some(layout) = &mut self.layout {
            layout.enabled = enabled;
        }
    }
}

/// C#'s `FakeDaemon` and `FakePersistence`: the commands sent, the writes made; a Start
/// "sticks" (the hook reports Running) once `starts_before_it_sticks` Starts were sent.
struct Harness {
    reconciler: Reconciler,
    world: FakeWorld,
    now: Duration,
    timers: Vec<(Duration, Wake)>,
    commands: Vec<&'static str>,
    starts_before_it_sticks: u32,
    enabled_writes: Vec<bool>,
    layout_writes: u32,
}

impl Harness {
    fn new(timings: Timings, layout: Option<LayoutState>) -> Self {
        let mut h = Harness {
            reconciler: Reconciler::new(timings),
            world: FakeWorld {
                signature: Box::new(|| "one-monitor".to_owned()),
                signature_reads: 0,
                rebuilds: 0,
                layout,
            },
            now: Duration::ZERO,
            timers: Vec::new(),
            commands: Vec::new(),
            starts_before_it_sticks: 0,
            enabled_writes: Vec::new(),
            layout_writes: 0,
        };
        // C#'s FakeDaemon starts out Stopped.
        h.send(Input::Hook(HookEvent::Stopped));
        h
    }

    fn enabled() -> Self {
        Self::new(fast(), Some(layout(true)))
    }

    fn set_signature(&mut self, signature: &'static str) {
        self.world.signature = Box::new(move || signature.to_owned());
    }

    fn starts(&self) -> usize {
        self.commands.iter().filter(|c| **c == "Start").count()
    }

    fn send(&mut self, input: Input) {
        let effects = self.reconciler.handle(input.clone(), &mut self.world);
        if std::env::var_os("TRACE").is_some() {
            eprintln!("{:?} {input:?} -> {effects:?}", self.now);
        }
        for effect in effects {
            match effect {
                Effect::Start => {
                    self.commands.push("Start");
                    if self.starts() >= self.starts_before_it_sticks as usize {
                        self.send(Input::Hook(HookEvent::Running));
                    }
                }
                Effect::Stop => {
                    self.commands.push("Stop");
                    self.send(Input::Hook(HookEvent::Stopped));
                }
                Effect::SaveEnabled => self
                    .enabled_writes
                    .push(self.world.layout.is_some_and(|l| l.enabled)),
                Effect::SaveLayout => self.layout_writes += 1,
                Effect::WakeAfter { wake, after } => self.timers.push((self.now + after, wake)),
                Effect::ProcessSeen(_) => {}
            }
        }
    }

    /// Runs the timers due within `span`, in time order.
    fn advance(&mut self, span: Duration) {
        let end = self.now + span;
        while let Some(i) = (0..self.timers.len())
            .filter(|&i| self.timers[i].0 <= end)
            .min_by_key(|&i| self.timers[i].0)
        {
            let (at, wake) = self.timers.remove(i);
            self.now = at;
            self.send(Input::Wake(wake));
        }
        self.now = end;
    }

    /// Runs every timer until none is left (a hung flow would fail the cap).
    fn settle(&mut self) {
        for _ in 0..10_000 {
            if self.timers.is_empty() {
                return;
            }
            self.advance(Duration::from_millis(1));
        }
        panic!("timers never ran out: {:?}", self.timers);
    }
}

fn layout(enabled: bool) -> LayoutState {
    LayoutState {
        enabled,
        is_virtual: false,
        saved: true,
    }
}

//==================//
// Display changes  //
//==================//

/// C#: `DisplayChangeCoordinatorTests.TheFirstChangeAlwaysRebuilds`.
#[test]
fn the_first_change_always_rebuilds() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();

    assert_eq!(h.world.rebuilds, 1);
    assert_eq!(h.starts(), 1);
    assert_eq!(h.reconciler.last_built_signature(), "one-monitor");
}

/// C#: `ABurstOfChangesCollapsesIntoASingleRebuild` — ten notifications, one layout.
#[test]
fn a_burst_of_changes_collapses_into_a_single_rebuild() {
    let mut h = Harness::enabled();
    for _ in 0..10 {
        h.send(Input::DisplayChanged);
    }
    h.settle();

    assert_eq!(h.world.rebuilds, 1);
    assert_eq!(h.starts(), 1);
}

/// C#: `ABurstThatKeepsChangingTheConfigurationStillRebuildsOnce` — the wake-from-sleep
/// shape: only the final, settled signature is built.
#[test]
fn a_burst_that_keeps_changing_the_configuration_still_rebuilds_once() {
    let mut h = Harness::enabled();
    for step in ["detaching", "one-monitor", "two-monitors"] {
        h.set_signature(step);
        h.send(Input::DisplayChanged);
    }
    h.settle();

    assert_eq!(h.world.rebuilds, 1);
    assert_eq!(h.reconciler.last_built_signature(), "two-monitors");
}

/// C#: `AConfigurationThatSettlesBackToItselfIsReHookedNotRebuilt` — a DPMS blink, a
/// mode re-apply: the hook unhooked itself over the change, so it is re-hooked, and
/// nothing is rebuilt.
#[test]
fn a_configuration_that_settles_back_to_itself_is_re_hooked_not_rebuilt() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();

    h.send(Input::Hook(HookEvent::DisplayChanged));
    h.send(Input::Hook(HookEvent::Stopped)); // the hook unhooks itself over any change
    h.settle();

    assert_eq!(h.world.rebuilds, 1);
    assert_eq!(h.starts(), 2, "re-hooked");
}

/// C#: `AChangedConfigurationRebuildsAgain`.
#[test]
fn a_changed_configuration_rebuilds_again() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.set_signature("two-monitors");
    h.send(Input::DisplayChanged);
    h.settle();

    assert_eq!(h.world.rebuilds, 2);
    assert_eq!(h.reconciler.last_built_signature(), "two-monitors");
}

/// C#: `WhileTheDisplayIsOffNothingIsEvenRead` — no work starts at all.
#[test]
fn while_the_display_is_off_nothing_is_even_read() {
    let mut h = Harness::enabled();
    h.send(Input::Hook(HookEvent::Suspended));
    h.send(Input::DisplayChanged);
    h.settle();

    assert_eq!(h.world.signature_reads, 0);
    assert_eq!(h.world.rebuilds, 0);
    assert!(h.commands.is_empty());
}

/// C#: `ADisplayGoingOffMidSettleDropsTheRebuild`.
#[test]
fn a_display_going_off_mid_settle_drops_the_rebuild() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.advance(Duration::from_millis(1)); // debounced, settling
    h.send(Input::Hook(HookEvent::Suspended));
    h.settle();

    assert_eq!(h.world.rebuilds, 0);
    assert_eq!(h.reconciler.last_built_signature(), "");
}

/// C#: `RefreshRebuildsWhatTheGuardWouldHaveSkipped` (#443) — and realigns the guard.
#[test]
fn refresh_rebuilds_what_the_guard_would_have_skipped() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Refresh);

    assert_eq!(h.world.rebuilds, 2);
    assert_eq!(h.starts(), 2);

    h.send(Input::Hook(HookEvent::Stopped));
    h.send(Input::DisplayChanged);
    h.settle();
    assert_eq!(
        h.world.rebuilds, 2,
        "the guard knows the refreshed signature"
    );
    assert_eq!(h.starts(), 3, "re-hooked instead");
}

/// C#: `RefreshDoesNothingWhileTheDisplayIsOff`.
#[test]
fn refresh_does_nothing_while_the_display_is_off() {
    let mut h = Harness::enabled();
    h.send(Input::Hook(HookEvent::Suspended));
    h.send(Input::Refresh);

    assert_eq!(h.world.rebuilds, 0);
}

/// C#: `AFlappingConfigurationDoesNotHangTheSettleLoop` — it builds what it last saw.
#[test]
fn a_flapping_configuration_does_not_hang_the_settle_loop() {
    let mut h = Harness::enabled();
    let mut flapping = 0;
    h.world.signature = Box::new(move || {
        flapping += 1;
        format!("flap-{flapping}")
    });
    h.send(Input::DisplayChanged);
    h.settle();

    assert_eq!(h.world.rebuilds, 1);
    // One read after the debounce, then the four capped re-reads.
    assert_eq!(h.reconciler.last_built_signature(), "flap-5");
}

//==================//
// Engine           //
//==================//

/// C#: `EngineControllerTests.AUserStartRecordsItselfBeforeHooking` — and nothing more:
/// the tray has no editor behind it.
#[test]
fn a_user_start_records_itself_before_hooking() {
    let mut h = Harness::new(
        fast(),
        Some(LayoutState {
            saved: false,
            ..layout(false)
        }),
    );
    h.send(Input::UserStart { keep_layout: false });

    assert!(h.world.layout.unwrap().enabled);
    assert_eq!(h.enabled_writes, [true]);
    assert_eq!(h.commands, ["Start"]);
    assert_eq!(h.layout_writes, 0);
}

/// C#: `AStartFromTheEditorKeepsTheGeometryItStarts` — one full save, not two writes.
#[test]
fn a_start_from_the_editor_keeps_the_geometry_it_starts() {
    let mut h = Harness::new(
        fast(),
        Some(LayoutState {
            saved: false,
            ..layout(false)
        }),
    );
    h.send(Input::UserStart { keep_layout: true });

    assert!(h.world.layout.unwrap().enabled);
    assert_eq!(h.layout_writes, 1);
    assert!(h.enabled_writes.is_empty());
    assert_eq!(h.commands, ["Start"]);
}

/// C#: `AStartFromTheEditorWithNothingToKeepWritesOnlyEnabled`.
#[test]
fn a_start_from_the_editor_with_nothing_to_keep_writes_only_enabled() {
    let mut h = Harness::new(fast(), Some(layout(false)));
    h.send(Input::UserStart { keep_layout: true });

    assert_eq!(h.layout_writes, 0);
    assert_eq!(h.enabled_writes, [true]);
    assert_eq!(h.commands, ["Start"]);
}

/// C#: `StartingAForeignLayoutSimulatesItWithoutAdoptingIt`.
#[test]
fn starting_a_foreign_layout_simulates_it_without_adopting_it() {
    let foreign = LayoutState {
        is_virtual: true,
        ..layout(false)
    };
    let mut h = Harness::new(fast(), Some(foreign));
    h.send(Input::UserStart { keep_layout: true });

    assert!(!h.world.layout.unwrap().enabled);
    assert!(h.enabled_writes.is_empty());
    assert_eq!(h.layout_writes, 0);
    assert_eq!(h.commands, ["Start"]);
}

/// C#: `AUserStopRecordsItselfBeforeUnhooking`.
#[test]
fn a_user_stop_records_itself_before_unhooking() {
    let mut h = Harness::enabled();
    h.send(Input::UserStop);

    assert!(!h.world.layout.unwrap().enabled);
    assert_eq!(h.enabled_writes, [false]);
    assert_eq!(h.commands, ["Stop"]);
}

/// C#: `StartingWithNoLayoutYetIsANoOpRatherThanAThrow`.
#[test]
fn starting_with_no_layout_yet_is_a_no_op_rather_than_a_throw() {
    let mut h = Harness::new(fast(), None);
    h.send(Input::UserStart { keep_layout: false });
    // The hook asking for a layout right after connecting is the other Start path.
    h.send(Input::Hook(HookEvent::Connected));
    h.send(Input::Hook(HookEvent::Stopped));

    assert!(h.commands.is_empty());
}

/// C#: `AFreshLayoutIsOnlyHandedOverWhenTheUserWantsTheEngine`, as #609 changed it: a
/// fresh layout the user turned off also takes down whatever the hook still runs —
/// the layout of the desktop that went away (#607). (Master's C# sends nothing then.)
#[test]
fn a_fresh_layout_is_only_handed_over_when_the_user_wants_the_engine() {
    let mut enabled = Harness::enabled();
    enabled.send(Input::DisplayChanged);
    enabled.settle();
    assert_eq!(enabled.commands, ["Start"]);

    let mut disabled = Harness::new(fast(), Some(layout(false)));
    disabled.send(Input::DisplayChanged);
    disabled.settle();
    assert_eq!(disabled.commands, ["Stop"]);
}

/// C#: `ReHookingSkipsAnEngineThatIsAlreadyHooked`.
#[test]
fn re_hooking_skips_an_engine_that_is_already_hooked() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.commands.clear();

    // Settles back to the built configuration while the hook stayed Running.
    h.send(Input::DisplayChanged);
    h.settle();

    assert!(h.commands.is_empty());
}

/// C#: `ReHookingPutsBackAHookTheDaemonDroppedOnItsOwn`.
#[test]
fn re_hooking_puts_back_a_hook_the_daemon_dropped_on_its_own() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.commands.clear();

    h.send(Input::Hook(HookEvent::Stopped));
    h.send(Input::DisplayChanged);
    h.settle();

    assert_eq!(h.commands, ["Start"]);
}

/// C#: `ReHookingStandsDownWhileTheDisplayIsOff` — the re-hook of a flow that
/// settled just as the display went off.
#[test]
fn re_hooking_stands_down_while_the_display_is_off() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.commands.clear();

    h.send(Input::Hook(HookEvent::Stopped));
    h.send(Input::DisplayChanged);
    h.send(Input::Hook(HookEvent::Suspended));
    h.settle();

    assert!(h.commands.is_empty());
}

/// C#: `ReHookingNeverOverridesAUserWhoTurnedTheEngineOff`.
#[test]
fn re_hooking_never_overrides_a_user_who_turned_the_engine_off() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::UserStop);
    h.commands.clear();

    h.send(Input::DisplayChanged);
    h.settle();

    assert!(h.commands.is_empty());
}

//==================//
// Resume watchdog  //
//==================//

/// The display comes back: the reconciler settles the display, then its watchdog runs.
fn resume(h: &mut Harness) {
    h.send(Input::Hook(HookEvent::Suspended));
    h.send(Input::Hook(HookEvent::Resumed));
}

/// C#: `AResumeThatTakesFirstTimeStartsOnce`.
#[test]
fn a_resume_that_takes_first_time_starts_once() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped)); // unhooked by the suspend
    h.commands.clear();

    resume(&mut h);
    h.settle();

    assert_eq!(h.starts(), 1);
    assert!(!h.reconciler.watchdog_active());
}

/// C#: `AResumeReAssertsStartUntilTheHookSticks` — the race the watchdog is for: a late
/// display change unhooks the hook again right after the wake.
#[test]
fn a_resume_re_asserts_start_until_the_hook_sticks() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped));
    h.commands.clear();
    h.starts_before_it_sticks = 3;

    resume(&mut h);
    h.settle();

    assert_eq!(h.starts(), 3);
    assert_eq!(
        h.reconciler.engine(),
        lbm_agent::reconcile::EngineState::Running
    );
}

/// C#: `AResumeGivesUpOnAnEngineThatIsLegitimatelyPaused` — an excluded game focused
/// at wake; bounded.
#[test]
fn a_resume_gives_up_on_an_engine_that_is_legitimately_paused() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped));
    h.commands.clear();
    h.starts_before_it_sticks = u32::MAX;

    resume(&mut h);
    h.settle();

    // The settled display's re-hook, then the watchdog's bounded restarts (the C# test
    // calls the watchdog alone, and counts only the latter).
    assert_eq!(h.starts(), 1 + fast().watchdog_max_restarts as usize);
    assert!(!h.reconciler.watchdog_active());
}

/// C#: `AResumeStopsTheMomentTheDisplayGoesBackToSleep`.
#[test]
fn a_resume_stops_the_moment_the_display_goes_back_to_sleep() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped));
    h.commands.clear();
    h.starts_before_it_sticks = u32::MAX;

    resume(&mut h);
    h.advance(Duration::from_millis(3)); // the display settled, the watchdog started
    assert!(h.reconciler.watchdog_active());
    h.send(Input::Hook(HookEvent::Suspended));
    h.settle();

    assert!(h.starts() < fast().watchdog_max_restarts as usize);
    assert!(!h.reconciler.watchdog_active());
}

/// C#: `AResumeStopsTheMomentTheUserDisablesTheEngine`.
#[test]
fn a_resume_stops_the_moment_the_user_disables_the_engine() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped));
    h.commands.clear();
    h.starts_before_it_sticks = u32::MAX;

    resume(&mut h);
    h.advance(Duration::from_millis(3));
    h.send(Input::UserStop);
    h.settle();

    assert!(h.starts() < fast().watchdog_max_restarts as usize);
    assert!(!h.reconciler.watchdog_active());
}

/// C#: `AResumeIsNotAttemptedAtAllWhenTheEngineIsDisabled`.
#[test]
fn a_resume_is_not_attempted_at_all_when_the_engine_is_disabled() {
    let mut h = Harness::new(fast(), Some(layout(false)));
    h.send(Input::DisplayChanged);
    h.settle();
    h.commands.clear();

    resume(&mut h);
    h.settle();

    assert!(!h.commands.contains(&"Start"));
    assert!(!h.reconciler.watchdog_active());
}

/// C#: `ANewerResumeSupersedesTheWatchdogStillRunningForTheOlderOne` — two wakes in
/// quick succession leave one watchdog driving.
#[test]
fn a_newer_resume_supersedes_the_watchdog_still_running_for_the_older_one() {
    let mut h = Harness::enabled();
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped));
    h.starts_before_it_sticks = u32::MAX;

    resume(&mut h);
    h.advance(Duration::from_millis(3));
    assert!(h.reconciler.watchdog_active());
    resume(&mut h);
    h.advance(Duration::from_millis(3));
    let starts = h.starts();
    h.starts_before_it_sticks = 0;
    h.settle();

    assert!(!h.reconciler.watchdog_active());
    assert_eq!(
        h.reconciler.engine(),
        lbm_agent::reconcile::EngineState::Running
    );
    assert_eq!(h.starts(), starts + 1, "one watchdog, one more Start");
}

/// C#: `WhileTheResumeWatchdogRunsTheWeakerReHookStandsAside` — a single-step watchdog
/// with a long pause gives a window where it is provably the driver.
#[test]
fn while_the_resume_watchdog_runs_the_weaker_re_hook_stands_aside() {
    let timings = Timings {
        watchdog_step: Duration::from_millis(300),
        watchdog_max_steps: 1,
        ..fast()
    };
    let mut h = Harness::new(timings, Some(layout(true)));
    h.send(Input::DisplayChanged);
    h.settle();
    h.send(Input::Hook(HookEvent::Stopped));
    h.commands.clear();
    h.starts_before_it_sticks = u32::MAX;

    resume(&mut h);
    h.advance(Duration::from_millis(3)); // settled: re-hooked once, the watchdog took over
    assert!(h.reconciler.watchdog_active());
    assert_eq!(h.starts(), 1);

    // A display change that settles back to the built configuration: the re-hook stands
    // aside while the watchdog drives.
    h.send(Input::DisplayChanged);
    h.advance(Duration::from_millis(5));
    assert_eq!(h.starts(), 1);

    // The watchdog's single step re-asserts the Start, then lets go.
    h.settle();
    assert!(!h.reconciler.watchdog_active());
    let before = h.starts();

    // Once the watchdog has let go, the re-hook is free to act again.
    h.send(Input::DisplayChanged);
    h.settle();
    assert_eq!(h.starts(), before + 1);
}

//==================//
// Hook events      //
//==================//

/// C#: `MainService.DaemonEventReceivedAsync` — a hook that reports Stopped right after
/// connecting has no layout yet: it is given one, if the user wants the engine.
#[test]
fn a_hook_stopped_right_after_connecting_is_given_the_layout() {
    let mut h = Harness::enabled();
    h.send(Input::Hook(HookEvent::Connected));
    h.send(Input::Hook(HookEvent::Stopped));
    assert_eq!(h.commands, ["Start"]);

    // Not every Stopped: once Running was seen, a Stopped is the hook's own business.
    h.send(Input::Hook(HookEvent::Stopped));
    assert_eq!(h.commands, ["Start"]);

    let mut disabled = Harness::new(fast(), Some(layout(false)));
    disabled.send(Input::Hook(HookEvent::Connected));
    disabled.send(Input::Hook(HookEvent::Stopped));
    assert!(disabled.commands.is_empty());
}

/// The processes the hook sees in the foreground are collected for the exclusion editor.
#[test]
fn focused_processes_are_reported() {
    let mut r = Reconciler::new(fast());
    let mut world = FakeWorld {
        signature: Box::new(String::new),
        signature_reads: 0,
        rebuilds: 0,
        layout: None,
    };
    let effects = r.handle(
        Input::Hook(HookEvent::FocusChanged("/usr/bin/game".into())),
        &mut world,
    );
    assert_eq!(effects, [Effect::ProcessSeen("/usr/bin/game".into())]);
}

/// C#: the boot sequence builds a layout before anything reconciles, and the hook gets it
/// when it asks — while a hook that survived the previous agent (D5) is left alone.
#[test]
fn at_boot_the_first_layout_waits_for_the_hook_to_ask() {
    let mut h = Harness::enabled();
    h.send(Input::Boot);
    assert_eq!(h.world.rebuilds, 1);
    assert!(h.commands.is_empty());
    // The guard does not know it yet: the first display change always rebuilds.
    assert_eq!(h.reconciler.last_built_signature(), "");

    h.send(Input::Hook(HookEvent::Connected));
    h.send(Input::Hook(HookEvent::Stopped));
    assert_eq!(h.commands, ["Start"]);

    // A hook still running from before: it says so, and nothing is sent.
    let mut survivor = Harness::enabled();
    survivor.send(Input::Boot);
    survivor.send(Input::Hook(HookEvent::Connected));
    survivor.send(Input::Hook(HookEvent::Running));
    assert!(survivor.commands.is_empty());
}
