using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Management;
using System.Reactive.Linq;
using System.Runtime.InteropServices;
using System.Text;
using DynamicData;
using ReactiveUI;
using Avalonia;
using Microsoft.Win32;
using HLab.ColorTools.Avalonia;
using HLab.Sys.Windows.API;

using static HLab.Sys.Windows.API.ShellScalingApi;
using static HLab.Sys.Windows.API.WinGdi;
using static HLab.Sys.Windows.API.WinUser;

namespace HLab.Sys.Windows.Monitors;

public static class MonitorDeviceHelper
{

    public static DisplayMode GetDisplayMode(DevMode dm)
    {
        return new DisplayMode
        {
            DisplayOrientation = (int)(((dm.Fields & DisplayModeFlags.DisplayOrientation) != 0)
                ? dm.DisplayOrientation : DevMode.DisplayOrientationEnum.Default),

            Position = ((dm.Fields & DisplayModeFlags.Position) != 0)
                ? new Point(dm.Position.X, dm.Position.Y) : new Point(0, 0),

            BitsPerPixel = ((dm.Fields & DisplayModeFlags.BitsPerPixel) != 0)
                ? dm.BitsPerPel : 0,

            Pels = ((dm.Fields & (DisplayModeFlags.PixelsWidth | DisplayModeFlags.PixelsHeight)) != 0)
                ? new Size(dm.PixelsWidth, dm.PixelsHeight) : new Size(1, 1),

            DisplayFlags = (int)((dm.Fields & DisplayModeFlags.DisplayFlags) != 0
                ? dm.DisplayFlags : 0),

            DisplayFrequency = (int)(((dm.Fields & DisplayModeFlags.DisplayFrequency) != 0)
                ? dm.DisplayFrequency : 0),

            DisplayFixedOutput = (int)(((dm.Fields & DisplayModeFlags.DisplayFixedOutput) != 0)
                ? dm.DisplayFixedOutput : 0),
        };
    }

    public static void UpdateDevModes(this DisplayDevice @this)
    {
        var devMode = new DevMode();

        var i = 0;
        while (EnumDisplaySettingsEx(@this.DeviceName, i, ref devMode, 0))
        {
            @this.DisplayModes.Add(GetDisplayMode(devMode));
            i++;
        }
    }

    public static DisplayMode GetCurrentMode(string deviceName)
    {
        var devMode = new DevMode(DevMode.SpecVersionEnum.Win8);

        return !EnumDisplaySettingsEx(deviceName, -1, ref devMode, 0) ? null : GetDisplayMode(devMode);
    }

    public static DeviceState BuildDeviceState(WinGdi.DisplayDevice @this)
    {
        var state = @this.StateFlags;

        return new DeviceState()
        {
            AttachedToDesktop = (state & DisplayDeviceStateFlags.AttachedToDesktop) != 0,
            MultiDriver = (state & DisplayDeviceStateFlags.MultiDriver) != 0,
            PrimaryDevice = (state & DisplayDeviceStateFlags.PrimaryDevice) != 0,
            MirroringDriver = (state & DisplayDeviceStateFlags.MirroringDriver) != 0,
            VgaCompatible = (state & DisplayDeviceStateFlags.VgaCompatible) != 0,
            Removable = (state & DisplayDeviceStateFlags.Removable) != 0,
            ModesPruned = (state & DisplayDeviceStateFlags.ModesPruned) != 0,
            Remote = (state & DisplayDeviceStateFlags.Remote) != 0,
            Disconnect = (state & DisplayDeviceStateFlags.Disconnect) != 0,
        };
    }

    public static DeviceCaps BuildDeviceCaps(this WinGdi.DisplayDevice @this)
    {
        var hdc = CreateDC("DISPLAY", @this.DeviceName, null, 0);
        try
        {
            return new DeviceCaps()
            {
                Size = new Size(
                    GetDeviceCaps(hdc, DeviceCap.HorzSize),
                    GetDeviceCaps(hdc, DeviceCap.VertSize)
                ),

                Resolution = new Size(
                    GetDeviceCaps(hdc, DeviceCap.HorzRes),
                    GetDeviceCaps(hdc, DeviceCap.VertRes)
                ),

                LogPixels = new Size(
                    GetDeviceCaps(hdc, DeviceCap.LogPixelsX),
                    GetDeviceCaps(hdc, DeviceCap.LogPixelsY)
                ),

                BitsPixel = GetDeviceCaps(hdc, DeviceCap.BitsPixel),

                Aspect = new Size(
                    GetDeviceCaps(hdc, DeviceCap.AspectX),
                    GetDeviceCaps(hdc, DeviceCap.AspectY)
                )
            };
        }
        finally
        {
            DeleteDC(hdc);
        }

        // TODO : https://msdn.microsoft.com/en-us/library/windows/desktop/dd144877(v=vs.85).aspx

    }

