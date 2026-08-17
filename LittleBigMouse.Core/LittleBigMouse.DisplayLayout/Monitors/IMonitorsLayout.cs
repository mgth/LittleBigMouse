#nullable enable
using System;
using System.Collections.ObjectModel;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Monitors;

public interface IMonitorsLayout : IDisposable
{
    ILayoutOptions Options { get; }

    /// <summary>
    /// Origin of this layout. Everything that must not happen on a virtual layout
    /// (persistence, autostart, hooking) keys on this — see <see cref="LayoutSource"/>.
    /// </summary>
    LayoutSource Source { get; }

    /// <summary>Human-readable origin of a virtual layout (file path, "import"…), null for system layouts.</summary>
    string? SourceOrigin { get; }

    /// <summary>True for any layout not built from the machine's actual displays.</summary>
    bool IsVirtual => Source != LayoutSource.System;

    bool Saved { get; set; }

    /// <summary>
    /// 
    /// </summary>
    Rect PhysicalBounds { get; }

    /// <summary>
    /// All physical monitors
    /// </summary>
    ReadOnlyObservableCollection<PhysicalMonitor> PhysicalMonitors { get; }

    /// <summary>
    /// All video sources
    /// </summary>
    ReadOnlyObservableCollection<PhysicalSource> PhysicalSources { get; }

    /// <summary>
    /// 
    /// </summary>
    double X0 { get; }

    /// <summary>
    /// 
    /// </summary>
    double Y0 { get; }

    /// <summary>
    /// Primary display source, or <see langword="null"/> while the layout is empty,
    /// partially built, or between primary sources during reconfiguration.
    /// </summary>
    DisplaySource? PrimarySource { get; }


    string Id { get; set; }

    DpiAwarenessKind DpiAwareness { get; }
    /// <summary>
    /// Physical monitor displaying <see cref="PrimarySource"/>, or
    /// <see langword="null"/> whenever no primary source is currently designated.
    /// </summary>
    PhysicalMonitor? PrimaryMonitor { get; }

    void Compact();
    void ForceCompact();

    void UpdatePhysicalMonitors();
}
