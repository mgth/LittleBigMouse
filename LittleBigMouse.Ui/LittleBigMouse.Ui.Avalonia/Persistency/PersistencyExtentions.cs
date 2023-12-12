using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout;
using Microsoft.Win32;
using System.Globalization;
using Avalonia;
using LittleBigMouse.DisplayLayout.Dimensions;
#pragma warning disable CA1416

namespace LittleBigMouse.Ui.Avalonia.Persistency;

public static class PersistencyExtensions
{
    const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";
    public static RegistryKey? OpenRegKey(this RegistryKey @this, string key, bool create = false) 
        => create ? @this.CreateSubKey(key) : @this.OpenSubKey(key);

    public static RegistryKey? OpenRootRegKey(bool create = false)
    {
        using var key = Registry.CurrentUser;
        return create ? key.CreateSubKey(ROOT_KEY) : key.OpenSubKey(ROOT_KEY);
    }

    public static RegistryKey? OpenRegKey(this IMonitorsLayout layout,  bool create = false)
    {
        using var key = OpenRootRegKey(create);
        return key?.OpenRegKey(@$"Layouts\{layout.Id}", create);
    }

    //==================//
    // Layout           //
    //==================//
    public static void Load(this MonitorsLayout @this)
    {
        using (var key = @this.OpenRegKey(true))
        {
            if (key != null)
            {
                @this.Enabled = key.GetOrSet("Enabled", () => false);
                @this.AdjustPointer = key.GetOrSet("AdjustPointer", () => false);
                @this.AdjustSpeed = key.GetOrSet("AdjustSpeed", () => false);
                @this.Algorithm = key.GetOrSet("Algorithm", () => "Strait");
                @this.Priority = key.GetOrSet("Priority", () => "Normal");
                @this.AllowOverlaps = key.GetOrSet("AllowOverlaps", () => false);
                @this.AllowDiscontinuity = key.GetOrSet("AllowDiscontinuity", () => false);
                @this.HomeCinema = key.GetOrSet("HomeCinema", () => false);
                @this.Pinned = key.GetOrSet("Pinned", () => false);
                @this.LoopX = key.GetOrSet("LoopX", () => false);
                @this.LoopY = key.GetOrSet("LoopY", () => false);
                @this.AutoUpdate = key.GetOrSet("AutoUpdate", () => false);
                @this.MaxTravelDistance = key.GetOrSet("MaxTravelDistance", () => 200.0);
            }

            @this.LoadAtStartup = @this.IsScheduled();

            if (key != null)
            {
                foreach (var monitor in @this.PhysicalMonitors)
                {
                    monitor.Model.Load();
                    monitor.Load();
                }
            }
        }
        @this.Saved = true;
        @this.UpdatePhysicalMonitors();

    }

    public static bool SaveEnabled(this IMonitorsLayout @this)
    {
        using var k = @this.OpenRegKey(true);
        if (k == null) return false;

        k.SetValue("Enabled", @this.Enabled ? "1" : "0");

        if (@this.LoadAtStartup) @this.Schedule(); else @this.Unschedule();

        //@this.Saved = true;
        return true;
    }

    public static bool Save(this MonitorsLayout @this)
    {
        using var k = @this.OpenRegKey(true);
        if (k == null) return false;

        k.SetKey("Enabled",  @this.Enabled);
        k.SetKey("AdjustPointer",  @this.AdjustPointer);
        k.SetKey("AdjustSpeed", @this.AdjustSpeed );
        k.SetKey("Algorithm", @this.Algorithm);
        k.SetKey("AllowOverlaps", @this.AllowOverlaps);
        k.SetKey("AllowDiscontinuity", @this.AllowDiscontinuity);
        k.SetKey("HomeCinema", @this.HomeCinema);
        k.SetKey("Pinned", @this.Pinned);
        k.SetKey("LoopX", @this.LoopX);
        k.SetKey("LoopY", @this.LoopY);
        k.SetKey("AutoUpdate", @this.AutoUpdate);
        k.SetKey("MaxTravelDistance", @this.MaxTravelDistance);

        if (@this.LoadAtStartup) @this.Schedule(); else @this.Unschedule();

        foreach (var monitor in @this.PhysicalMonitors)
        {
            monitor.Save();
            monitor.Model.Save();
        }

        @this.Saved = true;
        return true;
    }



