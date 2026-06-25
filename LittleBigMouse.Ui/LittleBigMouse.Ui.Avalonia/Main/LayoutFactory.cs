#nullable enable
using System;
using System.Linq;
using DynamicData;
using Avalonia;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Persistency;
using Avalonia.Media;
using HLab.ColorTools;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;

namespace LittleBigMouse.Ui.Avalonia.Main;

public static class LayoutFactory
{
    /// <summary>
    /// Update the layout from the monitors service
    /// </summary>
    /// <param name="layout"></param>
    /// <param name="service"></param>
    public static MonitorsLayout UpdateFrom(this MonitorsLayout layout, ISystemMonitorsService service)
    {

        foreach (var monitor in service.Root.AllMonitorDevices())
        {
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
                var model = layout.GetOrAddPhysicalMonitorModel(monitor.PnpCode,s => monitor.CreatePhysicalMonitorModel(s));

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

//        layout.Id = string.Join("+",layout.PhysicalMonitors.Select(m => $"{m.Id}_{m.Orientation}").OrderBy(s => s));
        layout.Id = string.Join("+",layout.PhysicalMonitors.Select(m => $"{m.Id}").OrderBy(s => s));

        // places monitors from windows configuration as best as possible
        layout.SetLocationsFromSystemConfiguration();

        //retrieve saved layout
        layout.Load();

        return layout;
    }


    public static DisplaySource CreateDisplaySource(this MonitorDevice monitor)
    {
        return new DisplaySource(monitor.SourceId).UpdateFrom(monitor);
    }

    public static DisplaySource UpdateFrom(this DisplaySource source, MonitorDevice monitor)
    {
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
        source.WallpaperPath = device.Parent.WallpaperPath;
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

        source.SourceNumber = monitor.MonitorNumber;

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

    public static Size Transpose(this Size size) => new(size.Height, size.Width);

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
        //        if (name.ToLower() == "generic pnp monitor") name = HtmlHelper.GetPnpName(@this.PnpCode);

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