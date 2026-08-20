using System;
using System.ComponentModel;
using System.Reactive.Disposables;
using System.Threading.Tasks;
using HLab.UserNotification;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// What the tray menu can do. Each entry is the action itself, not a service to look it up
/// on: the tray shows what it was handed and knows nothing about layouts, daemons or updaters.
/// </summary>
/// <param name="CheckUpdateAsync">
/// Null where the app cannot update itself (Linux: the distribution package owns updates), and
/// then the entry is not shown at all — a menu item that no-ops is worse than none.
/// </param>
public sealed record TrayMenu(
    Func<Task>? CheckUpdateAsync,
    Func<Task> OpenAsync,
    Func<Task> StartAsync,
    Func<Task> StopAsync,
    Func<Task> RefreshAsync,
    Func<Task> QuitAsync);

/// <summary>
/// The tray icon: its menu, its visibility, and the one thing it says about the engine — which
/// of the four states it is in. Everything it reacts to (a click, the "hide tray icon" option)
/// is held as a subscription it can give back.
/// </summary>
public sealed class TrayIconController(IUserNotificationService notify, ILayoutOptions options)
    : IDisposable
{
    readonly CompositeDisposable _subscriptions = [];

    /// <summary>
    /// The icon that stands for a daemon state, or null for the events the icon says nothing
    /// about. Suspended shares the paused icon: from the tray's point of view a display that
    /// went to sleep and an engine that stood down look the same — not running, not broken.
    /// </summary>
    public static string? IconFor(LittleBigMouseEvent daemonEvent) => daemonEvent switch
    {
        LittleBigMouseEvent.Running => "icon/lbm_on",
        LittleBigMouseEvent.Stopped => "icon/lbm_off",
        LittleBigMouseEvent.Dead => "icon/lbm_dead",
        LittleBigMouseEvent.Paused or LittleBigMouseEvent.Suspended => "icon/lbm_paused",
        _ => null
    };

    /// <summary>Reflect a daemon state, if it is one the icon has something to say about.</summary>
    public Task ShowDaemonStateAsync(LittleBigMouseEvent daemonEvent)
        => IconFor(daemonEvent) is { } icon ? notify.SetIconAsync(icon, 32) : Task.CompletedTask;

    /// <summary>Build the menu, wire the click and the visibility option, and show the icon.</summary>
    public async Task InitializeAsync(TrayMenu menu)
    {
        void OnClick(object sender, object args) => FireAndForget(menu.OpenAsync, "Tray icon click");

        _subscriptions.Add(OwnedSubscription.Create<Action<object, object>>(
            OnClick, h => notify.Click += h, h => notify.Click -= h));

        // Apply / react to the "hide tray icon" option. The notify service hides the tray icon
        // natively (Avalonia TrayIcon.IsVisible) — no per-OS code here.
        void OnOptionChanged(object? sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName == nameof(ILayoutOptions.HideTrayIcon)) ApplyVisibility();
        }

        _subscriptions.Add(OwnedSubscription.Create<PropertyChangedEventHandler>(
            OnOptionChanged, h => options.PropertyChanged += h, h => options.PropertyChanged -= h));

        if (menu.CheckUpdateAsync is { } checkUpdate)
            await notify.AddMenuAsync(-1, "Check for update", "Icon/lbm_on", checkUpdate);

        await notify.AddMenuAsync(-1, "Open", "Icon/lbm_off", menu.OpenAsync);
        await notify.AddMenuAsync(-1, "Start", "Icon/Start", menu.StartAsync);
        await notify.AddMenuAsync(-1, "Stop", "Icon/Stop", menu.StopAsync);
        await notify.AddMenuAsync(-1, "Refresh", "Icon/Refresh", menu.RefreshAsync);
        await notify.AddMenuAsync(-1, "Exit", "Icon/sys/Close", menu.QuitAsync);

        // Apply the initial "hide tray icon" preference BEFORE loading the icon bitmap.
        // SetIconAsync queues a dispatcher lambda that checks _visible; if Visible=false
        // is already set here, the lambda returns early and NIM_ADD never fires.
        ApplyVisibility();

        await notify.SetIconAsync("Icon/lbm_off", 128);

        notify.Show();
    }

    void ApplyVisibility() => notify.Visible = !options.HideTrayIcon;

    /// <summary>
    /// A tray click has nowhere to report a failure to, and an <c>async void</c> handler would
    /// take the process down with it — which is what the click used to be.
    /// <para>
    /// Surviving it costs something though: from the user's side a click that fails now looks
    /// exactly like a click that did nothing. So this goes to <see cref="Console.Error"/>, where
    /// the app's other startup diagnostics go and where a Release build still has it, rather
    /// than to <c>Debug.WriteLine</c>, which would leave nothing at all behind.
    /// </para>
    /// </summary>
    static void FireAndForget(Func<Task> action, string what)
    {
        _ = Run();
        return;

        async Task Run()
        {
            try
            {
                await action();
            }
            catch (Exception error)
            {
                Console.Error.WriteLine($"{what} failed: {error}");
            }
        }
    }

    public void Dispose() => _subscriptions.Dispose();
}
