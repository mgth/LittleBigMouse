using System.Collections.Generic;
using HLab.Geo;
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

    /// <summary>
    /// Adjust the desktop topology so the crossing engine can take control, called right
    /// before the engine starts. Windows needs nothing (no-op); Linux/KWin opens 1px
    /// logical gaps between contiguous outputs so the daemon's pointer barriers pass the
    /// compositor's validator (which only accepts barriers on outer edges).
    /// Returns true when the topology actually changed — zones computed before the call
    /// are then stale and must not be sent to the daemon.
    /// </summary>
    bool PrepareForEngine() => false;

    /// <summary>Undo <see cref="PrepareForEngine"/> once the engine stops.</summary>
    void RestoreAfterEngine() { }

    /// <summary>
    /// Apply new pixel positions (and optional per-output scales, where the platform
    /// supports it) as one topology change: the whole batch is staged then committed,
    /// so intermediate states never reach the OS. Positions are in the platform's
    /// positioning space (logical pixels on Wayland, physical pixels on Windows).
    /// </summary>
    bool SetLocations(IReadOnlyList<(DisplaySource Source, Point Position, double? Scale)> locations) => false;
}
