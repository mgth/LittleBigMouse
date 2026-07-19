using HLab.Sys.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Linux;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Model.PhysicalSize must receive the panel's INTRINSIC size: the rotation is applied
/// downstream (PhysicalRotated / DepthProjection), so feeding an oriented size gets it
/// transposed twice and a portrait display is drawn landscape (#511, the Linux twin of
/// the Windows #507). Both monitor sources (KScreen, xrandr) report ORIENTED millimeters,
/// while the EDID never rotates.
/// </summary>
public class LinuxLayoutMappingTests
{
    static LinuxMonitor Portrait(Edid? edid) => new()
    {
        ConnectorName = "DP-1",
        Orientation = 1,
        // Oriented values, as both sources report them for a left-rotated 27" QHD.
        PixelWidth = 1440,
        PixelHeight = 2560,
        LogicalWidth = 1440,
        LogicalHeight = 2560,
        WidthMm = 336,
        HeightMm = 597,
        Primary = true,
        Edid = edid
    };

    [Fact]
    public void PortraitMonitorWithEdid_ModelKeepsIntrinsicSize_AndRotatedIsPortrait()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        layout.AddMonitor(Portrait(new Edid
        {
            ManufacturerCode = "SAM",
            ProductCode = "1234",
            Serial = "S1",
            PhysicalWidth = 597,
            PhysicalHeight = 336
        }));

        var monitor = Assert.Single(layout.PhysicalMonitors);

        // Intrinsic (landscape panel) size on the shared model...
        Assert.Equal(597, monitor.Model.PhysicalSize.Width);
        Assert.Equal(336, monitor.Model.PhysicalSize.Height);

        // ...and a portrait geometry once the rotation is applied downstream.
        Assert.Equal(336, monitor.PhysicalRotated.Width);
        Assert.Equal(597, monitor.PhysicalRotated.Height);
    }

    [Fact]
    public void PortraitMonitorWithoutEdid_OrientedSourceSizeIsUnrotated()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        layout.AddMonitor(Portrait(edid: null));

        var monitor = Assert.Single(layout.PhysicalMonitors);

        Assert.Equal(597, monitor.Model.PhysicalSize.Width);
        Assert.Equal(336, monitor.Model.PhysicalSize.Height);
        Assert.Equal(336, monitor.PhysicalRotated.Width);
        Assert.Equal(597, monitor.PhysicalRotated.Height);
    }

    [Fact]
    public void LandscapeMonitor_SizeIsUntouched()
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design());

        layout.AddMonitor(new LinuxMonitor
        {
            ConnectorName = "DP-2",
            Orientation = 0,
            PixelWidth = 2560,
            PixelHeight = 1440,
            LogicalWidth = 2560,
            LogicalHeight = 1440,
            WidthMm = 597,
            HeightMm = 336,
            Primary = true
        });

        var monitor = Assert.Single(layout.PhysicalMonitors);

        Assert.Equal(597, monitor.Model.PhysicalSize.Width);
        Assert.Equal(336, monitor.Model.PhysicalSize.Height);
        Assert.Equal(597, monitor.PhysicalRotated.Width);
        Assert.Equal(336, monitor.PhysicalRotated.Height);
    }
}
