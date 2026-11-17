#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using LittleBigMouse.Platform.Linux;

namespace LittleBigMouse.Platform.MacOS;

/// <summary>
/// Display discovery through CoreGraphics. Produces the same neutral
/// <see cref="LinuxMonitor"/> record the Linux sources do, so the model mapping
/// (<see cref="LinuxLayoutMapping"/>) is shared rather than duplicated.
/// <para>
/// The coordinate space handed to the model is CoreGraphics' global display
/// <b>points</b> (<c>CGDisplayBounds</c>): the space the cursor moves in and the
/// one the Rust daemon reads and warps in. On a Retina panel one point is two
/// pixels, exactly like a logical unit at 200% under Wayland, so the mode's pixel
/// size and the backing scale are reported alongside.
/// </para>
/// </summary>
public static class MacDisplays
{
    /// <summary>
    /// A stable name for a display: vendor, model and serial as CoreGraphics
    /// reports them (EDID-derived), so saved layouts follow a monitor across
    /// ports and reboots. The built-in panel gets a readable fixed name.
    /// </summary>
    public static string NameOf(uint display)
    {
        if (CG.CGDisplayIsBuiltin(display) != 0) return "Built-in";
        var vendor = CG.CGDisplayVendorNumber(display);
        var model = CG.CGDisplayModelNumber(display);
        var serial = CG.CGDisplaySerialNumber(display);
        // Two identical panels without a serial would collide: the unit number
        // (stable for a given connection) tells them apart.
        return serial != 0
            ? $"{vendor:X4}-{model:X4}-{serial:X8}"
            : $"{vendor:X4}-{model:X4}-u{CG.CGDisplayUnitNumber(display)}";
    }

    public static uint[] ActiveDisplays()
    {
        var ids = new uint[32];
        if (CG.CGGetActiveDisplayList((uint)ids.Length, ids, out var count) != 0) return [];
        return ids.Take((int)count).ToArray();
    }

    /// <summary>The display currently known by <paramref name="name"/>, if attached.</summary>
    public static uint? Find(string name)
    {
        foreach (var id in ActiveDisplays())
            if (NameOf(id) == name) return id;
        return null;
    }

    public static List<LinuxMonitor> Query()
    {
        var main = CG.CGMainDisplayID();
        var monitors = new List<LinuxMonitor>();

        foreach (var id in ActiveDisplays())
        {
            var bounds = CG.CGDisplayBounds(id);
            var sizeMm = CG.CGDisplayScreenSize(id);

            int pixelWidth = (int)Math.Round(bounds.Width), pixelHeight = (int)Math.Round(bounds.Height);
            double refresh = 0;
            var mode = CG.CGDisplayCopyDisplayMode(id);
            if (mode != IntPtr.Zero)
            {
                pixelWidth = (int)CG.CGDisplayModeGetPixelWidth(mode);
                pixelHeight = (int)CG.CGDisplayModeGetPixelHeight(mode);
                refresh = CG.CGDisplayModeGetRefreshRate(mode);
                CG.CGDisplayModeRelease(mode);
            }

            // The mode reports the panel's pixels in its current orientation, the
            // bounds in points: their ratio is the backing scale (2 on Retina).
            var scale = bounds.Width > 0 ? pixelWidth / bounds.Width : 1.0;

            var rotation = (int)Math.Round(CG.CGDisplayRotation(id) / 90.0) & 3;

            monitors.Add(new LinuxMonitor
            {
                ConnectorName = NameOf(id),
                LogicalX = bounds.X,
                LogicalY = bounds.Y,
                LogicalWidth = bounds.Width,
                LogicalHeight = bounds.Height,
                PixelWidth = pixelWidth,
                PixelHeight = pixelHeight,
                Scale = Math.Round(scale, 4),
                // CGDisplayScreenSize answers for the current orientation, like the
                // kscreen/xrandr sizes the shared mapping expects (it un-rotates).
                WidthMm = Math.Round(sizeMm.Width, 1),
                HeightMm = Math.Round(sizeMm.Height, 1),
                Primary = id == main,
                Enabled = true,
                Orientation = rotation,
                Frequency = (int)Math.Round(refresh),
            });
        }

        return monitors;
    }

