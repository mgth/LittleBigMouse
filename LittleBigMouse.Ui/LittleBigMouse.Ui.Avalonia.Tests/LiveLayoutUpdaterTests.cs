using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The live-preview pump. What matters here is what it does <em>not</em> send: the
/// daemon unhooks and re-hooks the mouse on every Load it receives, so a tick that
/// repeats the current geometry is not merely wasteful, it is felt.
/// </summary>
public sealed class LiveLayoutUpdaterTests
{
    static ZonesLayout OneMonitor(double widthMm = 480)
    {
        var zones = new ZonesLayout();
        zones.Zones.Add(new Zone(
            new BorderResistance(), "DEV", "Monitor",
            new Rect(0, 0, 1920, 1080),
            new Rect(0, 0, widthMm, 270)));
        zones.Init();
        return zones;
    }

    sealed class Recorder
    {
        public List<ZonesLayout> Sent { get; } = [];
        public Func<Task>? Behaviour { get; set; }

        public Task Send(ZonesLayout zones, CancellationToken token)
        {
            Sent.Add(zones);
            return Behaviour?.Invoke() ?? Task.CompletedTask;
        }
    }

    [Fact]
    public async Task TheFirstTickSendsWhatThereIs()
    {
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => OneMonitor(), recorder.Send);

        Assert.True(await updater.TickAsync());
        Assert.Single(recorder.Sent);
    }

    [Fact]
    public async Task AnUnchangedLayoutIsNotSentAgain()
    {
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => OneMonitor(), recorder.Send);

        await updater.TickAsync();
        Assert.False(await updater.TickAsync());
        Assert.False(await updater.TickAsync());

        Assert.Single(recorder.Sent);
    }

    [Fact]
    public async Task EachChangeCostsExactlyOneSend()
    {
        var width = 480.0;
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => OneMonitor(width), recorder.Send);

        await updater.TickAsync();
        width = 500;
        await updater.TickAsync();
        await updater.TickAsync();
        width = 520;
        await updater.TickAsync();

        Assert.Equal(3, recorder.Sent.Count);
    }

    [Fact]
    public async Task EverythingBetweenTwoTicksCollapsesIntoOneSend()
    {
        // The buffering: the pump reads the layout, it is not pushed to. Ten moves
        // between two ticks are ten reads that never happened.
        var width = 480.0;
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => OneMonitor(width), recorder.Send);

        await updater.TickAsync();
        for (var i = 1; i <= 10; i++) width = 480 + i;
        await updater.TickAsync();

        Assert.Equal(2, recorder.Sent.Count);
        Assert.Equal(490, recorder.Sent[^1].Zones[0].PhysicalBounds.Width);
    }

    [Fact]
    public async Task ForgettingMakesTheNextTickSendAgain()
    {
        // What the daemon holds stops following from what we sent as soon as anything
        // else has fed it — a Start, a rebuild, the switch coming back on.
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => OneMonitor(), recorder.Send);

        await updater.TickAsync();
        updater.Forget();

        Assert.True(await updater.TickAsync());
        Assert.Equal(2, recorder.Sent.Count);
    }

    [Fact]
    public async Task TicksDuringASendAreDroppedNotQueued()
    {
        var gate = new TaskCompletionSource();
        var width = 480.0;
        var recorder = new Recorder { Behaviour = () => gate.Task };
        var updater = new LiveLayoutUpdater(() => OneMonitor(width), recorder.Send);

        var inFlight = updater.TickAsync();
        Assert.False(inFlight.IsCompleted);

        // The layout keeps moving while the daemon is busy.
        width = 500;
        Assert.False(await updater.TickAsync());
        width = 520;
        Assert.False(await updater.TickAsync());

        gate.SetResult();
        Assert.True(await inFlight);

        // One send, carrying the geometry as it stood when it left.
        Assert.Single(recorder.Sent);
        Assert.Equal(480, recorder.Sent[0].Zones[0].PhysicalBounds.Width);

        // And the next tick carries the latest, not the two it skipped.
        Assert.True(await updater.TickAsync());
        Assert.Equal(520, recorder.Sent[^1].Zones[0].PhysicalBounds.Width);
    }

    [Fact]
    public async Task AFailedSendIsRetriedOnTheNextTick()
    {
        var recorder = new Recorder { Behaviour = () => Task.FromException(new IOException("daemon gone")) };
        var updater = new LiveLayoutUpdater(() => OneMonitor(), recorder.Send);

        Assert.False(await updater.TickAsync());
        Assert.False(await updater.TickAsync());

        // Nothing was recorded as delivered, so the same geometry goes out each time.
        Assert.Equal(2, recorder.Sent.Count);

        recorder.Behaviour = null;
        Assert.True(await updater.TickAsync());
        Assert.False(await updater.TickAsync());
    }

    [Fact]
    public async Task NoLayoutMeansNoSend()
    {
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => null, recorder.Send);

        Assert.False(await updater.TickAsync());
        Assert.Empty(recorder.Sent);
    }
}
