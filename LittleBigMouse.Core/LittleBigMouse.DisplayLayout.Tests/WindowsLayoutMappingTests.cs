using HLab.Geo;
using HLab.Sys.Monitors;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Windows;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Mapping of a simulated Win32 <see cref="MonitorDevice"/> snapshot onto the neutral model:
/// physical-size inference (the EDID / GDI / DPI fallback chain), orientation inference and
/// the per-source field copy. These are pure over the Win32 tree — no OS calls — so they run
/// on a Linux workstation; only the registry/COM wallpaper paths need real Windows.
/// <para>
/// The intent mirrors <see cref="LinuxLayoutMappingTests"/>: the shared model must carry the
/// panel's INTRINSIC size, with the rotation applied downstream (#507).
/// </para>
/// </summary>
public class WindowsLayoutMappingTests
{
    /// <summary>
    /// Assembles a <see cref="MonitorDevice"/> the way the OS enumeration would, so
    /// <see cref="MonitorDevice.ActiveConnection"/> resolves to the built adapter. Only the
    /// fields the mapping reads are populated.
    /// </summary>
    static MonitorDevice Snapshot(
        Edid? edid,
        Size pels,
        int orientation,
        Size gdiSizeMm,
        Size resolution,
        Size logPixels,
        bool primary = false,
        string deviceString = "NVIDIA GeForce RTX 3080",
        string monitorDeviceString = "Generic PnP Monitor",
        bool attachedToDesktop = true)
    {
        var mode = new DisplayMode
        {
            Position = new Point(0, 0),
            Pels = pels,
            DisplayOrientation = orientation,
            DisplayFrequency = 60,
        };

        var adapter = new PhysicalAdapter
        {
            DeviceName = @"\\.\DISPLAY1",
            DeviceString = deviceString,
            Primary = primary,
            HMonitor = 1,
            CurrentMode = mode,
            Capabilities = new DeviceCaps
            {
                Size = gdiSizeMm,
                Resolution = resolution,
                LogPixels = logPixels,
            },
            State = new DeviceState(),
        };

        var monitor = new MonitorDevice
        {
            Id = "MON1",
            SourceId = "SRC1",
            PnpCode = edid != null ? $"{edid.ManufacturerCode}{edid.ProductCode}" : "GEN",
            Edid = edid!,
        };

        var connection = new MonitorDeviceConnection
        {
            DeviceName = @"\\.\DISPLAY1\Monitor0",
            DeviceString = monitorDeviceString,
            Monitor = monitor,
            Parent = adapter,
            State = new DeviceState { AttachedToDesktop = attachedToDesktop },
        };

        monitor.Connections.Add(connection);
        return monitor;
    }

    static Edid Edid(double physicalWidth, double physicalHeight, string model = "S24D300")
        => new()
        {
            ManufacturerCode = "SAM",
            ProductCode = "1234",
            Serial = "S1",
            SerialNumber = "S/N: S1",
            Model = model,
            VideoInterface = "Dvi",
            PhysicalWidth = physicalWidth,
            PhysicalHeight = physicalHeight,
        };

    // ---- Physical size inference -------------------------------------------------

    [Fact]
    public void LandscapeWithConsistentGdi_UsesGdiSize()
    {
        // 16:9 panel, GDI mm already consistent with the pixel aspect: taken as-is.
        var monitor = Snapshot(
            edid: Edid(597, 336),
            pels: new Size(2560, 1440),
            orientation: 0,
            gdiSizeMm: new Size(598, 336),
            resolution: new Size(2560, 1440),
            logPixels: new Size(96, 96));

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(598, w);
        Assert.Equal(336, h);
    }

    [Fact]
    public void RotatedGdi_IsNormalizedBackToIntrinsicOrientation()
    {
        // Driver reports the GDI size transposed (portrait) while the intrinsic panel is 16:9.
        // The mapping must swap it back to the intrinsic (landscape) orientation.
        var monitor = Snapshot(
            edid: Edid(597, 336),
            pels: new Size(1440, 2560),
            orientation: 1,
            gdiSizeMm: new Size(336, 598), // transposed by the driver
            resolution: new Size(1440, 2560),
            logPixels: new Size(96, 96));

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(598, w);
        Assert.Equal(336, h);
    }

    [Fact]
    public void SquareGdiPlaceholderWithEdid_FallsBackToEdidSize()
    {
        // EDID-less-ish virtual display reports a bogus 1000x1000 GDI size that fails the
        // aspect test in both orientations; the EDID size wins.
        var monitor = Snapshot(
            edid: Edid(600, 340),
            pels: new Size(1920, 1080),
            orientation: 0,
            gdiSizeMm: new Size(1000, 1000),
            resolution: new Size(1920, 1080),
            logPixels: new Size(96, 96));

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(600, w);
        Assert.Equal(340, h);
    }

