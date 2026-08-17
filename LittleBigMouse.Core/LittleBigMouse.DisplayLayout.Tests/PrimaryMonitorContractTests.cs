using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

namespace LittleBigMouse.DisplayLayout.Tests;

public class PrimaryMonitorContractTests
{
    static (PhysicalMonitor Monitor, DisplaySource Source, PhysicalSource PhysicalSource) CreateMonitor(
        IMonitorsLayout layout, string id, bool primary)
    {
        var model = new PhysicalMonitorModel($"PNP_{id}");
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;

        var monitor = new PhysicalMonitor(id, layout, model);
        var source = new DisplaySource($"{id}-source")
        {
            AttachedToDesktop = true,
            Primary = primary,
        };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource($"{id}-device", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        return (monitor, source, physicalSource);
    }

    static (PhysicalMonitor Monitor, DisplaySource Source) AddMonitor(
        MonitorsLayout layout, string id, bool primary)
    {
        var (monitor, source, physicalSource) = CreateMonitor(layout, id, primary);
        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);

        return (monitor, source);
    }

    [Fact]
    public void EmptyLayoutHasNoPrimary()
    {
        IMonitorsLayout layout = new MonitorsLayout(new ILayoutOptions.Design());

        Assert.Empty(layout.PhysicalMonitors);
        Assert.Null(layout.PrimaryMonitor);
        Assert.Null(layout.PrimarySource);
        Assert.Empty(layout.ComputePixelLocationsFromPhysical());
    }

    [Fact]
    public void PartiallyBuiltLayoutHasNoPrimaryAndPlacementIsIgnored()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        var (monitor, _, _) = CreateMonitor(layout, "PENDING", primary: true);
        layout.AddOrUpdatePhysicalMonitor(monitor);
        var before = monitor.DepthProjection.Bounds;

        Assert.Single(layout.PhysicalMonitors);
        Assert.Empty(layout.PhysicalSources);
        Assert.Null(layout.PrimaryMonitor);
        Assert.Null(layout.PrimarySource);

        layout.AnchorOnPrimary();
        layout.SetLocationsFromSystemConfiguration();
        layout.ForceCompact();

        Assert.Equal(before, monitor.DepthProjection.Bounds);
    }

    [Fact]
    public void DesignatingPrimaryUpdatesBothProperties()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        var (monitor, source) = AddMonitor(layout, "MONITOR", primary: false);

        source.Primary = true;

        Assert.Same(monitor, layout.PrimaryMonitor);
        Assert.Same(source, layout.PrimarySource);
    }

    [Fact]
    public void ReplacingPrimaryAllowsTheTransientStateAndSelectsTheReplacement()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        var (oldMonitor, oldSource) = AddMonitor(layout, "OLD", primary: true);
        var (newMonitor, newSource) = AddMonitor(layout, "NEW", primary: false);

        Assert.Same(oldMonitor, layout.PrimaryMonitor);
        Assert.Same(oldSource, layout.PrimarySource);

        oldSource.Primary = false;

        Assert.Null(layout.PrimaryMonitor);
        Assert.Null(layout.PrimarySource);

        newSource.Primary = true;

        Assert.Same(newMonitor, layout.PrimaryMonitor);
        Assert.Same(newSource, layout.PrimarySource);
    }
}
