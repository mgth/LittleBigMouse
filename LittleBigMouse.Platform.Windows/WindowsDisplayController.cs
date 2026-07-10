#nullable enable
using System.Collections.Generic;
using HLab.Geo;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="IDisplayController"/>: reads the opaque
/// <c>DisplaySource.InterfacePath</c> (\\?\DISPLAY#…) from the neutral model and delegates
/// to the battle-tested <see cref="MonitorDeviceHelper"/> CCD write-back. Each op self-applies.
/// </summary>
public class WindowsDisplayController : IDisplayController
{
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

        var ok = true;
        foreach (var (source, position) in changed)
            ok &= MonitorDeviceHelper.AttachToDesktop(
                source.InterfacePath, source.Primary,
                new Rect(position, source.InPixel.Bounds.Size), source.Orientation,
                apply: false);

        MonitorDeviceHelper.ApplyDesktop();
        return ok;
    }
}
