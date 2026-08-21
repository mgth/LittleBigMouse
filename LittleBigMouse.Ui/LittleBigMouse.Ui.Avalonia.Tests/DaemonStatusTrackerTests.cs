using System.ComponentModel;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// Turning what the daemon says into what the UI shows. The daemon reports facts and nothing
/// else — it deliberately knows nothing about layouts, previews or windows — so every judgement
/// about what an event <em>means</em> is made on this side, and can be got wrong here.
/// </summary>
public sealed class DaemonStatusTrackerTests
{
    sealed class Fixture
    {
        /// <summary>Rescues seen, each recording whether a preview was interrupted.</summary>
        public List<bool> Rescues { get; } = [];

        public bool Previewing { get; set; }

        /// <summary>Set to hold the marshalled writes instead of running them.</summary>
        public List<Action> Held { get; } = [];
        public bool HoldWrites { get; set; }

        public DaemonStatusTracker Tracker { get; }

        public Fixture()
        {
            Tracker = new DaemonStatusTracker(
                run =>
                {
                    if (HoldWrites) Held.Add(run);
                    else run();
                },
                () => Previewing,
                was => Rescues.Add(was));
        }

        public void Raise(LittleBigMouseEvent daemonEvent, string payload = "")
            => Tracker.Apply(new LittleBigMouseServiceEventArgs(daemonEvent, payload));
    }

    [Theory]
    [InlineData(LittleBigMouseEvent.Running, true, false)]
    [InlineData(LittleBigMouseEvent.Stopped, false, false)]
    [InlineData(LittleBigMouseEvent.Dead, false, true)]
    public void TheEngineStateEventsSetBothFlagsTogether(
        LittleBigMouseEvent daemonEvent, bool running, bool dead)
    {
        var f = new Fixture();

        f.Raise(daemonEvent);

        Assert.Equal(running, f.Tracker.Running);
        Assert.Equal(dead, f.Tracker.Dead);
    }

    [Fact]
    public void ADaemonThatComesBackIsNoLongerDead()
    {
        // Dead is not terminal: the UI reconnects, and a Start over a stale Dead flag would be
        // refused by every command that guards on it.
        var f = new Fixture();

        f.Raise(LittleBigMouseEvent.Dead);
        f.Raise(LittleBigMouseEvent.Running);

        Assert.True(f.Tracker.Running);
        Assert.False(f.Tracker.Dead);
    }

    [Fact]
    public void ALoadOutcomeIsTheOnlyFeedbackASimulationProduces()
    {
        // A Load without Run is never followed by Running, so this string is the whole of what
        // the virtual-layout badge can show.
        var f = new Fixture();

        f.Raise(LittleBigMouseEvent.Loaded, "3 zones (3 main), virtual");

        Assert.Equal("3 zones (3 main), virtual", f.Tracker.LayoutInfo);
        Assert.False(f.Tracker.Running);
    }

    [Fact]
    public void AFailedLoadAlwaysSaysSomethingEvenWithoutAPayload()
    {
        var f = new Fixture();

        f.Raise(LittleBigMouseEvent.LoadFailed, "zone 2 overlaps zone 1");
        Assert.Equal("zone 2 overlaps zone 1", f.Tracker.LayoutInfo);

        f.Raise(LittleBigMouseEvent.LoadFailed);
        Assert.Equal("load failed", f.Tracker.LayoutInfo);
    }

    [Fact]
    public void ARescueThatInterruptedAPreviewSaysTheExperimentWasThrownAway()
    {
        var f = new Fixture { Previewing = true };

        f.Raise(LittleBigMouseEvent.Rescued);

        Assert.Equal(new[] { true }, f.Rescues);
        Assert.Equal("rescued: back to the saved layout", f.Tracker.LayoutInfo);
    }

    [Fact]
    public void ARescueOutsideAPreviewLeavesTheEngineDownWithoutPromisingAReload()
    {
        // What trapped the user is what they committed to: reloading would put it straight back.
        var f = new Fixture { Previewing = false };

        f.Raise(LittleBigMouseEvent.Rescued);

        Assert.Equal(new[] { false }, f.Rescues);
        Assert.Equal("rescued: engine stopped", f.Tracker.LayoutInfo);
    }