    // TODO : public RegistryKey OpenRegKey(bool create = false) => OpenRegKey(Layout.OpenRegKey(create), create);// OpenRegKey(Layout.Id, Device.IdPhysicalMonitor, create);

    //==================//
    // Physical Monitor //
    //==================//

    public static RegistryKey? OpenRegKey(this PhysicalMonitor @this, bool create = false)
    {
        using var key = @this.Layout.OpenRegKey(create);
        return key?.OpenRegKey(@$"PhysicalMonitors\{@this.Id}", create);
    }

    public static PhysicalMonitor Load(this PhysicalMonitor @this)
    {
        using var key = @this.OpenRegKey(true);

        var active = key.GetOrSet("ActiveSource", () => @this.ActiveSource?.Source.Id??"");

        foreach (var source in @this.Sources.Items)
        {
            source.Source.Load(key);
            if (source.Source.Id != active && @this.ActiveSource != null) continue;

            @this.ActiveSource = source; 
            key.SetKey("ActiveSource",source.Source.Id);
        }

        @this.DepthProjection.X = key.GetOrSet("XLocationInMm", () => @this.DepthProjection.X, () => @this.Placed = true);
        @this.DepthProjection.Y = key.GetOrSet("YLocationInMm", () => @this.DepthProjection.Y, () => @this.Placed = true);

        @this.DepthProjection.Saved = true;

        @this.DepthRatio.X = key.GetOrSet("PhysicalRatioX", () => @this.DepthRatio.X);
        @this.DepthRatio.Y = key.GetOrSet("PhysicalRatioY", () => @this.DepthRatio.Y);

        @this.DepthRatio.Saved = true;

        @this.Saved = true;

        return @this;
    }

    public static void Save(this PhysicalMonitor @this)
    {
        using var key = @this.OpenRegKey(true);

        if (key == null) return;

        key.SetKey("XLocationInMm", @this.DepthProjection.X);
        key.SetKey("YLocationInMm", @this.DepthProjection.Y);

        @this.DepthProjection.Saved = true;

        key.SetKey("PhysicalRatioX", @this.DepthRatio.X);
        key.SetKey("PhysicalRatioY", @this.DepthRatio.Y);

        @this.DepthRatio.Saved = true;

        foreach (var source in @this.Sources.Items)
        {
            source.Source.Save(key);
        }

        key.SetKey("ActiveSource", @this.ActiveSource.Source.Id);
        key.SetKey("SerialNumber", @this.SerialNumber);

        @this.Saved = true;
    }

    //========================//
    // Physical Monitor Model //
    //========================//
    public static RegistryKey? OpenMonitorRegKey(this PhysicalMonitorModel @this, bool create = false)
    {
        using var key = OpenRootRegKey(create);
        return key?.OpenRegKey(@$"monitors\{@this.PnpCode}", create);
    }

