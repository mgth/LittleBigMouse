using HLab.Geo;
using HLab.Sys.Monitors;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Windows;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The Windows wallpaper read seam: <see cref="WindowsWallpaperReader"/> (registry/file facts +
/// poll signature) and the <see cref="WindowsWallpaperMapping"/> copy into the reactive model.
/// <para>
/// The registry/file reads themselves need a real HKCU, so they are <see cref="WindowsFactAttribute"/>
/// and only run in CI on Windows. Everything provable off Windows runs on a Linux workstation: the
/// reader is safe off Windows (returns <c>Empty</c>, never throws), the signature is stable across
/// repeated reads, and the model copy — including the fade-window fallback and invalid/absent paths —
/// is pure over a simulated Win32 snapshot.
/// </para>
/// </summary>
public class WindowsWallpaperReaderTests
{
    // ---- Reader: off-Windows contract (runs on Linux) ----------------------------

    [Fact]
    public void Read_OffWindows_ReturnsEmptyWithoutThrowing()
    {
        // The whole point of the seam: a Linux/macOS caller (tests, the Linux daemon build) can
        // invoke the reader and get a well-defined "nothing" instead of PlatformNotSupportedException.
        if (OperatingSystem.IsWindows()) return; // asserted by the WindowsFact tests instead

        var state = WindowsWallpaperReader.Read();

        Assert.Equal("", state.ImagePath);
        Assert.Equal("", state.Signature);
        Assert.Equal(WindowsWallpaperState.Empty, state);
    }

    [Fact]
    public void ReadImagePathAndSignature_OffWindows_AreEmpty()
    {
        if (OperatingSystem.IsWindows()) return;

        Assert.Equal("", WindowsWallpaperReader.ReadImagePath());
        Assert.Equal("", WindowsWallpaperReader.ReadSignature());
    }

    [Fact]
    public void EmptyState_HasEmptyFields()
    {
        Assert.Equal("", WindowsWallpaperState.Empty.ImagePath);
        Assert.Equal("", WindowsWallpaperState.Empty.Signature);
    }

    // ---- Reader: signature stability (runs everywhere) ---------------------------

    [Fact]
    public void Signature_IsStableAcrossRepeatedReads()
    {
        // Two reads of the same OS state must compare equal, or the poll gate in
        // WindowsLayoutFactory would re-read the device tree on every tick. Off Windows both are
        // "" (still equal); on Windows the underlying state is unchanged between two back-to-back
        // calls, so the fingerprint is identical.
        Assert.Equal(WindowsWallpaperReader.ReadSignature(), WindowsWallpaperReader.ReadSignature());
    }

    [Fact]
    public void Read_SignatureMatchesReadSignature()
    {
        // The one-pass Read() must agree with the standalone ReadSignature(): the poll gate uses
        // ReadSignature() while the mapping uses Read()/ReadImagePath(), and they must not disagree.
        Assert.Equal(WindowsWallpaperReader.ReadSignature(), WindowsWallpaperReader.Read().Signature);
    }

    [WindowsFact]
    public void Signature_OnWindows_IsNonEmptyAndHasFiveFields()
    {
        // Format is wallpaper|style|tile|background|mtime — five pipe-separated fields, always.
        var signature = WindowsWallpaperReader.ReadSignature();

        Assert.NotEqual("", signature);
        Assert.Equal(5, signature.Split('|').Length);
    }

    [WindowsFact]
    public void Read_OnWindows_ImagePathMatchesStandalone()
    {
        Assert.Equal(WindowsWallpaperReader.ReadImagePath(), WindowsWallpaperReader.Read().ImagePath);
    }

    // ---- Mapping: model copy over a simulated snapshot (runs on Linux) -----------

    static MonitorDevice WallpaperSnapshot(
        string? comWallpaperPath,
        DesktopWallpaperPosition position = DesktopWallpaperPosition.Fill,
        uint background = 0)
    {
        var adapter = new PhysicalAdapter
        {
            DeviceName = @"\\.\DISPLAY1",
            WallpaperPath = comWallpaperPath!,
            WallpaperPosition = position,
            Background = background,
            State = new DeviceState(),
        };

        var monitor = new MonitorDevice { Id = "MON1", SourceId = "SRC1" };

        var connection = new MonitorDeviceConnection
        {
            DeviceName = @"\\.\DISPLAY1\Monitor0",
            Monitor = monitor,
            Parent = adapter,
            State = new DeviceState { AttachedToDesktop = true },
        };

        monitor.Connections.Add(connection);
        return monitor;
    }

    [Fact]
    public void Mapping_CopiesComPathStyleAndBackground()
    {
        var monitor = WallpaperSnapshot(
            @"C:\Users\me\Pictures\bg.jpg",
            DesktopWallpaperPosition.Fit,
            background: 0x00332211); // COLORREF 0x00BBGGRR -> R=0x11 G=0x22 B=0x33

        var source = new DisplaySource("SRC1").UpdateWallpaperFrom(monitor);

        Assert.Equal(@"C:\Users\me\Pictures\bg.jpg", source.WallpaperPath);
        Assert.Equal(WallpaperStyle.Fit, source.WallpaperStyle);
        // Components are the raw COLORREF bytes stored as doubles (0..255), not normalized.
        Assert.Equal(0x11, source.BackgroundColor.Red);
        Assert.Equal(0x22, source.BackgroundColor.Green);
        Assert.Equal(0x33, source.BackgroundColor.Blue);
    }

    [Theory]
    [InlineData(DesktopWallpaperPosition.Fill, WallpaperStyle.Fill)]
    [InlineData(DesktopWallpaperPosition.Fit, WallpaperStyle.Fit)]
    [InlineData(DesktopWallpaperPosition.Center, WallpaperStyle.Center)]
    [InlineData(DesktopWallpaperPosition.Tile, WallpaperStyle.Tile)]
    [InlineData(DesktopWallpaperPosition.Span, WallpaperStyle.Span)]
    [InlineData(DesktopWallpaperPosition.Stretch, WallpaperStyle.Stretch)]
    public void Mapping_MapsEveryWallpaperPosition(DesktopWallpaperPosition position, WallpaperStyle expected)
    {
        var monitor = WallpaperSnapshot(@"C:\bg.jpg", position);

        var source = new DisplaySource("SRC1").UpdateWallpaperFrom(monitor);

        Assert.Equal(expected, source.WallpaperStyle);
    }

    [Fact]
    public void Mapping_EmptyComPath_UsesFallbackPath()
    {
        // Wallpaper fade: IDesktopWallpaper reports "" for the per-monitor path. The registry
        // fallback (the reader's ReadImagePath, injected here as the real current image) must win
        // so the monitor is not blanked mid-change.
        var monitor = WallpaperSnapshot("");

        var source = new DisplaySource("SRC1")
            .UpdateWallpaperFrom(monitor, fallbackPath: @"C:\real\current.jpg");

        Assert.Equal(@"C:\real\current.jpg", source.WallpaperPath);
    }

    [Fact]
    public void Mapping_EmptyComPath_NoFallback_StaysEmpty()
    {
        // Solid-color desktop: no COM path and no fallback. The source path is left empty rather
        // than fabricated — the background color drives the paint.
        var monitor = WallpaperSnapshot("");

        var source = new DisplaySource("SRC1").UpdateWallpaperFrom(monitor);

        Assert.Equal("", source.WallpaperPath);
    }

    [Fact]
    public void Mapping_NonEmptyComPath_IgnoresFallback()
    {
        // When the per-monitor COM path is present it is authoritative (per-monitor wallpapers):
        // the registry fallback must not override it.
        var monitor = WallpaperSnapshot(@"C:\per-monitor\left.jpg");

        var source = new DisplaySource("SRC1")
            .UpdateWallpaperFrom(monitor, fallbackPath: @"C:\global\other.jpg");

        Assert.Equal(@"C:\per-monitor\left.jpg", source.WallpaperPath);
    }

    [Fact]
    public void Mapping_NoActiveConnection_LeavesSourceUnchanged()
    {
        // A monitor with no connection (detached) has no adapter to read: the reactive model must
        // be left exactly as it was, never cleared.
        var monitor = new MonitorDevice { Id = "MON1", SourceId = "SRC1" };
        var source = new DisplaySource("SRC1") { WallpaperPath = @"C:\keep.jpg" };

        var result = source.UpdateWallpaperFrom(monitor);

        Assert.Same(source, result);
        Assert.Equal(@"C:\keep.jpg", source.WallpaperPath);
    }
}