    public static void Init(this DisplayDevice @this
        , DisplayDevice parent
        , WinGdi.DisplayDevice dev
        , IList<DisplayDevice> oldDevices
        , IList<MonitorDevice> oldMonitors
        , MonitorsService service)
    {
        @this.Parent = parent;

        var deviceId = @this.DeviceId = dev.DeviceID;
        var deviceString = @this.DeviceString = dev.DeviceString;

        var deviceKey = @this.DeviceKey = dev.DeviceKey;
        var deviceName = @this.DeviceName = dev.DeviceName;

        @this.State = BuildDeviceState(dev);

        @this.Capabilities = dev.BuildDeviceCaps();

        switch (deviceId.Split('\\')[0])
        {
            case "ROOT":
                break;

            case "MONITOR":

                var monitor = service.GetOrAddMonitor(deviceId, (id) =>
                {
                    var m = BuildFromId(id);
                    return m;
                });

                monitor.DeviceKey = deviceKey;
                monitor.DeviceString = deviceString;

                if (@this.State.AttachedToDesktop)
                {
                    monitor.AttachedDisplay = parent;
                    monitor.AttachedDevice = @this;
                }
                else
                {
                    monitor.AttachedDisplay = null;
                    monitor.AttachedDevice = null;
                }
                monitor.AttachedToDesktop = @this.State.AttachedToDesktop;

                var idx = oldMonitors.IndexOf(monitor);
                if (idx >= 0) oldMonitors.RemoveAt(idx);
                break;

            case "PCI":
            case "RdpIdd_IndirectDisplay":
            case string s when s.StartsWith("VID_DATRONICSOFT_PID_SPACEDESK_VIRTUAL_DISPLAY_"):

                var adapter = service.GetOrAddAdapter(deviceId, (id) => new PhysicalAdapter(id)
                {
                    DeviceString = deviceString
                });

                @this.CurrentMode = GetCurrentMode(@this.DeviceName);

                break;
            default:
                break;
        }

        uint i = 0;
        var child = new WinGdi.DisplayDevice();

        while (EnumDisplayDevices(deviceName, i++, ref child, 0))
        {
            var c = child;
            var device = service.GetOrAddDevice(c.DeviceName,
                (id) => new DisplayDevice(id));

            oldDevices.Remove(device);
            device.Init(@this, c, oldDevices, oldMonitors, service);
            child = new WinGdi.DisplayDevice();
        }
    }


    public static void UpdateFromMonitorInfo(this MonitorDevice @this, WinUser.MonitorInfoEx mi, IEnumerable<MonitorDevice> monitors)
    {
        @this.SetPrimary(monitors, mi.Flags == 1);
        @this.MonitorArea = mi.Monitor.ToRect();
        @this.WorkArea = mi.WorkArea.ToRect();
    }

    public static void UpdateFromMonitorInfo(this MonitorDevice @this, WinUser.MonitorInfo mi, IEnumerable<MonitorDevice> monitors)
    {
        @this.SetPrimary(monitors, mi.Flags == 1);
        @this.MonitorArea = mi.Monitor.ToRect();
        @this.WorkArea = mi.WorkArea.ToRect();
    }

