#nullable enable
using HLab.Sys.Monitors;
using System;
using System.Linq;
using DynamicData;
using HLab.ColorTools;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="ILayoutFactory"/>: reads the Win32 monitor device
/// tree and builds the neutral <see cref="MonitorsLayout"/> model directly — no
/// intermediate data layer. This is the former UI <c>LayoutFactory</c>, moved to the
/// Windows platform project and formalized behind the seam. A Linux factory builds the same
/// model from RandR/DRM.
/// </summary>
public class WindowsLayoutFactory : ILayoutFactory, IDisposable
{
    readonly ISystemMonitorsService _monitors;
    readonly Func<MonitorsLayout> _newLayout;
    readonly ILayoutPersistence _persistence;
    readonly WindowsWallpaperWatcher _wallpaperWatcher;
    string _lastWallpaperSignature = "";

    public WindowsLayoutFactory(ISystemMonitorsService monitors, Func<MonitorsLayout> newLayout, ILayoutPersistence persistence)
    {
        _monitors = monitors;
        _newLayout = newLayout;
        _persistence = persistence;

        _wallpaperWatcher = new WindowsWallpaperWatcher();
        _wallpaperWatcher.Changed += (_, _) => WallpaperChanged?.Invoke(this, EventArgs.Empty);
    }

    // Never raised on Windows: display changes arrive through the daemon's DisplayChanged
    // event (the daemon owns the unhook-on-change semantics there).
    public event EventHandler? DisplayChanged { add { } remove { } }

    public event EventHandler? WallpaperChanged;

    public void Dispose() => _wallpaperWatcher.Dispose();

    public MonitorsLayout Create()
    {
        // TEMP profiling (#displaychange-timing)
        WindowsLayoutMapping.Prof("[Create] START");
        var sw = System.Diagnostics.Stopwatch.StartNew();
        // Refresh the OS device tree (formerly MainService's UpdateDevices()).
        if (_monitors is SystemMonitorsService concrete) concrete.UpdateDevices();
        WindowsLayoutMapping.Prof($"[Create] UpdateDevices() {sw.ElapsedMilliseconds}ms"); sw.Restart();
        var layout = _newLayout().UpdateFrom(_monitors, _persistence);
        WindowsLayoutMapping.Prof($"[Create] UpdateFrom() {sw.ElapsedMilliseconds}ms");
        return layout;
    }

    /// <inheritdoc/>
    public string DisplaySignature() => MonitorDeviceHelper.DisplaySignature();

    public void UpdateWallpaper(MonitorsLayout layout)
    {
        // Cheap gate: compute a light signature (registry values + the transcoded-wallpaper file
        // timestamp) and only read the OS wallpaper / device tree when it actually changed. This
        // lets the UI poll this method frequently (while the config window is open) at near-zero
        // cost, which is the reliable trigger — the daemon's WM_SETTINGCHANGE broadcast is missed
        // intermittently (shared message pump), so it is only a best-effort fast path.
        var signature = WindowsLayoutMapping.WallpaperSignature();
        if (signature == _lastWallpaperSignature) return;
        _lastWallpaperSignature = signature;

        layout.UpdateWallpaper(_monitors);
    }
}

/// <summary>
/// Win32 → model mapping. Extension methods (hence a separate static class) reading the
/// <see cref="MonitorDevice"/> tree straight into the reactive model.
/// </summary>
internal static class WindowsLayoutMapping
{
    // TEMP profiling (#displaychange-timing): writes phase timings to C:\dev\lbm-dc.log. Remove after tuning.
    internal static void Prof(string m) { try { System.IO.File.AppendAllText(@"C:\dev\lbm-dc.log", $"{System.DateTime.Now:HH:mm:ss.fff} {m}{System.Environment.NewLine}"); } catch { } }

    public static MonitorsLayout UpdateFrom(this MonitorsLayout layout, ISystemMonitorsService service, ILayoutPersistence persistence)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        // DPI awareness is process-scoped (the UI thread's manifested awareness), formerly
        // computed in the MonitorsLayout constructor. Set it before building the sources.
        layout.DpiAwareness = (DpiAwarenessKind)(int)WinUser.GetAwarenessFromDpiAwarenessContext(
            WinUser.GetThreadDpiAwarenessContext());

        // First access to .Root triggers the (lazy, expensive) GetDisplayDevices enumeration.
        // Hold a strong ref for the whole method so the WeakReference can't drop it mid-build.
        var root = service.Root;
        Prof($"  [UF] RootAccess(->GDD) {sw.ElapsedMilliseconds}ms"); sw.Restart();

