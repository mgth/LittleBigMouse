using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Ui.Avalonia.Main;
using Microsoft.Win32;
using System.Globalization;
using System.Runtime.InteropServices;
using Avalonia;
using HLab.Sys.Windows.Monitors;
using static ExCSS.AttributeSelectorFactory;
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
        return key?.OpenRegKey(@"Layouts\Default", create);
    }

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
        using var key = @this.OpenRegKey();

        if (key == null) return @this;

        @this.DepthProjection.X = key.GetKey("XLocationInMm", () => @this.DepthProjection.X, () => @this.Placed = true);
        @this.DepthProjection.Y = key.GetKey("YLocationInMm", () => @this.DepthProjection.Y, () => @this.Placed = true);
        @this.DepthRatio.X = key.GetKey("PhysicalRatioX", () => @this.DepthRatio.X);
        @this.DepthRatio.Y = key.GetKey("PhysicalRatioY", () => @this.DepthRatio.Y);

        var active = key.GetKey("ActiveSource", () => "");
        foreach (var source in @this.Sources.Items)
        {
            source.Source.Load(key);
            if (source.Source.IdMonitorDevice == active || @this.ActiveSource == null)
                @this.ActiveSource = source;
        }

        return @this;
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

        key.SetKey("ActiveSource", @this.ActiveSource.Source.IdMonitorDevice);
        key.SetKey("Orientation", @this.Orientation);
        key.SetKey("SerialNumber", @this.SerialNumber);
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

        using (var key = @this.OpenMonitorRegKey(false))
        {
            if (key != null)
            {
                @this.PhysicalSize.TopBorder = key.GetKey(@"Borders\Top", ()=>@this.PhysicalSize.TopBorder);
                @this.PhysicalSize.RightBorder = key.GetKey(@"Borders\Right", ()=>@this.PhysicalSize.RightBorder);
                @this.PhysicalSize.BottomBorder = key.GetKey(@"Borders\Bottom", ()=>@this.PhysicalSize.BottomBorder);
                @this.PhysicalSize.LeftBorder = key.GetKey(@"Borders\Left", ()=>@this.PhysicalSize.LeftBorder);

                @this.PhysicalSize.Height = key.GetKey(@"Size\Height", ()=>@this.PhysicalSize.Height);
                @this.PhysicalSize.Width = key.GetKey(@"Size\Width", ()=>@this.PhysicalSize.Width);

                @this.PnpDeviceName = key.GetKey("PnpName", ()=>@this.PnpDeviceName);

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

        key.SetKey("PnpName", @this.PnpDeviceName);

        @this.Saved = true;
    }

    //================//
    // Display Source //
    //================//
    public static void Load(this DisplaySource @this, RegistryKey baseKey)
    {
        using var key = baseKey.OpenSubKey("GuiLocation");

        if (key == null) return;

        var left = key.GetKey("Left", () => @this.GuiLocation.Left);
        var width = key.GetKey("Width", () => @this.GuiLocation.Width);
        var top = key.GetKey("Top", () => @this.GuiLocation.Top);
        var height = key.GetKey("Height", () => @this.GuiLocation.Height);

        @this.GuiLocation = new Rect(new Point(left, top), new Size(width, height));
    }


    public static void Save(this DisplaySource @this, RegistryKey? baseKey)
    {
        using var key = baseKey?.CreateSubKey("GuiLocation");

        if (key == null) return;

        key.SetKey("Left", @this.GuiLocation.Left);
        key.SetKey("Width", @this.GuiLocation.Width);
        key.SetKey("Top", @this.GuiLocation.Top);
        key.SetKey("Height", @this.GuiLocation.Height);

        using (var key2 = baseKey.CreateSubKey(@this.IdMonitorDevice))
        {
            if (key2 == null) return;

            key2.SetKey("PixelX", @this.InPixel.X);
            key2.SetKey("PixelY", @this.InPixel.Y);
            key2.SetKey("PixelWidth", @this.InPixel.Width);
            key2.SetKey("PixelHeight", @this.InPixel.Height);

            key2.SetKey("Primary", @this.Primary);
        }

        @this.Saved = true;
    }

}