using Avalonia;
using Avalonia.Media;
using DynamicData;
using HLab.Sys.Windows.API;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Monitors;

public interface IMonitorsLayout
{
    bool Saved { get; set; }
    Rect PhysicalBounds { get; }
    SourceCache<PhysicalMonitor, string> PhysicalMonitors { get; }

    SourceCache<PhysicalSource, string> AllSources { get; }

    double X0 { get; }
    double Y0 { get; }

    DisplaySource PrimarySource { get; }
    bool AllowDiscontinuity { get; }
    bool AllowOverlaps { get; }
    string Id { get; set; }

    WinDef.DpiAwareness DpiAwareness { get; }
    PhysicalMonitor PrimaryMonitor { get; }

    WallpaperStyle WallpaperStyle { get; }

    ZonesLayout ComputeZones();
    void Compact(bool force = false);

    void UpdatePhysicalMonitors();
}