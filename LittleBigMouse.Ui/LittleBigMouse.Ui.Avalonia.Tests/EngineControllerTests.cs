using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// Hooking the engine is easy; hooking it <em>back</em> is where the bugs are. The daemon
/// unhooks itself over any display change so the cursor is never confined while the desktop
/// reshapes, which leaves the UI as the only side that can notice the hook is gone — without
/// ever forcing it on over a user who turned it off, or over an exclusion the daemon is
/// deliberately honouring.
/// </summary>
public sealed class EngineControllerTests
{
    static readonly ResumeWatchdogTimings Fast =
        new(StepMs: 1, StableSteps: 3, MaxRestarts: 6, MaxSteps: 60);

    sealed class Fixture
    {
        public FakeDaemon Daemon { get; } = new();
        public FakePersistence Persistence { get; } = new();
        public MonitorsLayout Layout { get; }
        public EngineController Engine { get; }

        public Fixture(bool enabled = true, bool withLayout = true, ResumeWatchdogTimings? timings = null)
        {
            Layout = MainServiceFakes.NewLayout(new LbmOptions(), enabled);
            var layout = withLayout ? Layout : null;
            Engine = new EngineController(Daemon, Persistence, () => layout, timings ?? Fast);
        }
    }

    [Fact]
    public async Task AUserStartRecordsItselfBeforeHooking()
    {
        // A start that doesn't persist Enabled=true is one the recovery guards — which all test
        // Options.Enabled — will keep quietly undoing.
        var f = new Fixture(enabled: false);

        await f.Engine.StartFromUserAsync();

        Assert.True(f.Layout.Options.Enabled);
        Assert.Equal(true, f.Persistence.LastEnabledWritten);
        Assert.Equal(new[] { "Start" }, f.Daemon.Commands);
    }

    [Fact]
    public async Task AUserStopRecordsItselfBeforeUnhooking()
    {
        var f = new Fixture();

        await f.Engine.StopFromUserAsync();

        Assert.False(f.Layout.Options.Enabled);
        Assert.Equal(false, f.Persistence.LastEnabledWritten);
        Assert.Equal(new[] { "Stop" }, f.Daemon.Commands);
    }

    [Fact]
    public async Task StartingWithNoLayoutYetIsANoOpRatherThanAThrow()
    {
        // Reachable only before the first build, which the boot sequence does before the tray
        // exists. A tray click that faults would take the handler down with it.
        var f = new Fixture(withLayout: false);

        await f.Engine.StartAsync();
        await f.Engine.StartFromUserAsync();

        Assert.Empty(f.Daemon.Commands);
    }

    [Fact]
    public async Task AFreshLayoutIsOnlyHandedOverWhenTheUserWantsTheEngine()
    {
        var enabled = new Fixture();
        await enabled.Engine.StartIfEnabledAsync();
        Assert.Equal(new[] { "Start" }, enabled.Daemon.Commands);

        var disabled = new Fixture(enabled: false);
        await disabled.Engine.StartIfEnabledAsync();
        Assert.Empty(disabled.Daemon.Commands);
    }

    [Fact]
    public async Task ReHookingSkipsAnEngineThatIsAlreadyHooked()
    {
        var f = new Fixture();
        f.Daemon.State = LittleBigMouseEvent.Running;

        await f.Engine.EnsureHookedAsync();

        Assert.Empty(f.Daemon.Commands);
    }

    [Fact]
    public async Task ReHookingPutsBackAHookTheDaemonDroppedOnItsOwn()
    {
        // The case the idempotence guard leaves behind: the configuration settled back to the
        // one already built, so nothing rebuilds — and nothing else would re-Start it either.
        var f = new Fixture();
        f.Daemon.State = LittleBigMouseEvent.Stopped;

        await f.Engine.EnsureHookedAsync();

        Assert.Equal(new[] { "Start" }, f.Daemon.Commands);
    }

    [Fact]
    public async Task ReHookingStandsDownWhileTheDisplayIsOff()
    {
        var f = new Fixture();
        f.Engine.Suspended = true;

        await f.Engine.EnsureHookedAsync();

        Assert.Empty(f.Daemon.Commands);
    }

    [Fact]
    public async Task ReHookingNeverOverridesAUserWhoTurnedTheEngineOff()
    {
        var f = new Fixture(enabled: false);

        await f.Engine.EnsureHookedAsync();

        Assert.Empty(f.Daemon.Commands);
    }

    [Fact]
    public async Task AResumeThatTakesFirstTimeStartsOnce()
    {
        var f = new Fixture();

        await f.Engine.EnsureRunningAfterResumeAsync();

        Assert.Equal(1, f.Daemon.StartCount);
        Assert.Equal(LittleBigMouseEvent.Running, f.Daemon.State);
    }

