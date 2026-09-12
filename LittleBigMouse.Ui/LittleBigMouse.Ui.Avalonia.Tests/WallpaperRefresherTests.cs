using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Main;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The desktop drawn behind each monitor: the one surface left in this process that only
/// ever <em>shows</em> something. It decides nothing, so what is worth pinning down is that
/// a wallpaper change costs a repaint rather than a rebuild, and that it stops listening
/// when told to.
/// <para>
/// The tray icon used to be tested alongside it. It is the agent's now — it is there whether
/// this window runs or not — and its menu and states are asserted in <c>lbm-agent</c>.
/// </para>
/// </summary>
public sealed class WallpaperRefresherTests
{
    [Fact]
    public void TheWallpaperIsRefreshedInPlaceWhenThePlatformSaysItChanged()
    {
        var options = new LbmOptions();
        var layout = MainServiceFakes.NewLayout(options);
        var factory = new FakeLayoutFactory(() => layout);
        using var refresher = new WallpaperRefresher(factory, () => layout, action => action());

        factory.RaiseWallpaperChanged();

        Assert.Equal(1, factory.WallpaperUpdates);
        // In place, not a rebuild: edits in progress have to survive a wallpaper change.
        Assert.Equal(0, factory.Creations);
    }

    [Fact]
    public void AWallpaperChangeBeforeTheFirstLayoutIsSimplyIgnored()
    {
        IMonitorsLayout? layout = null;
        var factory = new FakeLayoutFactory(() => throw new NotSupportedException());
        using var refresher = new WallpaperRefresher(factory, () => layout, action => action());

        factory.RaiseWallpaperChanged();

        Assert.Equal(0, factory.WallpaperUpdates);
    }

    [Fact]
    public void ADisposedRefresherStopsWatchingTheDesktop()
    {
        var options = new LbmOptions();
        var layout = MainServiceFakes.NewLayout(options);
        var factory = new FakeLayoutFactory(() => layout);
        var refresher = new WallpaperRefresher(factory, () => layout, action => action());

        refresher.Dispose();
        factory.RaiseWallpaperChanged();

        Assert.Equal(0, factory.WallpaperUpdates);
        Assert.False(factory.HasWallpaperSubscribers);
    }
}
