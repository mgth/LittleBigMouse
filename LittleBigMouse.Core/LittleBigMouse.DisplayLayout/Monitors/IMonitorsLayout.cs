using Avalonia;
using HLab.Sys.Windows.API;
using LittleBigMouse.Zoning;
using System;
using System.Collections.ObjectModel;

namespace LittleBigMouse.DisplayLayout.Monitors;

public interface IMonitorsLayout : IDisposable
{
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
    /// 
    /// </summary>
    DisplaySource PrimarySource { get; }

    /// <summary>
    /// Allow for discontinuities between monitors.
    /// </summary>
    bool AllowDiscontinuity { get; }

    /// <summary>
    /// Allow overlaps between monitors.
    /// </summary>
    bool AllowOverlaps { get; }
    string Id { get; set; }

    WinDef.DpiAwareness DpiAwareness { get; }
    PhysicalMonitor PrimaryMonitor { get; }

    bool Enabled { get; set; }

    bool AutoUpdate { get; }
    bool LoadAtStartup { get; }

    ZonesLayout ComputeZones();
    void Compact();
    void ForceCompact();

    void UpdatePhysicalMonitors();
    bool Schedule();
    void Unschedule();
}