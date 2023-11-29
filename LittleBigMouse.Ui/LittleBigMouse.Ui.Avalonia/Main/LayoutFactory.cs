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

namespace LittleBigMouse.Ui.Avalonia.Main;

public static class LayoutFactory
{

    /// <summary>
    /// Update the layout from the monitors service
    /// </summary>
    /// <param name="layout"></param>
    /// <param name="service"></param>
    public static void UpdateFrom(this MonitorsLayout layout, IMonitorsSet service)
    {
        /// <summary>
        ///    0 => Stretch.None,
        ///    2 => Stretch.Fill,
        ///    6 => Stretch.Uniform,
        ///    10 => Stretch.UniformToFill,
        ///    22 => // stretched across all screens
        /// </summary>

        layout.WallpaperStyle = service.WallpaperPosition switch
        {
            DesktopWallpaperPosition.Fill => WallpaperStyle.Fill,
            DesktopWallpaperPosition.Fit => WallpaperStyle.Fit,
            DesktopWallpaperPosition.Center => WallpaperStyle.Center,
            DesktopWallpaperPosition.Tile => WallpaperStyle.Tile,
            DesktopWallpaperPosition.Span => WallpaperStyle.Span,
            _ => WallpaperStyle.Stretch
        };


        foreach (var device in service.Monitors)
        {
            //if(!layout.ShowUnattachedMonitors && !device.AttachedToDesktop) continue;

            if (!device.AttachedToDesktop) { }

            var source = layout.PhysicalSources.FirstOrDefault(s => s.DeviceId == device.DeviceId);

            if (source != null)
            {
                source.Source.UpdateFrom(device);
                continue;
            }

            var monitor = layout.PhysicalMonitors.FirstOrDefault(m =>
                m.Model.PnpCode == device.PnpCode
                && m.IdPhysicalMonitor == device.IdPhysicalMonitor
            );

            if (monitor == null)
            {
                var model = device
                    .CreatePhysicalMonitorModel();

                monitor = device
                    .CreatePhysicalMonitor(layout, model);

                source = new PhysicalSource(device.DeviceId, monitor, device.CreateDisplaySource());

                monitor.ActiveSource = source;
                monitor.Sources.Add(source);

                layout.AddOrUpdatePhysicalMonitor(monitor);
            }
            else
            {
                // new source for an existing monitor
                source = new PhysicalSource(device.DeviceId, monitor, device.CreateDisplaySource());
                monitor.Sources.Add(source);
            }

            layout.AddOrUpdatePhysicalSource(source);
        }

        // places monitors from windows configuration as best as possible
        layout.SetLocationsFromSystemConfiguration();

        //retrieve saved layout
        layout.Load();
    }

    /// <summary>
    /// Update wallpaper and background color from the system
    /// </summary>
    /// <param name="layout"></param>
    //public static void SetWallPaper(this IMonitorsLayout layout)
    //{
    //    var currentWallpaper = new string('\0', SetupApi.MAX_PATH);
    //    WinUser.SystemParametersInfo(WinUser.SystemParametersInfoAction.GetDeskWallpaper, currentWallpaper.Length, currentWallpaper, 0);

    //    layout.WallPaperPath = currentWallpaper[..currentWallpaper.IndexOf('\0')];

    //    using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop", false))
    //    {
    //        if (key != null)
    //        {
    //            layout.TileWallpaper = key.GetValue("TileWallpaper", "0").ToString() == "1";
    //            if (int.TryParse(key.GetValue("WallpaperStyle", "0").ToString(), out var value))
    //            {
    //                layout.WallpaperStyle = value;
    //            }
    //        }
    //    }

    //    using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Colors", false))
    //    {
    //        if (key == null)
    //            layout.BackgroundColor = new[] { 0, 0, 0 };
    //        else
    //        {
    //            var s = key.GetValue("Background", "0 0 0").ToString();
    //            layout.BackgroundColor = s?.Split(' ').Select(int.Parse).ToArray() ?? new[] { 0, 0, 0 };
    //        }
    //    }
    //}