        foreach (var monitor in root.AllMonitorDevices())
        {
            // Specialized displays (VR headsets...) are hidden from the desktop by
            // Windows: keep them out of the layout too, the cursor can never reach
            // them and their zone would only trap it (#364)
            if (monitor.IsSpecialized) continue;

            var source = layout.PhysicalSources.FirstOrDefault(s => s.DeviceId == monitor.Id);

            if (source != null)
            {
                source.Source.UpdateFrom(monitor);
                continue;
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
        Prof($"  [UF] buildSources {sw.ElapsedMilliseconds}ms"); sw.Restart();

        layout.Id = layout.ComputeId();

        // places monitors from windows configuration as best as possible
        layout.SetLocationsFromSystemConfiguration();
        Prof($"  [UF] SetLocationsFromSystemConfiguration {sw.ElapsedMilliseconds}ms"); sw.Restart();

        //retrieve saved layout (registry, via the shared persistence engine)
        persistence.Load(layout);
        Prof($"  [UF] Load {sw.ElapsedMilliseconds}ms"); sw.Restart();

        // saved locations are anchored on the primary that was active at save time:
        // re-anchor so the current primary sits at (0,0) mm, like in pixels
        layout.AnchorOnPrimary();
        Prof($"  [UF] AnchorOnPrimary {sw.ElapsedMilliseconds}ms");

        return layout;
    }

    /// <summary>
    /// Re-read the desktop wallpaper (only) into the sources of an already-built layout, in
    /// place: geometry, DPI and monitor identity are left untouched, so any in-progress layout
    /// edits are preserved. The reactive <see cref="DisplaySource"/> properties drive the repaint.
    /// </summary>
    public static void UpdateWallpaper(this MonitorsLayout layout, ISystemMonitorsService service)
    {
        var root = service.Root;
        if (root is null) return;

        // Re-read the live wallpaper (COM IDesktopWallpaper) into the cached Win32 adapters.
        root.UpdateWallpaper();

        // IDesktopWallpaper.GetWallpaper returns "" while Windows plays the wallpaper fade, so an
        // image->image change would momentarily read empty. HKCU\Control Panel\Desktop\WallPaper
        // holds the real current image path (empty for a solid color) and is written immediately,
        // so use it as a fallback when the per-monitor COM path is still empty. Per-monitor
        // wallpapers keep working via the COM path when present.
        var registryPath = GetRegistryWallpaperPath();

        foreach (var monitor in root.AllMonitorDevices())
        {
            if (monitor.IsSpecialized) continue;
            layout.PhysicalSources
                .FirstOrDefault(s => s.DeviceId == monitor.Id)
                ?.Source.UpdateWallpaperFrom(monitor, registryPath);
        }
    }

    /// <summary>
    /// The current desktop wallpaper image path from HKCU\Control Panel\Desktop\WallPaper (empty
    /// for a solid color). Written immediately on change, unlike IDesktopWallpaper.GetWallpaper
    /// which returns "" during the wallpaper fade — a reliable fallback for a live change.
    /// </summary>
    static string GetRegistryWallpaperPath()
    {
        try
        {
            using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop");
            return key?.GetValue("WallPaper") as string ?? "";
        }
        catch
        {
            return "";
        }
    }

    /// <summary>
    /// A cheap fingerprint of the current desktop wallpaper state — registry values plus the
    /// transcoded-wallpaper file timestamp (rewritten on every image change). Changes whenever the
    /// image, fit or background color changes, so callers can poll it to detect a wallpaper change
    /// without touching the display device tree or COM.
    /// </summary>
    public static string WallpaperSignature()
    {
        try
        {
            using var desktop = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop");
            using var colors = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Control Panel\Colors");

            var wp = desktop?.GetValue("WallPaper") as string ?? "";
            var style = desktop?.GetValue("WallpaperStyle")?.ToString() ?? "";
            var tile = desktop?.GetValue("TileWallpaper")?.ToString() ?? "";
            var background = colors?.GetValue("Background")?.ToString() ?? "";

            var transcoded = System.IO.Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                "Microsoft", "Windows", "Themes", "TranscodedWallpaper");
            var mtime = System.IO.File.Exists(transcoded)
                ? System.IO.File.GetLastWriteTimeUtc(transcoded).Ticks
                : 0L;

            return $"{wp}|{style}|{tile}|{background}|{mtime}";
        }
        catch
        {
            return "";
        }
    }

    public static DisplaySource CreateDisplaySource(this MonitorDevice monitor)
    {
        return new DisplaySource(monitor.SourceId).UpdateFrom(monitor);
    }

