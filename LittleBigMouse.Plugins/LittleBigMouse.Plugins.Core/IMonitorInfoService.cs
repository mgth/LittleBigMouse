#nullable enable
using System;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

/// <summary>
/// Platform seam: dumps the raw platform-discovery data behind a monitor for the
/// "Info" view — where each value the layout model was built from came from.
/// Windows walks the Win32 device tree (EnumDisplaySettings, EDID registry blocks,
/// CCD paths); Linux reports the KScreen/XRandR output and the sysfs EDID.
/// </summary>
public interface IMonitorInfoService
{
    /// <summary>
    /// Emit (name, value, optional click handler, isTitle) rows for the monitor.
    /// </summary>
    void DisplayValues(PhysicalMonitor monitor, Action<string, string, Action?, bool> addValue);
}
