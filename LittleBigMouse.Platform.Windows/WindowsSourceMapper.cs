#nullable enable
using System;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Maps a single Win32 <see cref="MonitorDevice"/> onto the neutral model objects
/// (<see cref="DisplaySource"/>, <see cref="PhysicalMonitor"/>, <see cref="PhysicalMonitorModel"/>).
/// This is the per-monitor field-by-field copy: identity, DPI, mode/pixel rect, orientation,
/// serial and PnP name, plus the brand/logo lookups. The physical-size inference lives in
/// <see cref="WindowsPhysicalSize"/> and the wallpaper copy in <see cref="WindowsWallpaperMapping"/>;
/// this class wires them into the model together with everything else a source carries.
/// </summary>
internal static class WindowsSourceMapper
{
    public static DisplaySource CreateDisplaySource(this MonitorDevice monitor)
    {
        return new DisplaySource(monitor.SourceId).UpdateFrom(monitor);
    }

    public static DisplaySource UpdateFrom(this DisplaySource source, MonitorDevice monitor)
    {
        source.InterfacePath = monitor.InterfacePath;

        if (monitor.ActiveConnection is not { } device) return source;

        if (device.Parent == null) return source;

        source.DisplayName = device.Parent.DeviceName;
        source.DeviceName = device.DeviceName;

        source.SourceName = $"{monitor.Edid?.VideoInterface ?? "Unknown"}:{device.DeviceName}";


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

            source.Orientation = WindowsPhysicalSize.InferOrientation(mode, monitor.Edid);
        }
        else
        {
            source.DisplayFrequency = 0;
            source.InPixel.Set(new HLab.Geo.Rect(new HLab.Geo.Point(0, 0), new HLab.Geo.Size(0, 0)));
        }

        (source.InterfaceName, source.InterfaceLogo) = device.Parent.InterfaceBrandNameAndLogo();

        source.UpdateWallpaperFrom(monitor);

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

    public static PhysicalMonitorModel SetSizeFrom(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        using (@this.PhysicalSize.DelayChangeNotifications())
        {
            var old = @this.PhysicalSize.FixedAspectRatio;
            @this.PhysicalSize.FixedAspectRatio = false;

            var (width, height) = WindowsPhysicalSize.GetPhysicalSizeInMm(monitor);
            if (width > 0 && height > 0)
            {
                @this.PhysicalSize.Width = width;
                @this.PhysicalSize.Height = height;
            }

            @this.PhysicalSize.FixedAspectRatio = old;

            return @this;
        }
    }

    public static PhysicalMonitorModel SetPnpDeviceName(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        if (!string.IsNullOrEmpty(@this.PnpDeviceName)) return @this;

        var name = PnpName.Cleanup(monitor.ActiveConnection?.DeviceString ?? "");
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
        var dev = device.ActiveConnection?.Parent?.DeviceString;
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
        if (adapter.Parent == null) return ("detached", "icon/parts/detached");

        var dev = adapter.DeviceString?.ToLower() ?? "";

        foreach (var brand in Brands)
        {
            if (dev.Contains(brand)) return (dev, $"icon/pnp/{brand}");
        }
        return (dev, "");
    }
}