    public static DisplaySource UpdateFrom(this DisplaySource source, MonitorDevice monitor)
    {
        source.InterfacePath = monitor.InterfacePath;

        if(monitor.Connections.Count == 0) return source;
        var device = monitor.Connections[0];

        if(device.Parent == null) return source;

        source.DisplayName = device.Parent.DeviceName;
        source.DeviceName = device.DeviceName;

        source.SourceName = $"{monitor.Edid?.VideoInterface??"Unknown"}:{device.DeviceName}";


        source.Primary = device.Parent.Primary;
        source.AttachedToDesktop = device.State.AttachedToDesktop;

        source.EffectiveDpi.Set(device.Parent.EffectiveDpi);
        source.DpiAwareAngularDpi.Set(device.Parent.AngularDpi);
        source.RawDpi.Set(device.Parent.RawDpi);

        if (device.Parent.CurrentMode is { } mode)
        {
            source.DisplayFrequency = mode.DisplayFrequency;

            source.InPixel.Set(new HLab.Geo.Rect(
                mode.Position,
                mode.Pels));

            source.Orientation = mode.DisplayOrientation;
        }
        else
        {
            source.DisplayFrequency = 0;
            source.InPixel.Set(new HLab.Geo.Rect(new HLab.Geo.Point(0, 0),new HLab.Geo.Size(0, 0)));
        }

        (source.InterfaceName, source.InterfaceLogo) = device.Parent.InterfaceBrandNameAndLogo();

        source.UpdateWallpaperFrom(monitor);

        source.SourceNumber = monitor.MonitorNumber;

        return source;
    }

    /// <summary>
    /// Copy only the wallpaper (path, style, background color) from the Win32 adapter into the
    /// reactive model. Shared by the full <see cref="UpdateFrom(DisplaySource, MonitorDevice)"/>
    /// mapping and the in-place wallpaper refresh (live wallpaper change, no layout rebuild).
    /// <para>
    /// When <paramref name="allowClear"/> is false, an empty freshly-read path is ignored and the
    /// previous path kept — this is what stops the transient empty returned during the wallpaper
    /// fade from blanking the monitor. Style and background color are always applied.
    /// </para>
    /// </summary>
    public static DisplaySource UpdateWallpaperFrom(this DisplaySource source, MonitorDevice monitor, string? fallbackPath = null)
    {
        if (monitor.Connections.Count == 0) return source;
        var device = monitor.Connections[0];
        if (device.Parent == null) return source;

        // Per-monitor COM path is empty during the wallpaper fade; fall back to the registry path
        // (the real current image, written immediately) so a live change reflects at once.
        var comPath = device.Parent.WallpaperPath;
        source.WallpaperPath = !string.IsNullOrEmpty(comPath) ? comPath : (fallbackPath ?? comPath);

        source.WallpaperStyle = device.Parent.WallpaperPosition switch
        {
            DesktopWallpaperPosition.Fill => WallpaperStyle.Fill,
            DesktopWallpaperPosition.Fit => WallpaperStyle.Fit,
            DesktopWallpaperPosition.Center => WallpaperStyle.Center,
            DesktopWallpaperPosition.Tile => WallpaperStyle.Tile,
            DesktopWallpaperPosition.Span => WallpaperStyle.Span,
            _ => WallpaperStyle.Stretch
        };

        var color = device.Parent.Background;
        source.BackgroundColor = HLabColors.RGB<double>((byte)(color & 0xFF),(byte)((color >> 8) & 0xFF),(byte)((color >> 16) & 0xFF));

        return source;
    }

    public static PhysicalMonitorModel CreatePhysicalMonitorModel(this MonitorDevice monitor, string id)
        => new PhysicalMonitorModel(id).UpdateFrom(monitor);

    public static PhysicalMonitorModel UpdateFrom(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        using (@this.DelayChangeNotifications())
        {
            @this.SetSizeFrom(monitor);
            @this.SetPnpDeviceName(monitor);

            @this.Logo = monitor.BrandLogo();

            return @this;
        }
    }

    public static PhysicalMonitorModel SetSizeFrom(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        using (@this.PhysicalSize.DelayChangeNotifications())
        {
            var old = @this.PhysicalSize.FixedAspectRatio;
            @this.PhysicalSize.FixedAspectRatio = false;

            var (width, height) = GetPhysicalSizeInMm(monitor);
            if (width > 0 && height > 0)
            {
                @this.PhysicalSize.Width = width;
                @this.PhysicalSize.Height = height;
            }

            @this.PhysicalSize.FixedAspectRatio = old;

            return @this;
        }
    }