    [Fact]
    public void SquareGdiPlaceholderNoEdid_EstimatesFromResolutionAndDpi()
    {
        // No EDID at all and a square GDI placeholder: estimate mm from resolution / DPI.
        // 1920 px / 96 dpi * 25.4 = 508 mm, 1080 / 96 * 25.4 = 285.75 mm.
        var monitor = Snapshot(
            edid: null,
            pels: new Size(1920, 1080),
            orientation: 0,
            gdiSizeMm: new Size(1000, 1000),
            resolution: new Size(1920, 1080),
            logPixels: new Size(96, 96));

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(1920.0 / 96 * 25.4, w, 3);
        Assert.Equal(1080.0 / 96 * 25.4, h, 3);
    }

    [Fact]
    public void NoCurrentMode_WithEdid_UsesEdidSize()
    {
        // Detached / no mode: rely on the EDID.
        var monitor = new MonitorDevice
        {
            Id = "MON1",
            SourceId = "SRC1",
            Edid = Edid(510, 287),
        };
        // No connection => ActiveConnection is null => display?.CurrentMode is null.

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(510, w);
        Assert.Equal(287, h);
    }

    [Fact]
    public void NoCurrentMode_NoEdid_ReturnsZero()
    {
        var monitor = new MonitorDevice { Id = "MON1", SourceId = "SRC1" };

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(0, w);
        Assert.Equal(0, h);
    }

    [Fact]
    public void ZeroSizedEdid_IsTreatedAsAbsent()
    {
        // An incomplete EDID with 0 physical size must not be used as a size reference:
        // GetPhysicalSizeInMm treats it as absent and estimates from resolution/DPI here.
        var monitor = Snapshot(
            edid: Edid(0, 0),
            pels: new Size(1920, 1080),
            orientation: 0,
            gdiSizeMm: new Size(1000, 1000),
            resolution: new Size(1920, 1080),
            logPixels: new Size(96, 96));

        var (w, h) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);

