#nullable enable
using System;
using System.Linq;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="IMonitorInfoService"/>: shows where the layout
/// model's values came from — the KScreen/XRandR output on one side, the sysfs EDID
/// on the other. The monitor is matched by connector name (that is its DeviceId).
/// </summary>
public class LinuxMonitorInfoService : IMonitorInfoService
{
    readonly LinuxLayoutFactory _factory;

    public LinuxMonitorInfoService(LinuxLayoutFactory factory)
    {
        _factory = factory;
    }

    public void DisplayValues(PhysicalMonitor monitor, Action<string, string, Action?, bool> addValue)
    {
        var output = _factory.QueryMonitors().FirstOrDefault(m => m.ConnectorName == monitor.DeviceId);
        if (output == null)
        {
            addValue("Connector", monitor.DeviceId ?? "?", null, false);
            addValue("", "output not found by the discovery backend", null, false);
            return;
        }

        addValue("", $"Output ({_factory.SourceName})", null, true);
        addValue("Connector", output.ConnectorName, null, false);
        addValue("Enabled", output.Enabled ? "yes" : "no", null, false);
        addValue("Primary", output.Primary ? "yes" : "no", null, false);
        addValue("Mode", $"{output.PixelWidth} x {output.PixelHeight} @ {output.Frequency} Hz", null, false);
        addValue("Logical geometry", $"{output.LogicalWidth} x {output.LogicalHeight} + {output.LogicalX} + {output.LogicalY}", null, false);
        addValue("Scale", $"{output.Scale}", null, false);
        addValue("Rotation", $"{output.Orientation * 90}°", null, false);
        addValue("Size (reported)", $"{output.WidthMm} x {output.HeightMm} mm", null, false);

        var edid = output.Edid;
        if (edid == null)
        {
            addValue("", "EDID (none in /sys/class/drm)", null, true);
            return;
        }

        addValue("", "EDID (/sys/class/drm)", null, true);
        addValue("Manufacturer", edid.ManufacturerCode, null, false);
        addValue("Product code", edid.ProductCode, null, false);
        addValue("Model", edid.Model, null, false);
        addValue("Serial", edid.Serial, null, false);
        addValue("Serial number", edid.SerialNumber, null, false);
        addValue("Physical size", $"{edid.PhysicalWidth} x {edid.PhysicalHeight} mm", null, false);
        addValue("Manufactured", $"week {edid.Week}, {edid.Year}", null, false);
        addValue("Version", edid.Version, null, false);
        addValue("Interface", $"{edid.VideoInterface} ({(edid.Digital ? "digital" : "analog")}, {edid.BitDepth} bpc)", null, false);
        addValue("Gamma", $"{edid.Gamma}", null, false);
    }
}
