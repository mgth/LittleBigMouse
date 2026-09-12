using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The live-preview pump. What matters here is what it does <em>not</em> do: a tick over
/// a still layout must not touch the model, and a tick that would hand the agent what it
/// already has must not send — a preview rebuilds the hook's whole zone graph.
/// </summary>
public sealed class LiveLayoutUpdaterTests
{
    /// <summary>
    /// Stands in for <c>SavableReactiveModel.Revision</c>: monotonic, bumped by whatever
    /// marks the model unsaved. The tests move it by hand, exactly where an edit would.
    /// </summary>
    sealed class Model
    {
        readonly MonitorsLayout _layout = MainServiceFakes.NewLayout(new LbmOptions());

        public long Revision { get; private set; }
        public int Reads { get; private set; }

        /// <summary>A monitor's width, the one edit these tests make.</summary>
        public double WidthMm
        {
            get => _layout.PhysicalMonitors[0].Model.PhysicalSize.Width;
            set { _layout.PhysicalMonitors[0].Model.PhysicalSize.Width = value; Revision++; }
        }

        /// <summary>An edit the agent cannot see — a value set back to itself, a repaint.</summary>
        public void TouchWithoutMoving() => Revision++;

        public MonitorsLayout Current()
        {
            Reads++;
            return _layout;
        }
    }

    /// <summary>
    /// The agent's side. What it keeps is the width <em>as it stood when the send left</em>:
    /// the layout is a live object, so holding the reference would say nothing about what
    /// actually went out.
    /// </summary>
    sealed class Recorder
    {
        public List<double> Sent { get; } = [];
        public Func<Task>? Behaviour { get; set; }

        public Task Send(MonitorsLayout layout, CancellationToken token)
        {
            Sent.Add(layout.PhysicalMonitors[0].Model.PhysicalSize.Width);
            return Behaviour?.Invoke() ?? Task.CompletedTask;
        }
    }

    static LiveLayoutUpdater UpdaterOver(Model model, Recorder recorder)
        => new(() => model.Revision, model.Current, recorder.Send);

    [Fact]
    public async Task TheFirstTickSendsWhatThereIs()
    {
        var model = new Model();
        var recorder = new Recorder();
        var updater = UpdaterOver(model, recorder);

        Assert.True(await updater.TickAsync());
        Assert.Single(recorder.Sent);
    }

    [Fact]
    public async Task AStillLayoutIsNeverEvenRead()
    {
        // The reason leaving the switch on costs nothing: with nothing marked unsaved
        // since the last look, a tick stops at the counter — no zones computed, no
        // payload built, no IPC.
        var model = new Model();
        var recorder = new Recorder();
        var updater = UpdaterOver(model, recorder);

        await updater.TickAsync();
        var readsAfterFirstSend = model.Reads;

        Assert.False(await updater.TickAsync());
        Assert.False(await updater.TickAsync());

        Assert.Equal(readsAfterFirstSend, model.Reads);
        Assert.Single(recorder.Sent);
    }

    [Fact]
    public async Task AChangeTheAgentCannotSeeCostsNoSend()
    {
        // The counter is deliberately coarse — anything marking itself unsaved bumps it,
        // including things no zone carries. The payload is what decides.
        var model = new Model();
        var recorder = new Recorder();
        var updater = UpdaterOver(model, recorder);

        await updater.TickAsync();
        model.TouchWithoutMoving();

        Assert.False(await updater.TickAsync());
        Assert.Single(recorder.Sent);

        // And having looked once, it does not look again until something moves.
        var reads = model.Reads;
        Assert.False(await updater.TickAsync());
        Assert.Equal(reads, model.Reads);
    }

    [Fact]
    public async Task EachChangeCostsExactlyOneSend()
    {
        var model = new Model();
        var recorder = new Recorder();
        var updater = UpdaterOver(model, recorder);

        await updater.TickAsync();
        model.WidthMm = 500;
        await updater.TickAsync();
        await updater.TickAsync();
        model.WidthMm = 520;
        await updater.TickAsync();

        Assert.Equal(3, recorder.Sent.Count);
    }

