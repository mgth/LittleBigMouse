using System.Reactive.Threading.Tasks;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Controls;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The location control's view model lives as long as its view: HLab.Mvvm disposes it when
/// the view leaves the logical tree (window close), and a new one is built on the next open.
/// What these tests pin down is that disposal actually severs everything reaching beyond the
/// view model — the agent's client and the layout are process-lifetime residents, so anything
/// they still hold after Dispose is a generation of the UI that can never be collected.
/// <para>
/// Built through the internal constructor: the UI-thread seams run inline and the live
/// ticker is a recording fake, so no test touches Avalonia's dispatcher — without a platform
/// it belongs to whichever thread reaches it first, and a blocking Invoke from any other
/// thread waits forever on a loop nobody pumps.
/// </para>
/// </summary>
public sealed class LocationControlViewModelTests
{
    sealed class FakeMainService : IMainService
    {
        public IMonitorsLayout MonitorsLayout { get; set; } =
            MainServiceFakes.NewLayout(new ILayoutOptions.Design());

        public bool LivePreview { get; set; }

        public void UpdateLayout() { }
        public void ReloadSystemLayout() { }
        public Task StartNotifierAsync() => Task.CompletedTask;
        public Task ShowControlAsync() => Task.CompletedTask;
        public void CloseControl() { }
        public void AddControlPlugin(Action<IMainPluginsViewModel>? action) { }
    }

    sealed class FakeMonitorsService : ISystemMonitorsService
    {
        // Only ExportConfig walks Root, and only on Windows; no test comes near it.
        public DisplayDevice Root => null!;
        public DesktopWallpaperPosition WallpaperPosition => default;
    }

    sealed class FakeTicker : ILiveTicker
    {
        public bool Running { get; private set; }
        public void Start() => Running = true;
        public void Stop() => Running = false;
    }

    sealed class Fixture
    {
        /// <summary>
        /// Never started, so it never opens a socket: the frames an agent would have sent
        /// are handed to it directly, over the client's own parsing.
        /// </summary>
        public AgentClient Agent { get; }

        public FakePersistence Persistence { get; } = new();
        public FakeMainService Main { get; } = new();
        public FakeTicker Ticker { get; } = new();
        public LocationControlViewModel Vm { get; }

        public Fixture(AgentClient? agent = null)
        {
            Agent = agent ?? new AgentClient();
            Vm = new LocationControlViewModel(
                Agent,
                Main,
                new FakeMonitorsService(),
                Persistence,
                onUiThread: run => run(),
                postToUi: post => post(),
                liveTicker: _ => Ticker);
        }

        /// <summary>The hook said something, as the agent forwards it.</summary>
        public void Hook(LittleBigMouseEvent hookEvent)
            => Agent.Receive(AgentFrames.Hook(hookEvent));
    }

    /// <summary>
    /// With no agent answering there is nothing to drive: the view opens showing a dead
    /// engine rather than an idle one, and Start is not offered.
    /// </summary>
    [Fact]
    public void BeforeAnyAgentHasSpokenTheEngineIsDead()
    {
        var f = new Fixture();

        Assert.True(f.Vm.Dead);
        Assert.False(f.Vm.Running);
    }

    /// <summary>Positive control: gives the negative test below its meaning.</summary>
    [Fact]
    public void AHookEventReachesTheViewModelWhileItLives()
    {
        var f = new Fixture();

        f.Hook(LittleBigMouseEvent.Running);

        Assert.True(f.Vm.Running);
    }

    [Fact]
    public void AfterDisposalAHookEventNoLongerReachesTheViewModel()
    {
        // The subscription is the leak that matters: the client outlives every generation
        // of this view model, so a handler it cannot give back holds all of them.
        var f = new Fixture();
        f.Vm.Dispose();

        f.Hook(LittleBigMouseEvent.Running);

        Assert.False(f.Vm.Running);
    }

    [Fact]
    public async Task AnAgentGoingAwayIsTheSameAsNoHookAtAll()
    {
        // Nothing to drive, whether the hook died or the agent holding it did — and the
        // view has to say so rather than stay on a Running nobody is serving any more.
        // Over a real connection: the agent leaving is not something a frame can say.
        var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        var f = new Fixture(client);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot("Running"));
        await FakeAgent.WaitFor(() => f.Vm.Running);

        await agent.DisposeAsync();

        // Both at once: the two are set together, on the connection's thread, and reading
        // them one after the other from here can catch the pair half-published.
        await FakeAgent.WaitFor(() => f.Vm is { Dead: true, Running: false });
    }

    [Fact]
    public async Task WhatTheAgentRefusesIsShownRatherThanThrown()
    {
        // A command that lets the refusal out has nowhere to report it, and the layout is
        // not lost by a save that did not happen: say so where the load outcome is shown,
        // and leave the model dirty so the button is still there to press again.
        var agent = FakeAgent.Start();
        using var client = new AgentClient(agent.Endpoint);
        var f = new Fixture(client);
        client.Start();
        await agent.AnswerAsync(await agent.NextRequestAsync(), AgentFrames.Snapshot("Running"));
        await FakeAgent.WaitFor(() => f.Vm.Running);

        var layout = MainServiceFakes.NewLayout(new LbmOptions());
        layout.Saved = false;
        f.Vm.Model = layout;

        var saving = f.Vm.SaveCommand.Execute().ToTask();
        var request = await agent.NextRequestAsync();
        Assert.Equal("SaveLayout", request.GetProperty("Method").GetString());
        await agent.ErrorAsync(request, "the layout TESTMON1 is not the current one");
        await saving.WaitAsync(FakeAgent.Patience);

        Assert.Contains("not the current one", f.Vm.DaemonLayoutInfo);
        Assert.False(layout.Saved, "a refused save leaves the layout unsaved");
    }

    [Fact]
    public void DisposalEndsALivePreview()
    {
        // Closing the window while previewing: without this, the ticker keeps feeding the
        // daemon a layout nobody can see or stop, and the dispatcher roots the running timer.
        var f = new Fixture();
        f.Hook(LittleBigMouseEvent.Stopped); // an agent is there: live update is offered
        f.Vm.LiveUpdate = true;
        Assert.True(f.Main.LivePreview);
        Assert.True(f.Ticker.Running);

        f.Vm.Dispose();

        Assert.False(f.Main.LivePreview);
        Assert.False(f.Ticker.Running);
    }

    [Fact]
    public void AfterDisposalTheLayoutNoLongerReachesTheViewModel()
    {
        // The WhenAnyValue chains watching Model.Saved subscribe to the layout itself,
        // which lives on MainService: Dispose must let go of the model to unhook them.
        var f = new Fixture();
        var layout = MainServiceFakes.NewLayout(new LbmOptions());
        f.Vm.Model = layout;

        f.Vm.Dispose();
        Assert.Null(f.Vm.Model);

        f.Vm.Saved = true;
        layout.Saved = false;

        Assert.True(f.Vm.Saved);
    }
}