    /// <summary>
    /// Physical size of the panel in millimeters, in the current display orientation.
    /// Windows GDI (HORZSIZE/VERTSIZE) is used when it is consistent with the resolution
    /// aspect ratio. A display without EDID (virtual, DisplayLink, RDP, spacedesk...) reports
    /// a bogus square placeholder (e.g. 1000x1000): we then fall back to the EDID size, and
    /// finally to an estimate derived from the resolution and the DPI (HORZRES / LOGPIXELSX).
    /// </summary>
    static (double Width, double Height) GetPhysicalSizeInMm(MonitorDevice monitor)
    {
        var display = monitor.Connections[0].Parent;

        if (display?.CurrentMode != null)
        {
            var caps = display.Capabilities;
            var rotated = display.CurrentMode.DisplayOrientation % 2 != 0;

            // GDI physical size, oriented like the current resolution.
            var gdiW = rotated ? caps.Size.Height : caps.Size.Width;
            var gdiH = rotated ? caps.Size.Width : caps.Size.Height;

            if (IsAspectConsistent(gdiW, gdiH, caps.Resolution.Width, caps.Resolution.Height))
                return (gdiW, gdiH);

            // GDI size unreliable (EDID-less display): prefer the EDID size when available.
            if (monitor.Edid is { PhysicalWidth: > 0, PhysicalHeight: > 0 } edid)
                return rotated
                    ? (edid.PhysicalHeight, edid.PhysicalWidth)
                    : (edid.PhysicalWidth, edid.PhysicalHeight);

            // Otherwise estimate from the resolution and the DPI: inches = pixels / dpi.
            if (caps.LogPixels.Width > 0 && caps.LogPixels.Height > 0)
                return (
                    caps.Resolution.Width / caps.LogPixels.Width * 25.4,
                    caps.Resolution.Height / caps.LogPixels.Height * 25.4);

            return (gdiW, gdiH); // nothing better than the GDI value
        }

        // Detached / no current mode: rely on EDID if present.
        if (monitor.Edid is { PhysicalWidth: > 0, PhysicalHeight: > 0 } e)
            return (e.PhysicalWidth, e.PhysicalHeight);

        return (0, 0);
    }

    /// <summary>
    /// True when the physical size aspect ratio roughly matches the pixel aspect ratio
    /// (square pixels). A square placeholder (1000x1000) against a 16:9 resolution fails this.
    /// </summary>
    static bool IsAspectConsistent(double width, double height, double pixelsWidth, double pixelsHeight)
    {
        if (width <= 0 || height <= 0 || pixelsWidth <= 0 || pixelsHeight <= 0) return false;

        var sizeAspect = width / height;
        var pixelAspect = pixelsWidth / pixelsHeight;

        return Math.Abs(sizeAspect / pixelAspect - 1.0) < 0.12;
    }

    public static PhysicalMonitorModel SetPnpDeviceName(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        if (!string.IsNullOrEmpty(@this.PnpDeviceName)) return @this;

        var name = HtmlHelper.CleanupPnpName(monitor.Connections[0].DeviceString);
        // A monitor without EDID (virtual display, DisplayLink, RDP, spacedesk, some panels)
        // reports "Generic PnP Monitor" and has a null Edid: keep the generic name then.
        if (name.ToLower() == "generic pnp monitor" && !string.IsNullOrEmpty(monitor.Edid?.Model))
            name = monitor.Edid.Model;

        @this.PnpDeviceName = name;

        return @this;
    }

    public static PhysicalMonitor CreatePhysicalMonitor(this MonitorDevice device, string id, IMonitorsLayout layout, PhysicalMonitorModel model)
        => new PhysicalMonitor(id, layout, model).UpdateFrom(device);

    public static PhysicalMonitor UpdateFrom(this PhysicalMonitor monitor, MonitorDevice device)
    {
        monitor.DeviceId = device.Id;

        // Serial Number
        monitor.SerialNumber = device.Edid?.SerialNumber ?? "N/A";

        return monitor;
    }

    static string BrandLogo(this MonitorDevice device)
    {
        var dev = device.Connections[0].Parent?.DeviceString;
        if (dev != null)
        {
            // special case for Spacedesk support
            if (dev.Contains("spacedesk", StringComparison.OrdinalIgnoreCase)) return "icon/Pnp/Spacedesk";
            // special case for Remote desktop support
            if (dev == "Microsoft Remote Display Adapter") return "icon/Pnp/Microsoft";
        }

        if (device.Edid is null) return "icon/Pnp/LBM";

        // special case for Aorus support
        if (device.Edid.Model?.Contains("Aorus") == true) return "icon/Pnp/Aorus";

        return $"icon/Pnp/{device.Edid.ManufacturerCode}?icon/Pnp/LBM";
    }

    static readonly string[] Brands = { "intel", "amd", "nvidia", "microsoft" };
    public static (string, string) InterfaceBrandNameAndLogo(this PhysicalAdapter adapter)
    {
        if(adapter.Parent == null) return ("detached", "icon/parts/detached");

        var dev = adapter.DeviceString?.ToLower() ?? "";

        foreach (var brand in Brands)
        {
            if (dev.Contains(brand)) return (dev, $"icon/pnp/{brand}");
        }
        return (dev, "");
    }
}
