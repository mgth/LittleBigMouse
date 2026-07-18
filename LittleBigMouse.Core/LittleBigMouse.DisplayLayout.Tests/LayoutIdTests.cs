using DynamicData;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The layout persistence key must distinguish orientations (a layout saved in landscape
/// reloaded onto a portrait geometry is disjoint) while keeping the historical key —
/// no suffix — for non-rotated setups, so existing stored configs stay reachable.
/// </summary>
public class LayoutIdTests
{
    static MonitorsLayout NewLayout(params (string id, int orientation)[] monitors)
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        foreach (var (id, orientation) in monitors)
        {
            var model = new PhysicalMonitorModel("TST1234");
            model.PhysicalSize.Width = 600;
            model.PhysicalSize.Height = 340;

            var monitor = new PhysicalMonitor(id, layout, model);
            var source = new DisplaySource($"SRC_{id}") { AttachedToDesktop = true, Orientation = orientation };

            var physicalSource = new PhysicalSource($"DEV_{id}", monitor, source);
            monitor.ActiveSource = physicalSource;
            monitor.Sources.Add(physicalSource);

            layout.AddOrUpdatePhysicalMonitor(monitor);
            layout.AddOrUpdatePhysicalSource(physicalSource);
        }

        return layout;
    }

    [Fact]
    public void ComputeId_Landscape_KeepsLegacyKey()
        => Assert.Equal("MONA+MONB", NewLayout(("MONB", 0), ("MONA", 0)).ComputeId());

    [Fact]
    public void ComputeId_RotatedMonitor_GetsOrientationSuffix()
        => Assert.Equal("MONA_1+MONB", NewLayout(("MONA", 1), ("MONB", 0)).ComputeId());

    [Fact]
    public void ComputeId_ChangesWhenRotationChanges()
    {
        var layout = NewLayout(("MONA", 0), ("MONB", 0));
        var before = layout.ComputeId();

        layout.PhysicalMonitors.First(m => m.Id == "MONA").ActiveSource.Source.Orientation = 1;

        Assert.NotEqual(before, layout.ComputeId());
    }
}