    public static void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true)
    {
        var devMode = new DevMode()
        {
            DeviceName = deviceName,
            Position = new WinDef.Point((int)area.X, (int)area.Y),
            PixelsWidth = (uint)area.Width,
            PixelsHeight = (uint)area.Height,
            DisplayOrientation = (DevMode.DisplayOrientationEnum)orientation,
            BitsPerPel = 32,
            Fields = DisplayModeFlags.Position |
                     DisplayModeFlags.PixelsHeight |
                     DisplayModeFlags.PixelsWidth |
                     DisplayModeFlags.DisplayOrientation |
                     DisplayModeFlags.BitsPerPixel
        };

        var flag =
            ChangeDisplaySettingsFlags.UpdateRegistry |
            ChangeDisplaySettingsFlags.NoReset;

        if (primary) flag |= ChangeDisplaySettingsFlags.SetPrimary;


        var ch = ChangeDisplaySettingsEx(deviceName, ref devMode, 0, flag, 0);

        if (ch == DispChange.Successful && apply)
            ApplyDesktop();
    }

    public static void DetachFromDesktop(string deviceName, bool apply = true)
    {
        var devMode = new DevMode
        {
            DeviceName = deviceName,
            PixelsHeight = 0,
            PixelsWidth = 0,
            Fields = DisplayModeFlags.PixelsWidth |
                     DisplayModeFlags.PixelsHeight |
                     // DisplayModeFlags.BitsPerPixel |
                     DisplayModeFlags.Position |
                     DisplayModeFlags.DisplayFrequency |
                     DisplayModeFlags.DisplayFlags
        };

        var ch = ChangeDisplaySettingsEx(
            deviceName,
            ref devMode,
            0,
            ChangeDisplaySettingsFlags.UpdateRegistry |
            ChangeDisplaySettingsFlags.NoReset,
            0);

        if (ch == DispChange.Successful && apply)
            ApplyDesktop();
    }


    public static void ApplyDesktop()
    {
        ChangeDisplaySettingsEx(null, 0, 0, 0, 0);
    }

    public static RegistryKey OpenMonitorRegKey(this MonitorDevice @this, RegistryKey key, bool create = false)
    {
        if (key == null) return null;
        return create ? key.CreateSubKey(@"monitors\" + @this.IdMonitor) : key.OpenSubKey(@"monitors\" + @this.IdMonitor);
    }

    public static void UpdateDpi(this MonitorDevice @this, nint hMonitor)
    {
        {
            var hResult = GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Effective, out var x, out var y);
            if (hResult != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
            }

            @this.EffectiveDpi = new Vector(x, y);
        }
        {
            if (GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Angular, out var x, out var y) != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
            }
            @this.AngularDpi = new Vector(x, y);
        }
        {
            if (GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Raw, out var x, out var y) != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
            }
            @this.RawDpi = new Vector(x, y);
        }
        {
            var factor = 100;
            if (GetScaleFactorForMonitor(hMonitor, ref factor) != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetScaleFactorForMonitor failed with error code: {0}", errorCode);
            }
            @this.ScaleFactor = (double)factor / 100.0;
        }

        @this.CapabilitiesString = Gdi32.DDCCIGetCapabilitiesString(hMonitor);
    }


    public static MonitorDevice BuildFromId(string id)
    {
        var edid = GetEdid(id);

        return new MonitorDevice
        {
            DeviceId = id,
            PnpCode = GetPnpCodeFromId(id),
            Edid = edid,
            IdMonitor = GetMicrosoftId(id, edid),
            //Devices = service
            //    .Devices
            //    .Connect()
            //    .Filter(e => e.DeviceId == id)
            //    .ObserveOn(RxApp.MainThreadScheduler)
            //    .AsObservableCache()

        };
    }

    static string GetPnpCodeFromId(string deviceId)
    {
        var id = deviceId.Split('\\');
        return id.Length > 1 ? id[1] : id[0];
    }


    static string GetMicrosoftId(string deviceId, Edid edid)
    {
        var pnpCode = GetPnpCodeFromId(deviceId);

        return edid is null
            ? GetPhysicalId(deviceId, null)
            : $"{GetPhysicalId(deviceId, edid)}_{edid.Checksum:X2}";
    }

    static string GetPhysicalId(string deviceId, Edid edid)
    {
        var pnpCode = GetPnpCodeFromId(deviceId);
        return edid == null
            ? $"NOEDID_{pnpCode}_{deviceId.Split('\\').Last()}"
            : $"{pnpCode}{edid.SerialNumber}_{edid.Week:X2}_{edid.Year:X4}";
    }

    static Edid GetEdid(string deviceId)
    {
        var devInfo = SetupApi.SetupDiGetClassDevsEx(
            ref SetupApi.GUID_CLASS_MONITOR, //class GUID
            null, //enumerator
            0, //HWND
            SetupApi.DIGCF_PRESENT | SetupApi.DIGCF_PROFILE, // Primary //DIGCF_ALLCLASSES|
            0, // device info, create a new one.
            null, // machine name, local machine
            0
        ); // reserved

        try
        {
            if (devInfo == 0)
            {
                return null;
            }

            var devInfoData = new SetupApi.SP_DEVINFO_DATA();

            uint i = 0;

            do
            {
                if (SetupApi.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {

                    var hEdidRegKey = SetupApi.SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                        SetupApi.DICS_FLAG_GLOBAL, 0, SetupApi.DIREG_DEV, SetupApi.KEY_READ);

                    try
                    {
                        if (hEdidRegKey != 0 && ((int)hEdidRegKey != -1))
                        {
                            using var key = WinReg.RegistryKey(hEdidRegKey, 1);
                            var value = key?.GetValue("HardwareID");
                            if (value is string[] { Length: > 0 } s)
                            {
                                var id = s[0] + "\\" +
                                         key.GetValue("Driver");

                                if (id == deviceId)
                                {
                                    var hKeyName = WinReg.GetHKeyName(hEdidRegKey);
                                    using var keyEdid = WinReg.RegistryKey(hEdidRegKey);

                                    var edid = (byte[])keyEdid.GetValue("EDID");
                                    return edid != null ? new Edid(hKeyName, edid) : null;
                                }
                            }
                        }
                    }
                    finally
                    {
                        var result = WinReg.RegCloseKey(hEdidRegKey);
                        if (result > 0)
                            throw new Exception(ErrHandlingApi.GetLastErrorString());
                    }
                }


                i++;
            } while (WinBase.ErrorNoMoreItems != ErrHandlingApi.GetLastError());
        }
        finally
        {
            SetupApi.SetupDiDestroyDeviceInfoList(devInfo);
        }

        return null;
    }

    public static void UpdateDevices(this MonitorsService service)
    {
        var oldDevices = service.Devices.ToList();
        var oldMonitors = service.Monitors.ToList();

        var root = new DisplayDevice("ROOT");

        root.Init(null, new WinGdi.DisplayDevice() { DeviceID = "ROOT", DeviceName = null }, oldDevices, oldMonitors, service);

        foreach (var d in oldDevices)
        {
            service.RemoveDevice(d.DeviceId);
        }

        foreach (var m in oldMonitors)
        {
            service.RemoveMonitor(m.DeviceId);
        }

        var hdc = 0;//GetDCEx(0, 0, DeviceContextValues.Window);

        // GetMonitorInfo
        WinUser.EnumDisplayMonitors(hdc, 0,
            (nint hMonitor, nint hdcMonitor, ref WinDef.Rect lprcMonitor, nint dwData) =>
            {
                var mi = new WinUser.MonitorInfoEx();//.Default;
                var success = WinUser.GetMonitorInfo(hMonitor, ref mi);
                if (!success) // Continue
                {
                    var errorCode = Marshal.GetLastWin32Error();
                    Debug.WriteLine("GetMonitorInfo failed with error code: {0}", errorCode);
                    return true;
                }

                var monitors = service.Monitors.Where(d => d.AttachedDisplay?.DeviceName == new string(mi.DeviceName)).ToList();
                foreach (var monitor in monitors)
                {
                    monitor.UpdateFromMonitorInfo(mi, monitors);
                    monitor.UpdateDpi(hMonitor);
                }

                return true; // Continue
            }, 0);

        //ReleaseDC(0, hdc);

        //ParseWindowsConfig();
        UpdateWallpaper(service);

        string FromUShort(IEnumerable<ushort> array)
        {
            var sb = new StringBuilder();
            foreach (var t in array)
            {
                sb.Append((char)(t));
            }
            return sb.ToString().Split('\0').First();
        }

        try
        {
            var aConnectionOptions = new ConnectionOptions();
            var aManagementScope = new ManagementScope(@"\\.\root\WMI", aConnectionOptions);
            var aObjectQuery = new ObjectQuery("SELECT * FROM WmiMonitorID");
            var aManagementObjectSearcher =
                new ManagementObjectSearcher(aManagementScope, aObjectQuery);
            var aManagementObjectCollection = aManagementObjectSearcher.Get();
            foreach (var aManagementObject in aManagementObjectCollection.OfType<ManagementObject>())
            {
                foreach (var property in aManagementObject.Properties)
                {
                    if (property.Value is ushort[] a)
                    {
                        Debug.Print($"{property.Name} = {FromUShort(a)}");
                    }
                    else
                    {
                        Debug.Print($"{property.Name} = {property.Value}");
                    }
                }

                //var DEVPKEY_Device_BiosDeviceName = aManagementObject["DEVPKEY_Device_BiosDeviceName"];
            }

        }
        catch
        {

        }

        //ConnectionOptions aConnectionOptions = new(); 
        //ManagementScope aManagementScope = new("\\\\.\\root\\WMI", aConnectionOptions);
        //ObjectQuery aObjectQuery = new("SELECT * FROM WmiMonitorID"); 
        //ManagementObjectSearcher aManagementObjectSearcher = new(aManagementScope, aObjectQuery);
        //ManagementObjectCollection aManagementObjectCollection = aManagementObjectSearcher.Get();
        //foreach ( ManagementObject aManagementObject in aManagementObjectCollection) 
        //{
        //    var InstanceName = aManagementObject["InstanceName"];
        //    var ManufacturerName = FromUShort((ushort[])aManagementObject["ManufacturerName"]); ;
        //    var ProductCodeID = FromUShort((ushort[])aManagementObject["ProductCodeID"]); ;
        //    var SerialNumberID = FromUShort((ushort[])aManagementObject["SerialNumberID"]); ;
        //    var UserFriendlyName = FromUShort((ushort[])aManagementObject["UserFriendlyName"]); ;

        //}


        //DevicesUpdated?.Invoke(this, EventArgs.Empty);
    }

    public static void UpdateWallpaper(this MonitorsService service)
    {
        var info = WindowsWallpaperHelper.ParseWallpapers(wp =>
        {
            var r = new Rect(wp.Rect.X, wp.Rect.Y, wp.Rect.Width, wp.Rect.Height);

            foreach (var monitor in service.Monitors.Where(m => m.MonitorArea == r))
            {
                monitor.MonitorNumber = (int)wp.Index + 1;
                monitor.WallpaperPath = wp.FilePath;
            }
        });

        service.WallpaperPosition = info.Position;
        service.Background = info.Background.ToColor();
    }

    public static void UpdateWallpaper2(this MonitorsService service)
    {
        string path, id;

        var todo = service.Monitors.ToList();
        using var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\\Desktop");

        // retrieve per monitor wallpaper if it exists
        if (key?.GetValue("TranscodedImageCount") is int nb)
        {
            for (var i = 0; i < nb; i++)
            {
                if (key.GetValue($"TranscodedImageCache_{i:000}") is not byte[] imgCacheN) continue;

                (path, id) = GetTranscodedImageCache(imgCacheN);

                var monitors = service.Monitors.Where(m => m.Edid.HKeyName.Contains(id)).ToList();

                if (!monitors.Any()) continue;

                foreach (var monitor in monitors)
                {
                    var mirrors = service.Monitors.Where(m => m.AttachedDisplay?.DeviceName == monitor.AttachedDisplay?.DeviceName);
                    foreach (var mirror in mirrors)
                    {
                        mirror.WallpaperPath = path;
                        if (todo.Contains(mirror)) todo.Remove(mirror);
                    }

                    monitor.MonitorNumber = i + 1;
                }
            }
        }

        if (!todo.Any()) return;

        // retrieve default wallpaper for other monitors
        if (key?.GetValue("TranscodedImageCache") is not byte[] imgCache) return;

        (path, id) = GetTranscodedImageCache(imgCache);

        foreach (var monitor in todo) monitor.WallpaperPath = path;
    }

    static (string path, string id) GetTranscodedImageCache(byte[] data)
    {
        // TODO understand what first 24 bytes stand for.
        //  0 -  3 : Unknown
        //  4 -  7 : File size
        //  8 - 11 : Width
        // 12 - 15 : Height
        // 16 - 19 : ?
        // 20 - 23 : ?
        var path = Encoding.Unicode.GetString(data[24..]).Split('\0').First();
        var id =
            string.Join('\\',
                Encoding.Unicode.GetString(data[544..])
                    .Split('\0')
                    .First()
                    .Replace(@"\\?\", "")
                    .Split('#').SkipLast(1));

        return (path, id);
    }

}