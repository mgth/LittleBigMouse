#nullable enable
using System;
using System.Collections.Generic;
using System.Threading;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Platform.Linux;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.MacOS;

/// <summary>
/// macOS implementation of <see cref="ILayoutFactory"/>: discovery through
/// <see cref="MacDisplays"/> (CoreGraphics), mapped onto the model by the mapping
/// the Linux platform already uses. Display changes are picked up by polling the
/// display signature — the daemon reports them too, but the UI must also notice
/// while the engine is stopped and no daemon runs.
/// </summary>
public class MacLayoutFactory : ILayoutFactory, IDisposable
{
    static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(2);

    readonly Func<MonitorsLayout> _newLayout;
    readonly ILayoutPersistence _persistence;
    readonly Timer _pollTimer;
    string _lastSignature;

    public MacLayoutFactory(Func<MonitorsLayout> newLayout, ILayoutPersistence persistence)
    {
        _newLayout = newLayout;
        _persistence = persistence;
        _lastSignature = SafeSignature();
        _pollTimer = new Timer(_ => Poll(), null, PollInterval, PollInterval);
    }

    public event EventHandler? DisplayChanged;

    // Reading the desktop picture needs AppKit (NSWorkspace); not wired yet.
    public event EventHandler? WallpaperChanged { add { } remove { } }

    public void UpdateWallpaper(MonitorsLayout layout) { }

    public void Dispose() => _pollTimer.Dispose();

    /// <summary>Raise <see cref="DisplayChanged"/> after a topology change made from the UI.</summary>
    public void NotifyDisplayChanged() => DisplayChanged?.Invoke(this, EventArgs.Empty);

    public IReadOnlyList<LinuxMonitor> QueryMonitors() => MacDisplays.Query();

    void Poll()
    {
        var signature = SafeSignature();
        if (signature == _lastSignature) return;
        _lastSignature = signature;
        DisplayChanged?.Invoke(this, EventArgs.Empty);
    }

    static string SafeSignature()
    {
        try { return MacDisplays.Signature(); }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[macOS] display query failed: {ex.Message}");
            return "";
        }
    }

    public MonitorsLayout Create()
    {
        var layout = _newLayout();

        List<LinuxMonitor> monitors;
        try { monitors = MacDisplays.Query(); }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[macOS] display query failed: {ex.Message}");
            monitors = [];
        }

        if (monitors.Count == 0)
        {
            // Never open the editor empty: a plausible single display instead.
            monitors = [new LinuxMonitor
            {
                ConnectorName = "FALLBACK",
                LogicalWidth = 1920, LogicalHeight = 1080,
                PixelWidth = 1920, PixelHeight = 1080,
                WidthMm = 527, HeightMm = 296,
                Primary = true
            }];
        }

        foreach (var monitor in monitors)
            layout.AddMonitor(monitor);

        layout.Id = layout.ComputeId();

        // Same order as Windows and Linux: saved layout first, then place what it did
        // not cover from the system arrangement, then re-anchor on the main display.
        _persistence.Load(layout);
        layout.SetLocationsFromSystemConfiguration(placeAll: false);
        layout.AnchorOnPrimary();

        return layout;
    }

    public string DisplaySignature() => SafeSignature();
}
