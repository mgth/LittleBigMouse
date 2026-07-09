#nullable enable
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
}
