using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Ui.Avalonia.Main;
using Microsoft.Win32;
using System.Globalization;
using System.Runtime.InteropServices;
using HLab.Sys.Windows.Monitors;
#pragma warning disable CA1416

namespace LittleBigMouse.Ui.Avalonia.Persistency;

public static class PersistencyExtensions
{
    const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

    public static RegistryKey? OpenRootRegKey(bool create = false)
    {
        using var key = Registry.CurrentUser;
        return create ? key.CreateSubKey(ROOT_KEY) : key.OpenSubKey(ROOT_KEY);
    }

    public static RegistryKey? OpenRegKey(string layoutId, bool create = false)
    {
        using var key = OpenRootRegKey(create);

        if (key == null) return null;
        return create ? key.CreateSubKey(@"Layouts\" + layoutId) : key.OpenSubKey(@"Layouts\" + layoutId);
    }

    public static RegistryKey? OpenRegKey(this IMonitorsLayout layout,  bool create = false) 
        => OpenRegKey(layout.Id, create);

    //==================//
    // Layout           //
    //==================//
    public static void Load(this MonitorsLayout @this)
    {
        using (var key = @this.OpenRegKey())
        {
            if (key != null)
            {
                @this.Enabled = key.GetValue("Enabled", 0).ToString() == "1";
                @this.AdjustPointer = key.GetValue("AdjustPointer", 0).ToString() == "1";
                @this.AdjustSpeed = key.GetValue("AdjustSpeed", 0).ToString() == "1";
                @this.AllowCornerCrossing = key.GetValue("AllowCornerCrossing", 0).ToString() == "1";
                @this.AllowOverlaps = key.GetValue("AllowOverlaps", 0).ToString() == "1";
                @this.AllowDiscontinuity = key.GetValue("AllowDiscontinuity", 0).ToString() == "1";
                @this.HomeCinema = key.GetValue("HomeCinema", 0).ToString() == "1";
                @this.Pinned = key.GetValue("Pinned", 0).ToString() == "1";
                @this.LoopX = key.GetValue("LoopX", 0).ToString() == "1";
                @this.LoopY = key.GetValue("LoopY", 0).ToString() == "1";
                @this.AutoUpdate = key.GetValue("AutoUpdate", 0).ToString() == "1";
            }

            @this.LoadAtStartup = @this.IsScheduled();

            if (key != null)
            {
                foreach (var s in @this.PhysicalMonitors.Items)
                {
                    s.Load();
                }
            }
        }
        @this.Saved = true;
    }

    public static bool Save(this MonitorsLayout @this)
    {
        using var k = @this.OpenRegKey(true);
        if (k == null) return false;

        k.SetValue("Enabled",  @this.Enabled ? "1" : "0");
        k.SetValue("AdjustPointer",  @this.AdjustPointer ? "1" : "0");
        k.SetValue("AdjustSpeed", @this.AdjustSpeed ? "1" : "0");
        k.SetValue("AllowCornerCrossing", @this.AllowCornerCrossing ? "1" : "0");
        k.SetValue("AllowOverlaps", @this.AllowOverlaps ? "1" : "0");
        k.SetValue("AllowDiscontinuity", @this.AllowDiscontinuity ? "1" : "0");
        k.SetValue("HomeCinema", @this.HomeCinema ? "1" : "0");
        k.SetValue("Pinned", @this.Pinned ? "1" : "0");
        k.SetValue("LoopX", @this.LoopX ? "1" : "0");
        k.SetValue("LoopY", @this.LoopY ? "1" : "0");
        k.SetValue("AutoUpdate", @this.AutoUpdate ? "1" : "0");

        if (@this.LoadAtStartup) @this.Schedule(); else @this.Unschedule();

        foreach (var monitor in @this.PhysicalMonitors.Items)
        {
            monitor.Save();
            monitor.Model.Save();
        }

        @this.Saved = true;
        return true;
    }


    public static RegistryKey OpenRegKey(this RegistryKey @this, string key, bool create = false) 
        => create ? @this.CreateSubKey(key) : @this.OpenSubKey(key);

    // TODO : public RegistryKey OpenRegKey(bool create = false) => OpenRegKey(Layout.OpenRegKey(create), create);// OpenRegKey(Layout.Id, Device.IdPhysicalMonitor, create);

    //==================//
    // Physical Monitor //
    //==================//

    public static RegistryKey? OpenRegKey(this PhysicalMonitor @this, bool create = false)
    {
        using var key = @this.Layout.OpenRegKey(create);
        return key?.OpenRegKey(@"PhysicalMonitors\" + @this.IdPhysicalMonitor, create);
    }

    public static void Load(this PhysicalMonitor @this)
    {
        using var key = @this.OpenRegKey();

        if (key != null)
        {
            @this.DepthProjection.X = key.GetKey("XLocationInMm", () => @this.DepthProjection.X, () => @this.Placed = true);
            @this.DepthProjection.Y = key.GetKey("YLocationInMm", () => @this.DepthProjection.Y, () => @this.Placed = true);
            @this.DepthRatio.X = key.GetKey("PhysicalRatioX", () => @this.DepthRatio.X);
            @this.DepthRatio.Y = key.GetKey("PhysicalRatioY", () => @this.DepthRatio.Y);
        }

        var active = key.GetKey("ActiveSource", () => "");
        foreach (var source in @this.Sources.Items)
        {
            source.Source.Load(key);
            if (source.Source.IdMonitor == active || @this.ActiveSource == null)
                @this.ActiveSource = source;
        }
    }

    public static void Save(this PhysicalMonitor @this)
    {
        using var key = @this.OpenRegKey(true);

        if (key == null) return;

        key.SetKey("XLocationInMm", @this.DepthProjection.X);
        key.SetKey("YLocationInMm", @this.DepthProjection.Y);
        key.SetKey("PhysicalRatioX", @this.DepthRatio.X);
        key.SetKey("PhysicalRatioY", @this.DepthRatio.Y);

        foreach (var source in @this.Sources.Items)
        {
            source.Source.Save(key);
        }

        key.SetKey("ActiveSource", @this.ActiveSource.Source.IdMonitor);
        key.SetKey("Orientation", @this.Orientation);
    }

    public static void Save(this PhysicalMonitorModel @this)
    {
        if (@this.Saved) return;

        using var key = @this.OpenMonitorRegKey(true);
        if (key == null) return;

        key.SetKey("TopBorder", @this.PhysicalSize.TopBorder.ToString(CultureInfo.InvariantCulture));
        key.SetKey("RightBorder", @this.PhysicalSize.RightBorder.ToString(CultureInfo.InvariantCulture));
        key.SetKey("BottomBorder", @this.PhysicalSize.BottomBorder.ToString(CultureInfo.InvariantCulture));
        key.SetKey("LeftBorder", @this.PhysicalSize.LeftBorder.ToString(CultureInfo.InvariantCulture));

        key.SetKey("Height", @this.PhysicalSize.Height.ToString(CultureInfo.InvariantCulture));
        key.SetKey("Width", @this.PhysicalSize.Width.ToString(CultureInfo.InvariantCulture));

        key.SetKey("PnpName", @this.PnpDeviceName);

        @this.Saved = true;
    }

    public static void Save(this DisplaySource @this, RegistryKey? baseKey)
    {
        using var key = baseKey?.CreateSubKey("GuiLocation");

        if (key == null) return;

        key.SetKey("Left", @this.GuiLocation.Left);
        key.SetKey("Width", @this.GuiLocation.Width);
        key.SetKey("Top", @this.GuiLocation.Top);
        key.SetKey("Height", @this.GuiLocation.Height);

        // TODO avalonia

        //using (var key = baseKey.CreateSubKey(Device.IdMonitor))
        //{
        //    if (key == null) return;

        //    key.SetKey("PixelX", InPixel.X);
        //    key.SetKey("PixelY", InPixel.Y);
        //    key.SetKey("PixelWidth", InPixel.Width);
        //    key.SetKey("PixelHeight", InPixel.Height);

        //    key.SetKey("Primary", Primary);
        //}
    }
    public static RegistryKey? OpenMonitorRegKey(string id, bool create = false)
    {
        using var key = OpenRootRegKey(create);
        if (key == null) return null;
        return create ? key.CreateSubKey(@"monitors\" + id) : key.OpenSubKey(@"monitors\" + id);
    }
    public static RegistryKey? OpenMonitorRegKey(this PhysicalMonitorModel @this, bool create = false) => OpenMonitorRegKey(@this.PnpCode, create);
    public static PhysicalMonitorModel Load(this PhysicalMonitorModel @this, MonitorDevice monitor)
    {
        var display = monitor.AttachedDisplay;
        var old = @this.PhysicalSize.FixedAspectRatio;
        @this.PhysicalSize.FixedAspectRatio = false;

        @this.SetSizeFrom(monitor);

        using (var key = @this.OpenMonitorRegKey(false))
        {
            if (key != null)
            {
                @this.PhysicalSize.TopBorder = double.Parse(key.GetValue("TopBorder", @this.PhysicalSize.TopBorder).ToString(), CultureInfo.InvariantCulture);
                @this.PhysicalSize.RightBorder = double.Parse(key.GetValue("RightBorder", @this.PhysicalSize.RightBorder).ToString(), CultureInfo.InvariantCulture);
                @this.PhysicalSize.BottomBorder = double.Parse(key.GetValue("BottomBorder", @this.PhysicalSize.BottomBorder).ToString(), CultureInfo.InvariantCulture);
                @this.PhysicalSize.LeftBorder = double.Parse(key.GetValue("LeftBorder", @this.PhysicalSize.LeftBorder).ToString(), CultureInfo.InvariantCulture);

                @this.PhysicalSize.Height = double.Parse(key.GetValue("Height", @this.PhysicalSize.Height).ToString(), CultureInfo.InvariantCulture);
                @this.PhysicalSize.Width = double.Parse(key.GetValue("Whidth", @this.PhysicalSize.Width).ToString(), CultureInfo.InvariantCulture);

                @this.PnpDeviceName = key.GetValue("PnpName", "").ToString();

                //key.SetKey("DeviceId", Monitor.DeviceId);

            }

            @this.SetPnpDeviceName(monitor);

            @this.PhysicalSize.FixedAspectRatio = old;
        }
        @this.Saved = true;

        return @this;
    }

}