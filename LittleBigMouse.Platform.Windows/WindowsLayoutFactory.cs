#nullable enable
using System;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="ILayoutFactory"/>: reads the Win32 monitor device
/// tree and builds the neutral <see cref="MonitorsLayout"/> model directly — no
/// intermediate data layer. This is the former UI <c>LayoutFactory</c>, moved to the
/// Windows platform project and formalized behind the seam. A Linux factory builds the same
/// model from RandR/DRM.
/// <para>
/// The class is only the construction façade and the wallpaper-polling gate: the Win32
/// collect-and-assemble is <see cref="WindowsLayoutBuilder"/>, the per-monitor field mapping
/// <see cref="WindowsSourceMapper"/>, the physical-size inference <see cref="WindowsPhysicalSize"/>,
/// the wallpaper copy into the model <see cref="WindowsWallpaperMapping"/> and the registry/file
/// read behind it <see cref="WindowsWallpaperReader"/>.
/// </para>
/// </summary>
public class WindowsLayoutFactory : ILayoutFactory, IDisposable
{
    readonly ISystemMonitorsService _monitors;
    readonly Func<MonitorsLayout> _newLayout;
    readonly ILayoutPersistence _persistence;
    readonly WindowsWallpaperWatcher _wallpaperWatcher;
    string _lastWallpaperSignature = "";

    public WindowsLayoutFactory(ISystemMonitorsService monitors, Func<MonitorsLayout> newLayout, ILayoutPersistence persistence)
    {
        _monitors = monitors;
        _newLayout = newLayout;
        _persistence = persistence;

        _wallpaperWatcher = new WindowsWallpaperWatcher();
        _wallpaperWatcher.Changed += (_, _) => WallpaperChanged?.Invoke(this, EventArgs.Empty);
    }

    // Never raised on Windows: display changes arrive through the daemon's DisplayChanged
    // event (the daemon owns the unhook-on-change semantics there).
    public event EventHandler? DisplayChanged { add { } remove { } }

    public event EventHandler? WallpaperChanged;

    // Idempotent through the watcher's own guard; the factory is a DI singleton disposed once at
    // container teardown, but hardening here keeps a double-dispose from ever reaching a handle.
    public void Dispose() => _wallpaperWatcher.Dispose();

    public MonitorsLayout Create()
    {
        // Refresh the OS device tree (formerly MainService's UpdateDevices()).
        if (_monitors is SystemMonitorsService concrete) concrete.UpdateDevices();
        return _newLayout().UpdateFrom(_monitors, _persistence);
    }

    /// <inheritdoc/>
    public string DisplaySignature() => MonitorDeviceHelper.DisplaySignature();

    public void UpdateWallpaper(MonitorsLayout layout)
    {
        // Cheap gate: compute a light signature (registry values + the transcoded-wallpaper file
        // timestamp) and only read the OS wallpaper / device tree when it actually changed. This
        // lets the UI poll this method frequently (while the config window is open) at near-zero
        // cost, which is the reliable trigger — the daemon's WM_SETTINGCHANGE broadcast is missed
        // intermittently (shared message pump), so it is only a best-effort fast path.
        var signature = WindowsWallpaperReader.ReadSignature();
        if (signature == _lastWallpaperSignature) return;
        _lastWallpaperSignature = signature;

        layout.UpdateWallpaper(_monitors);
    }
}
