using LittleBigMouse.Ui.Avalonia.Main;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// What the coordinator is for is <em>not</em> rebuilding. A display change arrives as a burst
/// while the OS is still moving, and every filter here exists because rebuilding once per event
/// was tried and cost something real: a fully-wired layout generation dropped per spurious
/// event is what fills gigabytes over a storm (#412), and a rebuild skipped without re-hooking
/// is what leaves the engine stopped ("blue") after a monitor's power-save blink.
/// <para>
/// The timings are shrunk to the millisecond so a burst can be replayed in a test; the
/// generation counter that collapses it does not depend on how long the windows are.
/// </para>
/// </summary>
public sealed class DisplayChangeCoordinatorTests
{
    static readonly DisplayChangeTimings Fast = new(DebounceMs: 1, StabilityStepMs: 1, StabilityMaxSteps: 4);

    /// <summary>
    /// A display configuration under test: what it reads as, and a record of what was asked of
    /// it. <see cref="Signature"/> is what a real factory would return from a cheap probe.
    /// </summary>
    sealed class Display
    {
        public string Signature { get; set; } = "one-monitor";
        public bool Suspended { get; set; }
        public int SignatureReads { get; private set; }
        public int Rebuilds { get; private set; }
        public int Starts { get; private set; }
        public int Reconciles { get; private set; }

        public string Read()
        {
            SignatureReads++;
            return Signature;
        }

        public void Rebuild() => Rebuilds++;

        public Task StartAsync()
        {
            Starts++;
            return Task.CompletedTask;
        }

        public Task ReconcileAsync()
        {
            Reconciles++;
            return Task.CompletedTask;
        }
    }

    static DisplayChangeCoordinator Over(Display display) => new(
        display.Read,
        () => display.Suspended,
        display.Rebuild,
        display.StartAsync,
        display.ReconcileAsync,
        Fast);

    [Fact]
    public async Task TheFirstChangeAlwaysRebuilds()
    {
        var display = new Display();
        var coordinator = Over(display);

        await coordinator.NotifyAsync();

        Assert.Equal(1, display.Rebuilds);
        Assert.Equal(1, display.Starts);
        Assert.Equal("one-monitor", coordinator.LastBuiltSignature);
    }

    [Fact]
    public async Task ABurstOfChangesCollapsesIntoASingleRebuild()
    {
        // Every notification increments the generation before it awaits anything, so by the
        // time the debounce expires only the last one still matches — which is the whole of the
        // trailing debounce. Ten WM_DISPLAYCHANGE, one layout.
        var display = new Display();
        var coordinator = Over(display);

        var burst = Enumerable.Range(0, 10).Select(_ => coordinator.NotifyAsync()).ToArray();
        await Task.WhenAll(burst);

        Assert.Equal(1, display.Rebuilds);
        Assert.Equal(1, display.Starts);
    }

    [Fact]
    public async Task ABurstThatKeepsChangingTheConfigurationStillRebuildsOnce()
    {
        // The realistic wake-from-sleep shape: the events arrive while the configuration is
        // still moving under them. Only the final, settled signature must be built.
        var display = new Display();
        var coordinator = Over(display);

        var burst = new List<Task>();
        foreach (var step in new[] { "detaching", "one-monitor", "two-monitors" })
        {
            display.Signature = step;
            burst.Add(coordinator.NotifyAsync());
        }

        await Task.WhenAll(burst);

        Assert.Equal(1, display.Rebuilds);
        Assert.Equal("two-monitors", coordinator.LastBuiltSignature);
    }

    [Fact]
    public async Task AConfigurationThatSettlesBackToItselfIsReHookedNotRebuilt()
    {
        // A monitor's DPMS power-save blink, a mode re-apply, a stray broadcast: the signature
        // is the one already built. Rebuilding would drop a live layout generation for nothing —
        // but the daemon unhooked itself over the change, so somebody has to re-hook.
        var display = new Display();
        var coordinator = Over(display);

        await coordinator.NotifyAsync();
        await coordinator.NotifyAsync();

        Assert.Equal(1, display.Rebuilds);
        Assert.Equal(1, display.Reconciles);
    }

    [Fact]
    public async Task AChangedConfigurationRebuildsAgain()
    {
        var display = new Display();
        var coordinator = Over(display);

        await coordinator.NotifyAsync();
        display.Signature = "two-monitors";
        await coordinator.NotifyAsync();

        Assert.Equal(2, display.Rebuilds);
        Assert.Equal(0, display.Reconciles);
        Assert.Equal("two-monitors", coordinator.LastBuiltSignature);
    }

    [Fact]
    public async Task WhileTheDisplayIsOffNothingIsEvenRead()
    {
        // Not "rebuild and discard": the point is that no work starts at all. There is no
        // desktop to fingerprint, and the daemon's Resumed reconciles once there is.
        var display = new Display { Suspended = true };
        var coordinator = Over(display);

        await coordinator.NotifyAsync();

        Assert.Equal(0, display.SignatureReads);
        Assert.Equal(0, display.Rebuilds);
        Assert.Equal(0, display.Reconciles);
    }

    [Fact]
    public async Task ADisplayGoingOffMidSettleDropsTheRebuild()
    {
        // Suspended arrived while this change was being settled. The daemon has unhooked; the
        // desktop it describes no longer exists.
        var display = new Display();
        var coordinator = Over(display);

        var pending = coordinator.NotifyAsync();
        display.Suspended = true;
        await pending;

        Assert.Equal(0, display.Rebuilds);
        Assert.Equal("", coordinator.LastBuiltSignature);
    }

    [Fact]
    public async Task RefreshRebuildsWhatTheGuardWouldHaveSkipped()
    {
        // The tray's Refresh (#443): the escape hatch for a change the automatic detection
        // missed. It must go through the idempotence guard, not around and back into it — so it
        // also realigns the built signature, or the next event would rebuild on its account.
        var display = new Display();
        var coordinator = Over(display);

        await coordinator.NotifyAsync();
        await coordinator.RefreshAsync();

        Assert.Equal(2, display.Rebuilds);
        Assert.Equal(2, display.Starts);

        await coordinator.NotifyAsync();
        Assert.Equal(2, display.Rebuilds);
        Assert.Equal(1, display.Reconciles);
    }

    [Fact]
    public async Task RefreshDoesNothingWhileTheDisplayIsOff()
    {
        var display = new Display { Suspended = true };
        var coordinator = Over(display);

        await coordinator.RefreshAsync();

        Assert.Equal(0, display.Rebuilds);
    }

    [Fact]
    public async Task AFlappingConfigurationDoesNotHangTheSettleLoop()
    {
        // A signature that never repeats would loop forever without the step cap. It builds
        // whatever it last saw rather than hanging: a wrong layout is recoverable, a frozen UI
        // is not.
        var flapping = 0;
        var display = new Display();
        var coordinator = new DisplayChangeCoordinator(
            () => $"flap-{flapping++}",
            () => display.Suspended,
            display.Rebuild,
            display.StartAsync,
            display.ReconcileAsync,
            Fast);

        await coordinator.NotifyAsync().WaitAsync(TimeSpan.FromSeconds(5));

        Assert.Equal(1, display.Rebuilds);
    }
}