        Assert.Equal(1920.0 / 96 * 25.4, w, 3);
        Assert.Equal(1080.0 / 96 * 25.4, h, 3);
    }

    // ---- Orientation inference ---------------------------------------------------

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    [InlineData(3)]
    public void ExplicitDevmodeOrientation_IsReturnedAsIs(int orientation)
    {
        var mode = new DisplayMode { Pels = new Size(2560, 1440), DisplayOrientation = orientation };
        Assert.Equal(orientation, WindowsPhysicalSize.InferOrientation(mode, Edid(597, 336)));
    }

    [Fact]
    public void NoEdid_DefaultOrientation_StaysZero()
    {
        var mode = new DisplayMode { Pels = new Size(1440, 2560), DisplayOrientation = 0 };
        Assert.Equal(0, WindowsPhysicalSize.InferOrientation(mode, null));
    }

    [Fact]
    public void DriverRotatedBelowWindows_PixelPortraitLandscapeEdid_InfersRotation()
    {
        // DEVMODE says Default (0) but pixels are portrait while the EDID panel is landscape:
        // the display is effectively rotated (#507), so report 90 deg (1).
        var mode = new DisplayMode { Pels = new Size(1440, 2560), DisplayOrientation = 0 };
        Assert.Equal(1, WindowsPhysicalSize.InferOrientation(mode, Edid(597, 336)));
    }

    [Fact]
    public void PixelAndPanelAgree_NoInferredRotation()
    {
        var mode = new DisplayMode { Pels = new Size(2560, 1440), DisplayOrientation = 0 };
        Assert.Equal(0, WindowsPhysicalSize.InferOrientation(mode, Edid(597, 336)));
    }

    [Fact]
    public void SquarePixels_DecideNothing()
    {
        var mode = new DisplayMode { Pels = new Size(1080, 1080), DisplayOrientation = 0 };
        Assert.Equal(0, WindowsPhysicalSize.InferOrientation(mode, Edid(597, 336)));
    }

    [Fact]
    public void SquareEdid_DecidesNothing()
    {
        var mode = new DisplayMode { Pels = new Size(1440, 2560), DisplayOrientation = 0 };
        Assert.Equal(0, WindowsPhysicalSize.InferOrientation(mode, Edid(500, 500)));
    }

    // ---- Aspect consistency ------------------------------------------------------

    [Fact]
    public void AspectConsistency_MatchingAspectIsTrue()
    {
        Assert.True(WindowsPhysicalSize.IsAspectConsistent(598, 336, 2560, 1440));
    }

    [Fact]
    public void AspectConsistency_SquarePlaceholderAgainstWideResolutionIsFalse()
    {
        Assert.False(WindowsPhysicalSize.IsAspectConsistent(1000, 1000, 1920, 1080));
    }

    [Theory]
    [InlineData(0, 336, 2560, 1440)]
    [InlineData(598, 0, 2560, 1440)]
    [InlineData(598, 336, 0, 1440)]
    [InlineData(598, 336, 2560, 0)]
    public void AspectConsistency_NonPositiveInputsAreFalse(double w, double h, double pw, double ph)
    {
        Assert.False(WindowsPhysicalSize.IsAspectConsistent(w, h, pw, ph));
    }

    // ---- Source field mapping ----------------------------------------------------

    [Fact]
    public void SourceMapping_CopiesIdentityPrimaryAndPixelRect()
    {
        var monitor = Snapshot(
            edid: Edid(597, 336),
            pels: new Size(2560, 1440),
            orientation: 0,
            gdiSizeMm: new Size(598, 336),
            resolution: new Size(2560, 1440),
            logPixels: new Size(96, 96),
            primary: true);
        monitor.MonitorNumber = "1";

        var source = monitor.CreateDisplaySource();

        Assert.True(source.Primary);
        Assert.True(source.AttachedToDesktop);
        Assert.Equal(60, source.DisplayFrequency);
        Assert.Equal(0, source.Orientation);
        Assert.Equal(2560, source.InPixel.Width);
        Assert.Equal(1440, source.InPixel.Height);
        Assert.Equal("1", source.SourceNumber);
        Assert.Equal(@"\\.\DISPLAY1\Monitor0", source.DeviceName);
        Assert.Equal(@"\\.\DISPLAY1", source.DisplayName);
    }

    [Fact]
    public void SourceMapping_InfersRotationForDriverRotatedPanel()
    {
        var monitor = Snapshot(
            edid: Edid(597, 336),
            pels: new Size(1440, 2560),
            orientation: 0, // DEVMODE default, driver rotated below Windows
            gdiSizeMm: new Size(336, 598),
            resolution: new Size(1440, 2560),
            logPixels: new Size(96, 96));

        var source = monitor.CreateDisplaySource();

        Assert.Equal(1, source.Orientation);
    }

    [Fact]
    public void SourceMapping_NoCurrentMode_ClearsFrequencyAndZeroesRect()
    {
        var monitor = new MonitorDevice { Id = "MON1", SourceId = "SRC1", Edid = Edid(597, 336) };

        var source = monitor.CreateDisplaySource();

        Assert.Equal(0, source.DisplayFrequency);
    }

    // ---- Model mapping -----------------------------------------------------------

    [Fact]
    public void ModelMapping_KeepsIntrinsicSizeForDriverRotatedPortrait()
    {
        // Portrait pixels, driver-rotated below Windows, landscape EDID panel: the shared model
        // must keep the INTRINSIC (landscape) size (#507).
        var monitor = Snapshot(
            edid: Edid(597, 336),
            pels: new Size(1440, 2560),
            orientation: 1,
            gdiSizeMm: new Size(336, 598),
            resolution: new Size(1440, 2560),
            logPixels: new Size(96, 96));

        var model = monitor.CreatePhysicalMonitorModel("SAM1234");

        Assert.Equal(598, model.PhysicalSize.Width);
        Assert.Equal(336, model.PhysicalSize.Height);
    }

    [Fact]
    public void ModelMapping_GenericPnpNameWithEdidModel_UsesEdidModel()
    {
        var monitor = Snapshot(
            edid: Edid(597, 336, model: "MyPanel"),
            pels: new Size(2560, 1440),
            orientation: 0,
            gdiSizeMm: new Size(598, 336),
            resolution: new Size(2560, 1440),
            logPixels: new Size(96, 96),
            monitorDeviceString: "Generic PnP Monitor");

        var model = monitor.CreatePhysicalMonitorModel("SAM1234");

        Assert.Equal("MyPanel", model.PnpDeviceName);
    }

    [Fact]
    public void PhysicalMonitorMapping_CopiesDeviceIdAndSerial()
    {
        var monitor = Snapshot(
            edid: Edid(597, 336),
            pels: new Size(2560, 1440),
            orientation: 0,
            gdiSizeMm: new Size(598, 336),
            resolution: new Size(2560, 1440),
            logPixels: new Size(96, 96));

        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        var model = monitor.CreatePhysicalMonitorModel("SAM1234");
        var physical = monitor.CreatePhysicalMonitor("SRC1", layout, model);

        Assert.Equal("MON1", physical.DeviceId);
        Assert.Equal("S/N: S1", physical.SerialNumber);
    }

    [Fact]
    public void PhysicalMonitorMapping_NoEdid_SerialFallsBackToNA()
    {
        var monitor = Snapshot(
            edid: null,
            pels: new Size(1920, 1080),
            orientation: 0,
            gdiSizeMm: new Size(509, 286),
            resolution: new Size(1920, 1080),
            logPixels: new Size(96, 96));

        var layout = new MonitorsLayout(new ILayoutOptions.Design());
        var model = monitor.CreatePhysicalMonitorModel("GEN");
        var physical = monitor.CreatePhysicalMonitor("SRC1", layout, model);

        Assert.Equal("N/A", physical.SerialNumber);
    }
}
