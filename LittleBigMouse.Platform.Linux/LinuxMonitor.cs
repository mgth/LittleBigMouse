#nullable enable
using System.Collections.Generic;
using HLab.Sys.Monitors;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// One connected output as seen by a <see cref="ILinuxMonitorSource"/>, in the neutral shape
/// the layout mapping consumes. <see cref="LogicalBounds"/> is the compositor's coordinate
/// space — the space the cursor actually lives in under Wayland (KScreen positions are
/// logical; on native X11 logical == pixels). <see cref="PixelWidth"/>/<see cref="PixelHeight"/>
/// are the current mode, already swapped when the output is rotated.
/// </summary>
public record LinuxMonitor
{
    public required string ConnectorName { get; init; }

    public double LogicalX { get; init; }
    public double LogicalY { get; init; }
    public double LogicalWidth { get; init; }
    public double LogicalHeight { get; init; }

    public int PixelWidth { get; init; }
    public int PixelHeight { get; init; }

    public double Scale { get; init; } = 1.0;

    /// <summary>Physical size in millimeters as reported by the source (EDID-derived).</summary>
    public double WidthMm { get; init; }
    public double HeightMm { get; init; }

    public bool Primary { get; init; }
    public bool Enabled { get; init; } = true;

    /// <summary>0=0°, 1=90°, 2=180°, 3=270° (model convention).</summary>
    public int Orientation { get; init; }

    public int Frequency { get; init; }

    /// <summary>Parsed EDID when the connector could be matched in /sys/class/drm.</summary>
    public Edid? Edid { get; init; }
}

/// <summary>
/// A way to enumerate the current outputs. Implementations are probed in order:
/// KScreen (KDE, Wayland or X11) then XRandR (any X11 session).
/// </summary>
public interface ILinuxMonitorSource
{
    string Name { get; }
    bool IsAvailable();
    List<LinuxMonitor> Query();
}
