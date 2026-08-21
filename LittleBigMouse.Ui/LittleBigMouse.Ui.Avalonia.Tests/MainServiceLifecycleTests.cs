using HLab.Base.ReactiveUI;
using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Main;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// <c>MainService</c> lives as long as the process, which is exactly why its subscriptions are
/// worth pinning down: a handler that cannot be removed is invisible for as long as that stays
/// true, and becomes a leak the day it stops. Every event it listens to is held as something it
/// can give back, and disposing it gives all of them back.
/// <para>
/// The rest of what it does — deciding when a display change is worth a rebuild, when the
/// engine should be hooked — is asserted where it now lives, in
/// <see cref="DisplayChangeCoordinatorTests"/> and <see cref="EngineControllerTests"/>.
/// </para>
/// </summary>
public sealed class MainServiceLifecycleTests
{
    /// <summary>
    /// The view layer, present only so the service can be built. Any use of it here would be a
    /// test reaching for the window, which is not what these tests are about.
    /// </summary>
    sealed class UnusedMvvmService : IMvvmService
    {
        public ServiceState ServiceState => ServiceState.Available;
        public bool IsPlatformRegistered => true;
        public IMvvmContext MainContext => throw new NotSupportedException();
        public HelperFactory<IViewHelper> ViewHelperFactory => throw new NotSupportedException();
        public void RegisterPlatform<T>() where T : IMvvmPlatformImpl => throw new NotSupportedException();
        public Type? GetLinkedType(Type getType, Type viewMode, Type viewClass) => throw new NotSupportedException();
        public void Register() => throw new NotSupportedException();
        public void Register(Type baseType, Type linkedType, Type viewClass, Type viewMode) => throw new NotSupportedException();
        public IView GetNotFoundView(Type getType, Type viewMode, Type viewClass) => throw new NotSupportedException();
        public void PrepareView(IView view) => throw new NotSupportedException();
        public IWindow ViewAsWindow(IView? view) => throw new NotSupportedException();
        public IWindow ViewAsWindow<T>(IView? view) where T : IWindow, new() => throw new NotSupportedException();
    }

    sealed class Fixture
    {
        public LbmOptions Options { get; } = new();
        public FakeDaemon Daemon { get; } = new();
        public FakePersistence Persistence { get; } = new();
        public FakeNotification Notify { get; } = new();
        public FakeLayoutFactory Factory { get; }
        public MainService Service { get; }

        public Fixture()
        {
            Factory = new FakeLayoutFactory(() => MainServiceFakes.NewLayout(Options));

            Service = new MainService(
                mainViewModelLocator: () => throw new NotSupportedException(),
                mvvmService: new UnusedMvvmService(),
                littleBigMouseClientService: Daemon,
                notify: Notify,
                layoutFactory: Factory,
                layoutPersistence: Persistence,
                processesCollector: new ProcessesCollector(),
                updaterLocator: () => throw new NotSupportedException(),
                options: Options,
                engine: new EngineController(Daemon, Persistence, () => Service?.MonitorsLayout));
        }
    }

    [Fact]
    public void BuildingTheServiceSubscribesToTheDaemonAndThePlatform()
    {
        var f = new Fixture();

        Assert.True(f.Daemon.HasSubscribers);
        Assert.True(f.Factory.HasDisplaySubscribers);
        Assert.True(f.Factory.HasWallpaperSubscribers);
    }

    [Fact]
    public void DisposingTheServiceGivesEverySubscriptionBack()
    {
        // The whole point of naming the handlers. Before, each of these was a lambda with no
        // way back out, so the daemon client and the layout factory held the service — and
        // everything it reaches — for the life of the process whether or not it was still wanted.
        var f = new Fixture();

        f.Service.Dispose();

        Assert.False(f.Daemon.HasSubscribers);
        Assert.False(f.Factory.HasDisplaySubscribers);
        Assert.False(f.Factory.HasWallpaperSubscribers);
    }

    [Fact]
    public async Task AfterDisposalAPlatformDisplayChangeDoesNothingAtAll()
    {
        // Not "does nothing visible": the layout factory must not even be asked to build.
        var f = new Fixture();
        f.Service.Dispose();

        f.Factory.RaiseDisplayChanged();
        f.Factory.RaiseWallpaperChanged();
        f.Daemon.Raise(LittleBigMouse.Zoning.LittleBigMouseEvent.DisplayChanged);

        // Long enough that a surviving handler would have cleared its debounce window.
        await Task.Delay(500);

        Assert.Equal(0, f.Factory.Creations);
        Assert.Equal(0, f.Factory.WallpaperUpdates);
        Assert.Empty(f.Daemon.Commands);
    }

    [Fact]
    public void DisposingTheServiceReleasesTheTrayIconToo()
    {
        var f = new Fixture();

        Assert.False(f.Notify.HasClickSubscribers); // nothing wired before the notifier starts
        f.Service.Dispose();
        Assert.False(f.Notify.HasClickSubscribers);
    }

    [Fact]
    public void DisposingIsIdempotent()
    {
        // Shutdown paths overlap: the container disposes what it owns, and a caller may have
        // done it already. Neither may be the one that throws.
        var f = new Fixture();

        f.Service.Dispose();
        f.Service.Dispose();
    }

    sealed class DisposeProbe : IDisposable
    {
        public bool Disposed { get; private set; }
        public void Dispose() => Disposed = true;
    }

    [Fact]
    public void RebuildingTheLayoutDisposesTheGenerationItReplaces()
    {
        // Every rebuild drops a fully-wired layout that Avalonia's compositor otherwise keeps
        // alive; a storm of them left unreleased is what filled gigabytes (#412).
        var f = new Fixture();

        f.Service.UpdateLayout();
        var first = (MonitorsLayout)f.Service.MonitorsLayout!;
        var firstProbe = new DisposeProbe();
        firstProbe.DisposeWith(first);

        f.Service.UpdateLayout();
        var second = (MonitorsLayout)f.Service.MonitorsLayout!;
        var secondProbe = new DisposeProbe();
        secondProbe.DisposeWith(second);

        Assert.NotSame(first, second);
        Assert.True(firstProbe.Disposed);
        Assert.False(secondProbe.Disposed);
        Assert.Equal(2, f.Factory.Creations);

        f.Service.Dispose();
    }
}
