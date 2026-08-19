using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The two surfaces that only ever <em>show</em> something: the tray icon and the wallpaper
/// drawn behind each monitor. Neither decides anything, so what is worth pinning down is the
/// menu the user is actually offered, the state the icon is allowed to claim, and that both
/// stop listening when told to.
/// </summary>
public sealed class TrayAndWallpaperTests
{
    static TrayMenu MenuOver(List<string> log, bool canUpdate = true) => new(
        CheckUpdateAsync: canUpdate ? () => Record(log, "update") : null,
        OpenAsync: () => Record(log, "open"),
        StartAsync: () => Record(log, "start"),
        StopAsync: () => Record(log, "stop"),
        RefreshAsync: () => Record(log, "refresh"),
        QuitAsync: () => Record(log, "quit"));

    static Task Record(List<string> log, string what)
    {
        log.Add(what);
        return Task.CompletedTask;
    }

    [Theory]
    [InlineData(LittleBigMouseEvent.Running, "icon/lbm_on")]
    [InlineData(LittleBigMouseEvent.Stopped, "icon/lbm_off")]
    [InlineData(LittleBigMouseEvent.Dead, "icon/lbm_dead")]
    [InlineData(LittleBigMouseEvent.Paused, "icon/lbm_paused")]
    // A display asleep and an engine stood down look the same from the tray: not running,
    // not broken.
    [InlineData(LittleBigMouseEvent.Suspended, "icon/lbm_paused")]
    public void TheIconSpeaksForTheStatesThatHaveAState(LittleBigMouseEvent daemonEvent, string icon)
        => Assert.Equal(icon, TrayIconController.IconFor(daemonEvent));

    [Theory]
    [InlineData(LittleBigMouseEvent.Resumed)]
    [InlineData(LittleBigMouseEvent.Connected)]
    [InlineData(LittleBigMouseEvent.DisplayChanged)]
    [InlineData(LittleBigMouseEvent.Loaded)]
    [InlineData(LittleBigMouseEvent.Rescued)]
    [InlineData(LittleBigMouseEvent.Probed)]
    public void EventsThatSayNothingAboutTheEngineLeaveTheIconAlone(LittleBigMouseEvent daemonEvent)
        => Assert.Null(TrayIconController.IconFor(daemonEvent));

    [Fact]
    public async Task TheMenuOffersEverythingAndTheIconComesUp()
    {
        var notify = new FakeNotification();
        var options = new LbmOptions();
        using var tray = new TrayIconController(notify, options);

        await tray.InitializeAsync(MenuOver([]));

        Assert.Equal(
            new[] { "Check for update", "Open", "Start", "Stop", "Refresh", "Exit" },
            notify.MenuHeaders);
        Assert.True(notify.Shown);
        Assert.True(notify.Visible);
    }

    [Fact]
    public async Task WhereTheAppCannotUpdateItselfTheEntryIsNotThere()
    {
        // A menu item that no-ops is worse than none: on Linux the distribution package owns
        // updates, and clicking "Check for update" would do nothing at all.
        var notify = new FakeNotification();
        using var tray = new TrayIconController(notify, new LbmOptions());

        await tray.InitializeAsync(MenuOver([], canUpdate: false));

        Assert.DoesNotContain("Check for update", notify.MenuHeaders);
        Assert.Equal("Open", notify.MenuHeaders[0]);
    }

    [Fact]
    public async Task AHiddenTrayIsHiddenBeforeTheIconIsEverLoaded()
    {
        // SetIconAsync queues a dispatcher lambda that checks visibility; applying the
        // preference afterwards means the lambda returns early and the icon is never added.
        var notify = new FakeNotification();
        var options = new LbmOptions { HideTrayIcon = true };
        using var tray = new TrayIconController(notify, options);

        await tray.InitializeAsync(MenuOver([]));

        Assert.False(notify.Visible);
    }

    [Fact]
    public async Task TheHideOptionIsFollowedAfterStartupToo()
    {
        var notify = new FakeNotification();
        var options = new LbmOptions();
        using var tray = new TrayIconController(notify, options);
        await tray.InitializeAsync(MenuOver([]));

        options.HideTrayIcon = true;
        Assert.False(notify.Visible);

        options.HideTrayIcon = false;
        Assert.True(notify.Visible);
    }

    [Fact]
    public async Task ClickingTheIconOpensTheWindow()
    {
        var log = new List<string>();
        var notify = new FakeNotification();
        using var tray = new TrayIconController(notify, new LbmOptions());
        await tray.InitializeAsync(MenuOver(log));

        notify.RaiseClick();

        Assert.Equal(new[] { "open" }, log);
    }

    [Fact]
    public async Task ADisposedTrayStopsListeningToBothTheClickAndTheOption()
    {
        var log = new List<string>();
        var notify = new FakeNotification();
        var options = new LbmOptions();
        var tray = new TrayIconController(notify, options);
        await tray.InitializeAsync(MenuOver(log));

        tray.Dispose();

        notify.RaiseClick();
        options.HideTrayIcon = true;

        Assert.Empty(log);
        Assert.False(notify.HasClickSubscribers);
        Assert.True(notify.Visible); // the option changed and nobody acted on it
    }

    [Fact]
    public async Task AFailingOpenDoesNotTakeTheProcessDownWithIt()
    {
        // The click handler has nowhere to report to. As an async void lambda it used to be an
        // unobserved exception on the UI thread, which is a crash.
        var notify = new FakeNotification();
        using var tray = new TrayIconController(notify, new LbmOptions());
        await tray.InitializeAsync(new TrayMenu(
            CheckUpdateAsync: null,
            OpenAsync: () => Task.FromException(new InvalidOperationException("no window")),
            StartAsync: () => Task.CompletedTask,
            StopAsync: () => Task.CompletedTask,
            RefreshAsync: () => Task.CompletedTask,
            QuitAsync: () => Task.CompletedTask));

        notify.RaiseClick();
        await Task.Delay(50);
    }

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
