using HLab.Base.ReactiveUI;
using HLab.Core.Annotations;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// <c>MainService</c> lives as long as the process, which is exactly why its subscriptions are
/// worth pinning down: a handler that cannot be removed is invisible for as long as that stays
/// true, and becomes a leak the day it stops. Every event it listens to is held as something it
/// can give back, and disposing it gives all of them back.
/// <para>
/// What it no longer does is decide anything about the engine (v6, phase 4): when a display
/// change is worth a rebuild, when the hook should be taken, what is written — all of that is
/// the agent's, and is asserted there. What is left here is the window's own: the layout it
/// shows, and the platform events that make it stale.
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
        public FakePersistence Persistence { get; } = new();
        public FakeLayoutFactory Factory { get; }

        /// <summary>Never started: nothing here reaches an agent, so it never opens a socket.</summary>
        public AgentClient Agent { get; } = new();

        public MainService Service { get; }

        public Fixture()
        {
            Factory = new FakeLayoutFactory(() => MainServiceFakes.NewLayout(Options));

            Service = new MainService(
                mainViewModelLocator: () => throw new NotSupportedException(),
                mvvmService: new UnusedMvvmService(),
                agent: Agent,
                layoutFactory: Factory,
                layoutPersistence: Persistence,
                updaterLocator: () => throw new NotSupportedException(),
                options: Options,
                postToUi: post => post(),
                // Nothing here opens a window, so nothing here closes one either.
                leave: () => Assert.Fail("nothing in these tests should end the process"));
        }
    }

    [Fact]
    public void BuildingTheServiceSubscribesToThePlatform()
    {
        var f = new Fixture();

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

        Assert.False(f.Factory.HasDisplaySubscribers);
        Assert.False(f.Factory.HasWallpaperSubscribers);
    }

    /// <summary>Positive control: gives the negative test below its meaning.</summary>
    [Fact]
    public void APlatformDisplayChangeRebuildsTheLayoutTheWindowShows()
    {
        var f = new Fixture();

        f.Factory.RaiseDisplayChanged();

        Assert.Equal(1, f.Factory.Creations);
        Assert.NotNull(f.Service.MonitorsLayout);
    }

    [Fact]
    public async Task AfterDisposalAPlatformDisplayChangeDoesNothingAtAll()
    {
        // Not "does nothing visible": the layout factory must not even be asked to build.
        var f = new Fixture();
        f.Service.Dispose();

        f.Factory.RaiseDisplayChanged();
        f.Factory.RaiseWallpaperChanged();

        // Long enough that a surviving handler would have cleared its debounce window.
        await Task.Delay(500);

        Assert.Equal(0, f.Factory.Creations);
        Assert.Equal(0, f.Factory.WallpaperUpdates);
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