    [Theory]
    [InlineData(LittleBigMouseEvent.Suspended)]
    [InlineData(LittleBigMouseEvent.Resumed)]
    [InlineData(LittleBigMouseEvent.Probed)]
    [InlineData(LittleBigMouseEvent.ShortcutUnavailable)]
    public void EventsThisBuildDoesNotHandleAreIgnoredRatherThanRejected(LittleBigMouseEvent unknown)
    {
        // These used to throw. The handler that carries them is the one that also carries
        // Running, so faulting on a newer daemon's vocabulary loses the engine state with it.
        var f = new Fixture();
        f.Raise(LittleBigMouseEvent.Running);

        f.Raise(unknown, "whatever");

        Assert.True(f.Tracker.Running);
        Assert.Equal("", f.Tracker.LayoutInfo);
    }

    [Theory]
    [InlineData(LittleBigMouseEvent.SettingsChanged)]
    [InlineData(LittleBigMouseEvent.DisplayChanged)]
    [InlineData(LittleBigMouseEvent.DesktopChanged)]
    [InlineData(LittleBigMouseEvent.FocusChanged)]
    [InlineData(LittleBigMouseEvent.Paused)]
    [InlineData(LittleBigMouseEvent.Connected)]
    public void EventsThatAreNotAboutTheEngineStateLeaveItAlone(LittleBigMouseEvent noise)
    {
        var f = new Fixture();
        f.Raise(LittleBigMouseEvent.Running);
        f.Raise(LittleBigMouseEvent.Loaded, "3 zones");

        f.Raise(noise);

        Assert.True(f.Tracker.Running);
        Assert.False(f.Tracker.Dead);
        Assert.Equal("3 zones", f.Tracker.LayoutInfo);
    }

    [Fact]
    public void EveryStateWriteGoesThroughTheMarshaller()
    {
        // Apply runs on the daemon's receive thread. A write that skipped the marshaller would
        // reach a bound property off the UI thread, which Avalonia only punishes intermittently.
        var f = new Fixture { HoldWrites = true };

        f.Raise(LittleBigMouseEvent.Running);
        f.Raise(LittleBigMouseEvent.Loaded, "3 zones");

        Assert.False(f.Tracker.Running);
        Assert.Equal("", f.Tracker.LayoutInfo);
        Assert.Equal(2, f.Held.Count);

        foreach (var write in f.Held) write();

        Assert.True(f.Tracker.Running);
        Assert.Equal("3 zones", f.Tracker.LayoutInfo);
    }

    [Fact]
    public void ARescueIsDecidedOnTheUiThreadNotWhenTheEventArrives()
    {
        // The preview flag is UI state; reading it on the receive thread would race the very
        // switch the rescue is about to turn off.
        var f = new Fixture { HoldWrites = true, Previewing = false };

        f.Raise(LittleBigMouseEvent.Rescued);
        Assert.Empty(f.Rescues);

        f.Previewing = true;
        foreach (var write in f.Held) write();

        Assert.Equal(new[] { true }, f.Rescues);
    }

    [Fact]
    public void ForgettingTheLoadOutcomeDropsItWithoutTouchingTheEngineState()
    {
        var f = new Fixture();
        f.Raise(LittleBigMouseEvent.Running);
        f.Raise(LittleBigMouseEvent.Loaded, "3 zones");

        f.Tracker.ForgetLayoutInfo();

        Assert.Equal("", f.Tracker.LayoutInfo);
        Assert.True(f.Tracker.Running);
    }

    [Fact]
    public void TheThreePropertiesNotifyOnChange()
    {
        // The view model republishes these through ToProperty; without notifications the
        // bindings and every command guarding on Running would freeze at their initial value.
        var f = new Fixture();
        f.Raise(LittleBigMouseEvent.Running);

        var changed = new List<string?>();
        ((INotifyPropertyChanged)f.Tracker).PropertyChanged += (_, e) => changed.Add(e.PropertyName);

        // From Running, so both flags actually move — an unchanged value notifies nobody.
        f.Raise(LittleBigMouseEvent.Dead);
        f.Raise(LittleBigMouseEvent.Loaded, "3 zones");

        Assert.Contains(nameof(DaemonStatusTracker.Dead), changed);
        Assert.Contains(nameof(DaemonStatusTracker.Running), changed);
        Assert.Contains(nameof(DaemonStatusTracker.LayoutInfo), changed);
    }
}
