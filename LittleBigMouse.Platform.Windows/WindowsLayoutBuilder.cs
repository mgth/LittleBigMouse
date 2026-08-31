#nullable enable
using System.Linq;
using DynamicData;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Walks the Win32 monitor device tree and builds the neutral <see cref="MonitorsLayout"/>
/// model — the collect-and-assemble step. It owns the ordering of sources, the creation of
/// models/monitors/sources and their de-duplication (clones map to extra sources of the same
/// monitor), then hands off to the shared persistence load and placement. The per-monitor
/// field mapping lives in <see cref="WindowsSourceMapper"/> and the physical-size inference
/// in <see cref="WindowsPhysicalSize"/>; this class only orchestrates them.
/// </summary>
internal static class WindowsLayoutBuilder
{
    public static MonitorsLayout UpdateFrom(this MonitorsLayout layout, ISystemMonitorsService service, ILayoutPersistence persistence)
    {
        // DPI awareness is process-scoped (the UI thread's manifested awareness), formerly
        // computed in the MonitorsLayout constructor. Set it before building the sources.
        layout.DpiAwareness = (DpiAwarenessKind)(int)WinUser.GetAwarenessFromDpiAwarenessContext(
            WinUser.GetThreadDpiAwarenessContext());

        // First access to .Root triggers the (lazy, expensive) GetDisplayDevices enumeration.
        // Hold a strong ref for the whole method so the WeakReference can't drop it mid-build.
        var root = service.Root;

        foreach (var monitor in root.AllMonitorDevices())
        {
            // Specialized displays (VR headsets...) are hidden from the desktop by
            // Windows: keep them out of the layout too, the cursor can never reach
            // them and their zone would only trap it (#364)
            if (monitor.IsSpecialized) continue;

            layout.AddOrUpdateMonitorDevice(monitor);
        }
        layout.Id = layout.ComputeId();

        //retrieve saved layout (registry, via the shared persistence engine)
        persistence.Load(layout);

        // Place the monitors the stored layout did not cover (new or never-saved config)
        // from the windows configuration. AFTER Load, so the placement runs on the fully
        // loaded state (stored model sizes, borders, neighbor positions) — exactly what the
        // "place from windows" button does; running it before Load placed with default
        // models and the first appearance of a config differed from the button result.
        layout.SetLocationsFromSystemConfiguration(placeAll: false);

        // saved locations are anchored on the primary that was active at save time:
        // re-anchor so the current primary sits at (0,0) mm, like in pixels
        layout.AnchorOnPrimary();

        return layout;
    }

    /// <summary>
    /// Map one <see cref="MonitorDevice"/> into the layout: refresh an existing source, or
    /// create the monitor/model/source (a clone of an already-seen monitor becomes an extra
    /// source of that same <see cref="PhysicalMonitor"/>).
    /// </summary>
    static void AddOrUpdateMonitorDevice(this MonitorsLayout layout, MonitorDevice monitor)
    {
        var source = layout.PhysicalSources.FirstOrDefault(s => s.DeviceId == monitor.Id);

        if (source != null)
        {
            source.Source.UpdateFrom(monitor);
            return;
        }

        var id = monitor.SourceId;

        var physicalMonitor = layout.PhysicalMonitors.FirstOrDefault(m => m.Id == id);

        if (physicalMonitor == null)
        {
            // first get the monitor model, it defines physical size
            var model = layout.GetOrAddPhysicalMonitorModel(monitor.PnpCode, s => monitor.CreatePhysicalMonitorModel(s));

            physicalMonitor = monitor.CreatePhysicalMonitor(id, layout, model);

            source = new PhysicalSource(monitor.Id, physicalMonitor, monitor.CreateDisplaySource());

            physicalMonitor.ActiveSource = source;
            physicalMonitor.Sources.Add(source);

            layout.AddOrUpdatePhysicalMonitor(physicalMonitor);
        }
        else
        {
            // new source for an existing monitor
            source = new PhysicalSource(monitor.Id, physicalMonitor, monitor.CreateDisplaySource());
            physicalMonitor.Sources.Add(source);
        }

        layout.AddOrUpdatePhysicalSource(source);
    }
}
