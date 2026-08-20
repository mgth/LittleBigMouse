using System;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// Keeps the wallpaper drawn behind each monitor in step with the desktop's.
/// <para>
/// The platform watches the OS (on Windows, a <c>RegNotifyChangeKeyValue</c> registry watcher)
/// and raises <see cref="ILayoutFactory.WallpaperChanged"/> from a background thread. The
/// refresh is in place — no layout teardown, so edits in progress survive it — and
/// <see cref="ILayoutFactory.UpdateWallpaper"/> gates on a cheap signature, which is what
/// collapses the several registry writes Windows makes per change into a single actual read.
/// </para>
/// <para>
/// A whole class for six lines of work, because the six lines are not the point: the
/// subscription is, and it now has an owner that can end it.
/// </para>
/// </summary>
public sealed class WallpaperRefresher : IDisposable
{
    readonly ILayoutFactory _layoutFactory;
    readonly Func<IMonitorsLayout?> _currentLayout;
    readonly Action<Action> _postToUiThread;
    readonly IDisposable _subscription;

    /// <param name="postToUiThread">
    /// The model is read and written on the UI thread; the event does not arrive on it.
    /// </param>
    public WallpaperRefresher(
        ILayoutFactory layoutFactory,
        Func<IMonitorsLayout?> currentLayout,
        Action<Action> postToUiThread)
    {
        _layoutFactory = layoutFactory;
        _currentLayout = currentLayout;
        _postToUiThread = postToUiThread;

        _subscription = OwnedSubscription.Create<EventHandler>(
            OnWallpaperChanged,
            h => _layoutFactory.WallpaperChanged += h,
            h => _layoutFactory.WallpaperChanged -= h);
    }

    void OnWallpaperChanged(object? sender, EventArgs e) => Refresh();

    /// <summary>Re-read the desktop wallpaper into the current layout, on the UI thread.</summary>
    public void Refresh() => _postToUiThread(() =>
    {
        if (_currentLayout() is MonitorsLayout layout) _layoutFactory.UpdateWallpaper(layout);
    });

    public void Dispose() => _subscription.Dispose();
}