    [Fact]
    public async Task EverythingBetweenTwoTicksCollapsesIntoOneSend()
    {
        // The buffering: the pump reads the layout, it is not pushed to. Ten moves
        // between two ticks are ten reads that never happened.
        var model = new Model();
        var recorder = new Recorder();
        var updater = UpdaterOver(model, recorder);

        await updater.TickAsync();
        var reads = model.Reads;

        for (var i = 1; i <= 10; i++) model.WidthMm = 480 + i;
        await updater.TickAsync();

        Assert.Equal(reads + 1, model.Reads);
        Assert.Equal(2, recorder.Sent.Count);
        Assert.Equal(490, recorder.Sent[^1]);
    }

    [Fact]
    public async Task ForgettingMakesTheNextTickSendAgain()
    {
        // What the agent holds stops following from what we sent as soon as anything
        // else has fed it — a Start, a rebuild, the switch coming back on. Nothing in
        // the model has moved, so the counter alone would have kept the tick quiet.
        var model = new Model();
        var recorder = new Recorder();
        var updater = UpdaterOver(model, recorder);

        await updater.TickAsync();
        updater.Forget();

        Assert.True(await updater.TickAsync());
        Assert.Equal(2, recorder.Sent.Count);
    }

    [Fact]
    public async Task TicksDuringASendAreDroppedNotQueued()
    {
        var gate = new TaskCompletionSource();
        var model = new Model();
        var recorder = new Recorder { Behaviour = () => gate.Task };
        var updater = UpdaterOver(model, recorder);

        var inFlight = updater.TickAsync();
        Assert.False(inFlight.IsCompleted);

        // The layout keeps moving while the agent is busy.
        model.WidthMm = 500;
        Assert.False(await updater.TickAsync());
        model.WidthMm = 520;
        Assert.False(await updater.TickAsync());

        gate.SetResult();
        Assert.True(await inFlight);

        // One send, carrying the geometry as it stood when it left.
        Assert.Single(recorder.Sent);
        Assert.Equal(600, recorder.Sent[0]);

        // And the next tick carries the latest, not the two it skipped — the revision
        // was taken before the model was read, so those edits are not lost.
        Assert.True(await updater.TickAsync());
        Assert.Equal(520, recorder.Sent[^1]);
    }

    [Fact]
    public async Task AnEditLandingDuringASendIsNotLost()
    {
        var gate = new TaskCompletionSource();
        var model = new Model();
        var recorder = new Recorder { Behaviour = () => gate.Task };
        var updater = UpdaterOver(model, recorder);

        var inFlight = updater.TickAsync();
        model.WidthMm = 500;
        gate.SetResult();
        Assert.True(await inFlight);

        Assert.True(await updater.TickAsync());
        Assert.Equal(500, recorder.Sent[^1]);
    }

    [Fact]
    public async Task AFailedSendIsRetriedOnTheNextTick()
    {
        var model = new Model();
        var recorder = new Recorder { Behaviour = () => Task.FromException(new IOException("agent gone")) };
        var updater = UpdaterOver(model, recorder);

        Assert.False(await updater.TickAsync());
        Assert.False(await updater.TickAsync());

        // A failure takes neither the payload nor the revision, so the same geometry
        // goes out again rather than the edit being swallowed.
        Assert.Equal(2, recorder.Sent.Count);

        recorder.Behaviour = null;
        Assert.True(await updater.TickAsync());
        Assert.False(await updater.TickAsync());
    }

    [Fact]
    public async Task NoLayoutMeansNoSend()
    {
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => 1, () => null, recorder.Send);

        Assert.False(await updater.TickAsync());
        Assert.Empty(recorder.Sent);
    }

    [Fact]
    public async Task AForeignLayoutIsNeverPreviewedIntoTheLocalMouse()
    {
        // A layout opened for inspection describes someone else's desktop: the agent
        // refuses to hook it, and sending it every fifth of a second is asking for that
        // refusal over and over.
        var foreign = MainServiceFakes.NewLayout(new LbmOptions(), source: LayoutSource.VirtualFile);
        var recorder = new Recorder();
        var updater = new LiveLayoutUpdater(() => 1, () => foreign, recorder.Send);

        Assert.False(await updater.TickAsync());
        Assert.Empty(recorder.Sent);
    }
}
