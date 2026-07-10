#nullable enable
using System.Diagnostics;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="IDisplayController"/>. Phase 1: no-op — the layout
/// builder only needs discovery. Phase 2 maps these onto kscreen-doctor (KDE) or xrandr.
/// </summary>
public class LinuxDisplayController : IDisplayController
{
    public bool SetPrimary(DisplaySource source)
    {
        Debug.WriteLine($"LinuxDisplayController.SetPrimary({source.InterfacePath}): not implemented");
        return false;
    }

    public bool AttachToDesktop(DisplaySource source)
    {
        Debug.WriteLine($"LinuxDisplayController.AttachToDesktop({source.InterfacePath}): not implemented");
        return false;
    }

    public bool DetachFromDesktop(DisplaySource source)
    {
        Debug.WriteLine($"LinuxDisplayController.DetachFromDesktop({source.InterfacePath}): not implemented");
        return false;
    }
}