    public static PhysicalMonitorModel Load(this PhysicalMonitorModel @this)
    {
        var old = @this.PhysicalSize.FixedAspectRatio;
        @this.PhysicalSize.FixedAspectRatio = false;

        using (var key = @this.OpenMonitorRegKey(true))
        {
            if (key != null)
            {
                @this.PhysicalSize.TopBorder = key.GetOrSet(@"Borders\Top", ()=>@this.PhysicalSize.TopBorder);
                @this.PhysicalSize.RightBorder = key.GetOrSet(@"Borders\Right", ()=>@this.PhysicalSize.RightBorder);
                @this.PhysicalSize.BottomBorder = key.GetOrSet(@"Borders\Bottom", ()=>@this.PhysicalSize.BottomBorder);
                @this.PhysicalSize.LeftBorder = key.GetOrSet(@"Borders\Left", ()=>@this.PhysicalSize.LeftBorder);

                @this.PhysicalSize.Height = key.GetOrSet(@"Size\Height", ()=>@this.PhysicalSize.Height);
                @this.PhysicalSize.Width = key.GetOrSet(@"Size\Width", ()=>@this.PhysicalSize.Width);

                @this.PhysicalSize.Saved = true;

                @this.PnpDeviceName = key.GetOrSet("PnpName", ()=>@this.PnpDeviceName);

                //key.SetKey("DeviceId", Monitor.DeviceId);

            }

            @this.PhysicalSize.FixedAspectRatio = old;
        }
        @this.Saved = true;

        return @this;
    }

    public static void Save(this PhysicalMonitorModel @this)
    {
        //if (@this.Saved) return;

        using var key = @this.OpenMonitorRegKey(true);

        if (key == null) return;

        key.SetKey(@"Borders\Top", @this.PhysicalSize.TopBorder.ToString(CultureInfo.InvariantCulture));
        key.SetKey(@"Borders\Right", @this.PhysicalSize.RightBorder.ToString(CultureInfo.InvariantCulture));
        key.SetKey(@"Borders\Bottom", @this.PhysicalSize.BottomBorder.ToString(CultureInfo.InvariantCulture));
        key.SetKey(@"Borders\Left", @this.PhysicalSize.LeftBorder.ToString(CultureInfo.InvariantCulture));

        key.SetKey(@"Size\Height", @this.PhysicalSize.Height.ToString(CultureInfo.InvariantCulture));
        key.SetKey(@"Size\Width", @this.PhysicalSize.Width.ToString(CultureInfo.InvariantCulture));

        @this.PhysicalSize.Saved = true;

        key.SetKey("PnpName", @this.PnpDeviceName);

        @this.Saved = true;
    }

    //================//
    // Display Source //
    //================//
    public static void Load(this DisplaySource @this, RegistryKey key)
    {

        var id = @this.Id;

        if(!@this.AttachedToDesktop)
        {
            var x = key.GetOrSet($@"{id}\PixelX", () => @this.InPixel.X);
            var y = key.GetOrSet($@"{id}\PixelY", () => @this.InPixel.Y);
            var width = key.GetOrSet($@"{id}\PixelWidth", () => @this.InPixel.Width);
            var height = key.GetOrSet($@"{id}\PixelHeight", () => @this.InPixel.Height);

            var orientation = key.GetOrSet($@"{id}\Orientation", () => @this.Orientation);

            @this.InPixel.Set(new Rect(new Point(x, y), new Size(width, height)));

            @this.Orientation = orientation;

            @this.DisplayName = key.GetOrSet($@"{id}\DisplayName", () => @this.DisplayName);

            @this.Saved = true;
        }
        else @this.Save(key);

    }

    public static void Save(this DisplaySource @this, RegistryKey? key)
    {
        var id = @this.Id;

        // This values are stored in order to be retrieved to be restored when the monitor is re-attached
        if(@this.AttachedToDesktop)
        {
            key.SetKey($@"{id}\PixelX", @this.InPixel.X);
            key.SetKey($@"{id}\PixelY", @this.InPixel.Y);
            key.SetKey($@"{id}\PixelWidth", @this.InPixel.Width);
            key.SetKey($@"{id}\PixelHeight", @this.InPixel.Height);
            key.SetKey($@"{id}\Orientation", @this.Orientation);
            @this.InPixel.Saved = true;

            key.SetKey($@"{id}\DisplayName", @this.DisplayName);
            key.SetKey($@"{id}\Primary", @this.Primary);
        }
        @this.Saved = true;
    }

}