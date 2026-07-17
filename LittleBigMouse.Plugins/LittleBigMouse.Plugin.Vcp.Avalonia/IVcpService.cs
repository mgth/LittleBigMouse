#nullable enable
using HLab.Sys.Windows.MonitorVcp;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Platform seam: resolve the DDC/CI channel of a monitor from the layout.
/// Windows walks the Win32 device tree to a dxva2 handle
/// (<see cref="WindowsVcpService"/>), Linux maps the monitor's connector to an
/// I2C bus through ddcutil (<see cref="DdcUtilVcpService"/>).
/// </summary>
public interface IVcpService
{
    /// <summary>Null when the monitor has no reachable DDC/CI channel.</summary>
    VcpControl? GetControl(PhysicalMonitor monitor);

    /// <summary>Resolve the physical channel without blocking the UI thread.</summary>
    Task<VcpControl?> GetControlAsync(
        PhysicalMonitor monitor,
        CancellationToken cancellationToken = default);
}
