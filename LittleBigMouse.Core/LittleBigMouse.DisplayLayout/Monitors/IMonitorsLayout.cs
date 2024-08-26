using Avalonia;
using HLab.Sys.Windows.API;
using System;
using System.Collections.ObjectModel;

namespace LittleBigMouse.DisplayLayout.Monitors;

public interface IMonitorsLayout : IDisposable
{
    ILayoutOptions Options { get; }

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


    string Id { get; set; }

    WinDef.DpiAwareness DpiAwareness { get; }
    PhysicalMonitor PrimaryMonitor { get; }

    void Compact();
    void ForceCompact();

    void UpdatePhysicalMonitors();
    bool Schedule(bool elevated);
    void Unschedule();
}