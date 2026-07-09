using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

/// <summary>
/// Platform seam: applies desktop-topology changes for a given display source. Operates on
/// the neutral model (<see cref="DisplaySource"/>); the platform implementation reads
/// whatever opaque handle it needs (Windows: <c>DisplaySource.InterfacePath</c>) and issues
/// the native call. Each operation self-applies (commits to the OS).
/// </summary>
public interface IDisplayController
{
    /// <summary>Make the source's monitor the primary display.</summary>
    bool SetPrimary(DisplaySource source);

    /// <summary>Attach the source's monitor to the desktop at its current position/mode.</summary>
    bool AttachToDesktop(DisplaySource source);

    /// <summary>Detach the source's monitor from the desktop.</summary>
    bool DetachFromDesktop(DisplaySource source);
}