    /// <summary>
    /// Move displays in the system arrangement (System Settings → Displays).
    /// Coordinates are global points; the main display defines (0,0), and the
    /// system snaps whatever it is given so that every display touches another.
    /// </summary>
    public static bool SetOrigins(IReadOnlyList<(uint Display, int X, int Y)> origins)
    {
        if (origins.Count == 0) return true;
        if (CG.CGBeginDisplayConfiguration(out var config) != 0) return false;

        foreach (var (display, x, y) in origins)
        {
            if (CG.CGConfigureDisplayOrigin(config, display, x, y) != 0)
            {
                CG.CGCancelDisplayConfiguration(config);
                return false;
            }
        }

        return CG.CGCompleteDisplayConfiguration(config, CG.kCGConfigurePermanently) == 0;
    }

    /// <summary>One line per display, for the change poll.</summary>
    public static string Signature()
        => string.Join("|", Query()
            .OrderBy(m => m.ConnectorName, StringComparer.Ordinal)
            .Select(m => $"{m.ConnectorName}[{m.LogicalX},{m.LogicalY} {m.LogicalWidth}x{m.LogicalHeight} "
                         + $"{m.PixelWidth}x{m.PixelHeight}@{m.Scale} r{m.Orientation}]{(m.Primary ? "*" : "")}"
                         + $"d{m.WidthMm}x{m.HeightMm}"));

    [StructLayout(LayoutKind.Sequential)]
    internal struct CGRect
    {
        public double X;
        public double Y;
        public double Width;
        public double Height;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct CGSize
    {
        public double Width;
        public double Height;
    }

    static class CG
    {
        const string Lib = "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics";

        public const int kCGConfigurePermanently = 2;

        [DllImport(Lib)] public static extern int CGGetActiveDisplayList(uint maxDisplays, [Out] uint[] displays, out uint count);
        [DllImport(Lib)] public static extern uint CGMainDisplayID();
        [DllImport(Lib)] public static extern CGRect CGDisplayBounds(uint display);
        [DllImport(Lib)] public static extern CGSize CGDisplayScreenSize(uint display);
        [DllImport(Lib)] public static extern double CGDisplayRotation(uint display);
        [DllImport(Lib)] public static extern int CGDisplayIsBuiltin(uint display);
        [DllImport(Lib)] public static extern uint CGDisplayVendorNumber(uint display);
        [DllImport(Lib)] public static extern uint CGDisplayModelNumber(uint display);
        [DllImport(Lib)] public static extern uint CGDisplaySerialNumber(uint display);
        [DllImport(Lib)] public static extern uint CGDisplayUnitNumber(uint display);

        [DllImport(Lib)] public static extern IntPtr CGDisplayCopyDisplayMode(uint display);
        [DllImport(Lib)] public static extern nuint CGDisplayModeGetPixelWidth(IntPtr mode);
        [DllImport(Lib)] public static extern nuint CGDisplayModeGetPixelHeight(IntPtr mode);
        [DllImport(Lib)] public static extern double CGDisplayModeGetRefreshRate(IntPtr mode);
        [DllImport(Lib)] public static extern void CGDisplayModeRelease(IntPtr mode);

        [DllImport(Lib)] public static extern int CGBeginDisplayConfiguration(out IntPtr config);
        [DllImport(Lib)] public static extern int CGConfigureDisplayOrigin(IntPtr config, uint display, int x, int y);
        [DllImport(Lib)] public static extern int CGCompleteDisplayConfiguration(IntPtr config, int option);
        [DllImport(Lib)] public static extern int CGCancelDisplayConfiguration(IntPtr config);
    }
}
