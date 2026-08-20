using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The monitor layouts the border section tests are written against, shared by the
/// editor's arithmetic and by the gesture sequences.
/// </summary>
static class BorderTestLayouts
{
    // Two 1920x1080 monitors, 480x270 mm each, side by side and vertically aligned.
    public static MonitorsLayout TwoMonitors(out PhysicalMonitor left, out PhysicalMonitor right)
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design()) { Id = "TEST" };

        left = AddMonitor(layout, "LEFT", -480, 0);
        right = AddMonitor(layout, "RIGHT", 0, 0);

        return layout;
    }

    public static PhysicalMonitor AddMonitor(
        MonitorsLayout layout, string id, double xMm, double yMm,
        double widthMm = 480, double heightMm = 270)
    {
        var model = new PhysicalMonitorModel($"PNP-{id}");
        model.PhysicalSize.Width = widthMm;
        model.PhysicalSize.Height = heightMm;

        var monitor = new PhysicalMonitor(id, layout, model);
        var source = new DisplaySource($"SRC-{id}") { AttachedToDesktop = true };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource($"DEV-{id}", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);

        monitor.DepthProjection.X = xMm;
        monitor.DepthProjection.Y = yMm;

        return monitor;
    }

    /// <summary>
    /// 540 UI px over a 270 mm edge: 2 px per mm. The band is 16 px thick by default,
    /// so the handle is 8 px — 4 mm — and the mitre 8 mm.
    /// </summary>
    public static BorderSideViewModel RightEdgeOf(PhysicalMonitor monitor, double pixelLength = 540)
        => new(monitor, BorderSideKind.Right, monitor.BorderResistance.Right) { PixelLength = pixelLength };

    public static PhysicalMonitor TwoMonitorsLeft()
    {
        TwoMonitors(out var left, out _);
        return left;
    }
}
