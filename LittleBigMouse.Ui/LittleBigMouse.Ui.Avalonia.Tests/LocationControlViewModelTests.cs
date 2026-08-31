using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Controls;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The location control's view model lives as long as its view: HLab.Mvvm disposes it when
/// the view leaves the logical tree (window close), and a new one is built on the next open.
/// What these tests pin down is that disposal actually severs everything reaching beyond the
/// view model — the daemon client and the layout are process-lifetime residents, so anything
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
        public FakeDaemon Daemon { get; } = new();
        public FakePersistence Persistence { get; } = new();
        public FakeMainService Main { get; } = new();
        public FakeTicker Ticker { get; } = new();
        public LocationControlViewModel Vm { get; }

        public Fixture()
        {
            Vm = new LocationControlViewModel(
                Daemon,
                Main,
                new FakeMonitorsService(),
                Persistence,
                new EngineController(Daemon, Persistence, () => Vm?.Model),
                onUiThread: run => run(),
                postToUi: post => post(),
                liveTicker: _ => Ticker);
        }
    }

    [Fact]
    public void BuildingTheViewModelSubscribesToTheDaemon()
    {
        var f = new Fixture();

        Assert.True(f.Daemon.HasSubscribers);
    }

    [Fact]
    public void DisposingTheViewModelGivesTheDaemonSubscriptionBack()
    {
        var f = new Fixture();

        f.Vm.Dispose();

        Assert.False(f.Daemon.HasSubscribers);
    }

    /// <summary>Positive control: gives the negative test below its meaning.</summary>
    [Fact]
    public void ADaemonEventReachesTheViewModelWhileItLives()
    {
        var f = new Fixture();

        f.Daemon.Raise(LittleBigMouseEvent.Running);

        Assert.True(f.Vm.Running);
    }

    [Fact]
    public void AfterDisposalADaemonEventNoLongerReachesTheViewModel()
    {
        var f = new Fixture();
        f.Vm.Dispose();

        f.Daemon.Raise(LittleBigMouseEvent.Running);

        Assert.False(f.Vm.Running);
    }

    [Fact]
    public void DisposalEndsALivePreview()
    {
        // Closing the window while previewing: without this, the ticker keeps feeding the
        // daemon a layout nobody can see or stop, and the dispatcher roots the running timer.
        var f = new Fixture();
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