    public static DisplaySource CreateDisplaySource(this MonitorDevice device)
    {
        return new DisplaySource(device.IdMonitor).UpdateFrom(device);
    }

    public static DisplaySource UpdateFrom(this DisplaySource source, MonitorDevice device)
    {
        source.DisplayName = device.AttachedDisplay?.DeviceName;
        source.DeviceName = device.AttachedDevice?.DeviceName;

        source.Primary = device.Primary;
        source.AttachedToDesktop = device.AttachedToDesktop;

        source.EffectiveDpi.Set(device.EffectiveDpi);
        source.DpiAwareAngularDpi.Set(device.AngularDpi);
        source.RawDpi.Set(device.RawDpi);

        if (device.AttachedDisplay?.CurrentMode is { } mode)
        {
            source.DisplayFrequency = mode.DisplayFrequency;

            source.InPixel.Set(new Rect(
                mode.Position,
                mode.Pels));
        }
        else
        {
            source.DisplayFrequency = 0;
            source.InPixel.Set(new Rect(new Point(0, 0),new Size(0, 0)));
        }

        (source.InterfaceName, source.InterfaceLogo) = device.InterfaceBrandNameAndLogo();

        source.WallpaperPath = device.WallpaperPath;

        return source;
    }

    public static PhysicalMonitorModel CreatePhysicalMonitorModel(this MonitorDevice device)
        => new PhysicalMonitorModel(device.PnpCode).UpdateFrom(device);

    public static PhysicalMonitorModel UpdateFrom(this PhysicalMonitorModel @this, MonitorDevice device)
    {
        using (@this.DelayChangeNotifications())
        {
            @this.SetSizeFrom(device);
            @this.SetPnpDeviceName(device);

            @this.Logo = device.BrandLogo();

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

            var display = monitor.AttachedDisplay;

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

        var name = HtmlHelper.CleanupPnpName(monitor.DeviceString);
        if (name.ToLower() == "generic pnp monitor") name = monitor.Edid.Model;
        //        if (name.ToLower() == "generic pnp monitor") name = HtmlHelper.GetPnpName(@this.PnpCode);

        @this.PnpDeviceName = name;

        return @this;
    }

    public static PhysicalMonitor CreatePhysicalMonitor(this MonitorDevice device, IMonitorsLayout layout, PhysicalMonitorModel model)
        => new PhysicalMonitor(device.IdPhysicalMonitor, layout, model).UpdateFrom(device);

    public static PhysicalMonitor UpdateFrom(this PhysicalMonitor monitor, MonitorDevice device)
    {
        if (device.IdPhysicalMonitor != monitor.IdPhysicalMonitor)
            throw new Exception("Cannot update monitor from different device");

        // Serial Number
        monitor.SerialNumber = device.Edid?.SerialNumber ?? "N/A";

        // Orientation
        var display = device.AttachedDisplay;
        if (display?.CurrentMode is { } mode)
        {
            monitor.Orientation = mode.DisplayOrientation;
        }


        monitor.Load();

        return monitor;
    }

    static string BrandLogo(this MonitorDevice device)
    {
        var dev = device.AttachedDevice?.Parent.DeviceString;
        if (dev != null)
        {
            // special case for Spacedesk support
            if (dev.Contains("spacedesk", StringComparison.OrdinalIgnoreCase)) return "icon/Pnp/Spacedesk";
            if (dev == "Microsoft Remote Display Adapter") return "icon/Pnp/Windows";
        }

        return $"icon/Pnp/{device.Edid?.ManufacturerCode ?? "LBM"}";
    }

    static readonly string[] Brands = { "intel", "amd", "nvidia", "microsoft" };
    public static (string, string) InterfaceBrandNameAndLogo(this MonitorDevice device)
    {
        if(device.AttachedDevice == null) return ("detached", "icon/parts/detached");

        var dev = device.AttachedDevice?.Parent?.DeviceString?.ToLower() ?? "";

        foreach (var brand in Brands)
        {
            if (dev.Contains(brand)) return (dev, $"icon/pnp/{brand}");
        }
        return (dev, "");
    }

}