    [Fact]
    public async Task AResumeReAssertsStartUntilTheHookSticks()
    {
        // The race the watchdog exists for: a late WM_DISPLAYCHANGE makes the daemon unhook
        // itself ~1-2s after Resumed, so the first Start is lost and the engine would sit
        // stopped ("blue") until a manual Start.
        var f = new Fixture();
        f.Daemon.StartsBeforeItSticks = 3;

        await f.Engine.EnsureRunningAfterResumeAsync();

        Assert.Equal(3, f.Daemon.StartCount);
        Assert.Equal(LittleBigMouseEvent.Running, f.Daemon.State);
    }

    [Fact]
    public async Task AResumeGivesUpOnAnEngineThatIsLegitimatelyPaused()
    {
        // An excluded app (a game) focused at wake: the daemon accepts Start and stays paused,
        // for ever, on purpose. Bounded so the watchdog does not spin for the session.
        var f = new Fixture();
        f.Daemon.StartsBeforeItSticks = int.MaxValue;

        await f.Engine.EnsureRunningAfterResumeAsync().WaitAsync(TimeSpan.FromSeconds(10));

        Assert.Equal(Fast.MaxRestarts, f.Daemon.StartCount);
        Assert.False(f.Engine.ResumeWatchdogActive);
    }

    [Fact]
    public async Task AResumeStopsTheMomentTheDisplayGoesBackToSleep()
    {
        var f = new Fixture();
        f.Daemon.StartsBeforeItSticks = int.MaxValue;

        var watchdog = f.Engine.EnsureRunningAfterResumeAsync();
        f.Engine.Suspended = true;
        await watchdog.WaitAsync(TimeSpan.FromSeconds(10));

        Assert.True(f.Daemon.StartCount < Fast.MaxRestarts);
    }

    [Fact]
    public async Task AResumeStopsTheMomentTheUserDisablesTheEngine()
    {
        var f = new Fixture();
        f.Daemon.StartsBeforeItSticks = int.MaxValue;

        var watchdog = f.Engine.EnsureRunningAfterResumeAsync();
        f.Layout.Options.Enabled = false;
        await watchdog.WaitAsync(TimeSpan.FromSeconds(10));

        Assert.True(f.Daemon.StartCount < Fast.MaxRestarts);
    }

    [Fact]
    public async Task AResumeIsNotAttemptedAtAllWhenTheEngineIsDisabled()
    {
        var f = new Fixture(enabled: false);

        await f.Engine.EnsureRunningAfterResumeAsync();

        Assert.Empty(f.Daemon.Commands);
    }

    [Fact]
    public async Task ANewerResumeSupersedesTheWatchdogStillRunningForTheOlderOne()
    {
        // Two wakes in quick succession must not leave two watchdogs fighting over the daemon.
        var f = new Fixture();
        f.Daemon.StartsBeforeItSticks = int.MaxValue;

        var older = f.Engine.EnsureRunningAfterResumeAsync();
        var newer = f.Engine.EnsureRunningAfterResumeAsync();

        await older.WaitAsync(TimeSpan.FromSeconds(10));
        f.Daemon.StartsBeforeItSticks = 0;
        await newer.WaitAsync(TimeSpan.FromSeconds(10));

        Assert.False(f.Engine.ResumeWatchdogActive);
        Assert.Equal(LittleBigMouseEvent.Running, f.Daemon.State);
    }

    [Fact]
    public async Task WhileTheResumeWatchdogRunsTheWeakerReHookStandsAside()
    {
        // Both would Start; only one should be driving, and it must be the one that retries.
        // A single-step watchdog with a long pause gives a window where it is provably the
        // active driver and provably idle, so anything that happens is the re-hook's doing.
        var f = new Fixture(timings: new ResumeWatchdogTimings(StepMs: 300, MaxSteps: 1));
        f.Daemon.StartsBeforeItSticks = int.MaxValue;

        var watchdog = f.Engine.EnsureRunningAfterResumeAsync();
        Assert.True(f.Engine.ResumeWatchdogActive);
        Assert.Equal(1, f.Daemon.StartCount);

        await f.Engine.EnsureHookedAsync();
        Assert.Equal(1, f.Daemon.StartCount);

        await watchdog.WaitAsync(TimeSpan.FromSeconds(10));
        Assert.False(f.Engine.ResumeWatchdogActive);

        // Once the watchdog has let go, the re-hook is free to act again.
        await f.Engine.EnsureHookedAsync();
        Assert.Equal(2, f.Daemon.StartCount);
    }
}
