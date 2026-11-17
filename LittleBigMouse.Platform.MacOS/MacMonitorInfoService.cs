#nullable enable
using System;
using System.Linq;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.MacOS;

/// <summary>
/// macOS implementation of <see cref="IMonitorInfoService"/>: what CoreGraphics
/// reported for the display behind a monitor (matched by its DeviceId, which is
/// the name <see cref="MacDisplays.NameOf"/> gave it).
/// </summary>
public class MacMonitorInfoService : IMonitorInfoService
{
    readonly MacLayoutFactory _factory;

    public MacMonitorInfoService(MacLayoutFactory factory)
    {
        _factory = factory;
    }

    public void DisplayValues(PhysicalMonitor monitor, Action<string, string, Action?, bool> addValue)
    {
        var output = _factory.QueryMonitors().FirstOrDefault(m => m.ConnectorName == monitor.DeviceId);
        if (output == null)
        {
            addValue("Display", monitor.DeviceId ?? "?", null, false);
            addValue("", "display not reported by CoreGraphics", null, false);
            return;
        }

        addValue("", "Display (CoreGraphics)", null, true);
        addValue("Identifier", output.ConnectorName, null, false);
        addValue("Main display", output.Primary ? "yes" : "no", null, false);
        addValue("Mode", $"{output.PixelWidth} x {output.PixelHeight} @ {output.Frequency} Hz", null, false);
        addValue("Bounds (points)", $"{output.LogicalWidth} x {output.LogicalHeight} + {output.LogicalX} + {output.LogicalY}", null, false);
        addValue("Backing scale", $"{output.Scale}", null, false);
        addValue("Rotation", $"{output.Orientation * 90}°", null, false);
        addValue("Size (reported)", $"{output.WidthMm} x {output.HeightMm} mm", null, false);
    }
}
