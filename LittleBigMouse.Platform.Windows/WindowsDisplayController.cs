#nullable enable
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

public interface IWindowsDisplayTransaction
{
    bool Stage(DisplaySource source, Rect bounds);
    bool Commit();
    bool Restore();
}

sealed class NativeWindowsDisplayTransaction : IWindowsDisplayTransaction
{
    public bool Stage(DisplaySource source, Rect bounds)
        => MonitorDeviceHelper.AttachToDesktop(source.InterfacePath, source.Primary,
            bounds, source.Orientation, apply: false);

    public bool Commit() => MonitorDeviceHelper.ApplyDesktop();
    public bool Restore() => MonitorDeviceHelper.ResaveCurrentConfiguration();
}

/// <summary>
/// Windows implementation of <see cref="IDisplayController"/>: reads the opaque
/// <c>DisplaySource.InterfacePath</c> (\\?\DISPLAY#…) from the neutral model and delegates
/// to the battle-tested <see cref="MonitorDeviceHelper"/> CCD write-back. Each op self-applies.
/// </summary>
public class WindowsDisplayController : IDisplayController
{
    readonly IWindowsDisplayTransaction _transaction;

    public WindowsDisplayController() : this(new NativeWindowsDisplayTransaction()) { }

    public WindowsDisplayController(IWindowsDisplayTransaction transaction)
        => _transaction = transaction;

    public bool SetPrimary(DisplaySource source)
        => MonitorDeviceHelper.SetPrimary(source.InterfacePath);

    public bool AttachToDesktop(DisplaySource source)
        => MonitorDeviceHelper.AttachToDesktop(
            source.InterfacePath, source.Primary, source.InPixel.Bounds, source.Orientation);

    public bool DetachFromDesktop(DisplaySource source)
        => MonitorDeviceHelper.DetachFromDesktop(source.InterfacePath);

    /// <summary>
    /// Stage every position with NoReset then commit once: the desktop reconfigures a
    /// single time for the whole batch. Scale is ignored — Windows exposes no supported
    /// API to change per-monitor scaling.
    /// </summary>
    public bool SetLocations(IReadOnlyList<(DisplaySource Source, Point Position, double? Scale)> locations)
    {
        var changed = new List<(DisplaySource Source, Point Position)>();
        foreach (var (source, position, _) in locations)
            if (position != source.InPixel.Bounds.Location)
                changed.Add((source, position));

        if (changed.Count == 0) return true;

        if (changed.Any(item => string.IsNullOrWhiteSpace(item.Source.InterfacePath)
            || item.Source.InPixel.Bounds.Width <= 0
            || item.Source.InPixel.Bounds.Height <= 0))
            return false;

        foreach (var (source, position) in changed)
        {
            if (_transaction.Stage(source,
                    new Rect(position, source.InPixel.Bounds.Size))) continue;

            _transaction.Restore();
            return false;
        }

        if (_transaction.Commit()) return true;
        _transaction.Restore();
        return false;
    }
}
