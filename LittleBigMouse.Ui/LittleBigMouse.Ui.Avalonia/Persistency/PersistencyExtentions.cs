using System;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout;
using Microsoft.Win32;
using System.Globalization;
using System.IO;
using System.Security.Principal;
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

    public static void Load(this ILayoutOptions @this, RegistryKey? mainKey, RegistryKey? key)
    {
        if (mainKey == null) return;

        // Options to be loaded from the main key, was previously loaded from the layout key
        @this.DaemonPort = mainKey.GetOrSet("DaemonPort", () => 25196);
        @this.Priority = mainKey.GetOrSet("Priority",  () => key.GetOrSet("Priority",() => "Normal"));
        @this.PriorityUnhooked = mainKey.GetOrSet("PriorityUnhooked", () => key.GetOrSet("PriorityUnhooked",() => "Below"));

        @this.HomeCinema = mainKey.GetOrSet("HomeCinema", () => key.GetOrSet("HomeCinema",() => false));
        @this.Pinned = mainKey.GetOrSet("Pinned", () => key.GetOrSet("Pinned",() => false));
        @this.AutoUpdate = mainKey.GetOrSet("AutoUpdate", () => key.GetOrSet("AutoUpdate",() => false));
        @this.StartMinimized = mainKey.GetOrSet("StartMinimized", () => key.GetOrSet("StartMinimized",() => false));
        @this.StartElevated = mainKey.GetOrSet("StartElevated", () => key.GetOrSet("StartElevated",() => false));

        @this.ExcludedList.Clear();

        var file = @this.GetConfigPath("Excluded.txt",true);
        if (!File.Exists(file)) return;

        foreach (var line in File.ReadAllLines(file))
        {
            @this.ExcludedList.Add(line);
        }

        if (key == null) return;
        // Options to be loaded from the layout key
        @this.AllowOverlaps = key.GetOrSet("AllowOverlaps", () => false);
        @this.AllowDiscontinuity = key.GetOrSet("AllowDiscontinuity", () => false);

        @this.Algorithm = key.GetOrSet("Algorithm", () => "Strait");
        @this.MaxTravelDistance = key.GetOrSet("MaxTravelDistance", () => 200.0);
        @this.LoopX = key.GetOrSet("LoopX", () => false);
        @this.LoopY = key.GetOrSet("LoopY", () => false);

        @this.Enabled = key.GetOrSet("Enabled", () => false);
        @this.AdjustPointer = key.GetOrSet("AdjustPointer", () => false);
        @this.AdjustSpeed = key.GetOrSet("AdjustSpeed", () => false);
        @this.Priority = key.GetOrSet("Priority", () => "Normal");
        @this.PriorityUnhooked = key.GetOrSet("PriorityUnhooked", () => "Below");

        @this.Saved = true;
    }

    static bool IsProcessElevated()
    {
        using var identity = WindowsIdentity.GetCurrent();
        var principal = new WindowsPrincipal(identity);
        return principal.IsInRole(WindowsBuiltInRole.Administrator);
    }


    //==================//
    // Layout           //
    //==================//
    public static void Load(this MonitorsLayout @this)
    {
        using var mainKey = OpenRootRegKey(true);
        using var key = @this.OpenRegKey(true);

        @this.Options.LoadAtStartup = @this.IsScheduled();

        @this.Options.Elevated = IsProcessElevated();

        @this.Options.Load(mainKey, key);

        if (key != null)
        {
            foreach (var monitor in @this.PhysicalMonitors)
            {
                monitor.Model.Load();
                monitor.Load();
            }
        }
        
        @this.Saved = true;
        @this.UpdatePhysicalMonitors();
    }

    public static bool SaveEnabled(this IMonitorsLayout @this)
    {
        using var k = @this.OpenRegKey(true);
        if (k == null) return false;

        k.SetValue("Enabled", @this.Options.Enabled ? "1" : "0");

        if (@this.Options.LoadAtStartup) @this.Schedule(@this.Options.StartElevated); else @this.Unschedule();

        return true;
    }

    public static void Save(this ILayoutOptions @this, RegistryKey? mainKey, RegistryKey? key)
    {
        if (mainKey == null) return;
        mainKey.SetKey("Priority", @this.Priority);

        mainKey.SetKey("Pinned", @this.Pinned);
        mainKey.SetKey("AutoUpdate", @this.AutoUpdate);
        mainKey.SetKey("StartMinimized", @this.StartMinimized);
        mainKey.SetKey("StartElevated", @this.StartElevated);

        var file = @this.GetConfigPath("Excluded.txt",true);

        using var sw = File.CreateText(file);

        foreach (var line in @this.ExcludedList)
        {
            sw.WriteLine(line);
        }
        sw.Close();

        if (key == null) return;
        key.SetKey("AllowOverlaps", @this.AllowOverlaps);
        key.SetKey("AllowDiscontinuity", @this.AllowDiscontinuity);

        key.SetKey("Algorithm", @this.Algorithm);
        key.SetKey("MaxTravelDistance", @this.MaxTravelDistance);
        key.SetKey("LoopX", @this.LoopX);
        key.SetKey("LoopY", @this.LoopY);

        key.SetKey("Enabled",  @this.Enabled);
        key.SetKey("AdjustPointer",  @this.AdjustPointer);
        key.SetKey("AdjustSpeed", @this.AdjustSpeed );

        key.SetKey("HomeCinema", @this.HomeCinema);

        @this.Saved = true;
    }

    public static bool Save(this MonitorsLayout @this)
    {
        using var mainKey = OpenRootRegKey(true);
        using var key = @this.OpenRegKey(true);

        if (@this.Options.LoadAtStartup) @this.Schedule(@this.Options.StartElevated); else @this.Unschedule();

        @this.Options.Save(mainKey, key);

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

        @this.BorderResistance.Left = key.GetOrSet(@"BorderResistance\Left", ()=> @this.BorderResistance.Left);
        @this.BorderResistance.Top =  key.GetOrSet(@"BorderResistance\Top", ()=> @this.BorderResistance.Top);
        @this.BorderResistance.Right = key.GetOrSet(@"BorderResistance\Right", ()=> @this.BorderResistance.Right);
        @this.BorderResistance.Bottom =  key.GetOrSet(@"BorderResistance\Bottom", ()=> @this.BorderResistance.Bottom);
        @this.BorderResistance.Saved = true;

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

        key.SetKey(@"BorderResistance\Left", @this.BorderResistance.Left);
        key.SetKey(@"BorderResistance\Top", @this.BorderResistance.Top);
        key.SetKey(@"BorderResistance\Right", @this.BorderResistance.Right);
        key.SetKey(@"BorderResistance\Bottom", @this.BorderResistance.Bottom);

        @this.BorderResistance.Saved = true;


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

            // @this.DisplayName = key.GetOrSet($@"{id}\DisplayName", () => @this.DisplayName);

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