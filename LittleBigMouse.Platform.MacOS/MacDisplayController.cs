#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.MacOS;

/// <summary>
/// macOS implementation of <see cref="IDisplayController"/> through the
/// CoreGraphics display configuration API — the same thing System Settings →
/// Displays → Arrange does. <c>DisplaySource.InterfacePath</c> holds the name
/// <see cref="MacDisplays.NameOf"/> gave the display.
/// <para>
/// Enabling or disabling a display has no public API on macOS (mirroring is the
/// closest thing), so attach/detach report "not done". Scale changes are not
/// applied either: a scale on macOS is a display mode choice, left to the user.
/// </para>
/// </summary>
public class MacDisplayController : IDisplayController
{
    readonly MacLayoutFactory _factory;

    public MacDisplayController(MacLayoutFactory factory)
    {
        _factory = factory;
    }

    /// <summary>
    /// The main display is the one at the origin: translate the whole arrangement
    /// so that <paramref name="source"/> sits at (0,0). That also moves the menu bar.
    /// </summary>
    public bool SetPrimary(DisplaySource source)
    {
        var target = _factory.QueryMonitors().FirstOrDefault(m => m.ConnectorName == source.InterfacePath);
        if (target == null) return false;

        var dx = (int)Math.Round(target.LogicalX);
        var dy = (int)Math.Round(target.LogicalY);
        if (dx == 0 && dy == 0) return true;

        var origins = new List<(uint, int, int)>();
        foreach (var m in _factory.QueryMonitors())
        {
            if (MacDisplays.Find(m.ConnectorName) is not { } id) continue;
            origins.Add((id, (int)Math.Round(m.LogicalX) - dx, (int)Math.Round(m.LogicalY) - dy));
        }
        return Notify(MacDisplays.SetOrigins(origins));
    }

    public bool AttachToDesktop(DisplaySource source) => false;

    public bool DetachFromDesktop(DisplaySource source) => false;

    public bool SetLocations(IReadOnlyList<(DisplaySource Source, Point Position, double? Scale)> locations)
    {
        if (locations.Count == 0) return true;

        var origins = new List<(uint, int, int)>();
        foreach (var (source, position, _) in locations)
        {
            if (MacDisplays.Find(source.InterfacePath) is not { } id) continue;
            if (position == source.InPixel.Bounds.Location) continue;
            origins.Add((id, (int)Math.Round(position.X), (int)Math.Round(position.Y)));
        }

        if (origins.Count == 0) return true;
        return Notify(MacDisplays.SetOrigins(origins));
    }

    bool Notify(bool success)
    {
        if (success) _factory.NotifyDisplayChanged();
        return success;
    }
}
