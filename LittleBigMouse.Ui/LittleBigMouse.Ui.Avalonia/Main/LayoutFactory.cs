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
        var device = monitor.Connections[0];

        source.DisplayName = device.Parent?.DeviceName;
        source.DeviceName = device.DeviceName;

        source.SourceName = $"{monitor.Edid?.VideoInterface??"Unknown"}:{device.DeviceName}";


        source.Primary = device.Parent.Primary;
        source.AttachedToDesktop = device.State.AttachedToDesktop;

        source.EffectiveDpi.Set(device.Parent.EffectiveDpi);
        source.DpiAwareAngularDpi.Set(device.Parent.AngularDpi);
        source.RawDpi.Set(device.Parent.RawDpi);

        if (device.Parent?.CurrentMode is { } mode)
        {
            source.DisplayFrequency = mode.DisplayFrequency;

            source.InPixel.Set(new Rect(
                mode.Position,
                mode.Pels));

            source.Orientation = mode.DisplayOrientation;
        }
        else
        {
            source.DisplayFrequency = 0;
            source.InPixel.Set(new Rect(new Point(0, 0),new Size(0, 0)));
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
        source.BackgroundColor = Color.FromRgb((byte)(color & 0xFF),(byte)((color >> 8) & 0xFF),(byte)((color >> 16) & 0xFF));

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

            var display = monitor.Connections[0].Parent;

            if (display?.CurrentMode != null)
            {
                @this.PhysicalSize.Set(
                    display.CurrentMode.DisplayOrientation % 2 == 0 
                    ? display.Capabilities.Size 
                    : display.Capabilities.Size.Transpose());
            }
            else if (monitor.Edid != null)
            {
                @this.PhysicalSize.Set(monitor.Edid.PhysicalSize);
            }

            @this.PhysicalSize.FixedAspectRatio = old;

            return @this;
        }
    }

    public static PhysicalMonitorModel SetPnpDeviceName(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        if (!string.IsNullOrEmpty(@this.PnpDeviceName)) return @this;

        var name = HtmlHelper.CleanupPnpName(monitor.Connections[0].DeviceString);
        if (name.ToLower() == "generic pnp monitor") name = monitor.Edid.Model;
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
        if (device.Edid.Model.Contains("Aorus")) return "icon/Pnp/Aorus";

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