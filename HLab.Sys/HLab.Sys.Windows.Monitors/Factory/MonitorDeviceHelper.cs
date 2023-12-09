using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using DynamicData;
using Avalonia;
using Microsoft.Win32;
using HLab.Sys.Windows.API;

using static HLab.Sys.Windows.API.ErrHandlingApi;
using static HLab.Sys.Windows.API.SetupApi;
using static HLab.Sys.Windows.API.WinBase;
using static HLab.Sys.Windows.API.WinGdi;
using static HLab.Sys.Windows.API.WinGdi.DisplayModeFlags;
using static HLab.Sys.Windows.API.WinReg;
using static HLab.Sys.Windows.API.WinUser;

namespace HLab.Sys.Windows.Monitors.Factory;

public static class MonitorDeviceHelper
{
    public static DisplayMode GetDisplayMode(DevMode dm)
    {
        return new DisplayMode
        {
            DisplayOrientation = (int)((dm.Fields & DisplayOrientation) != 0
                ? dm.DisplayOrientation : DevMode.DisplayOrientationEnum.Default),

            Position = (dm.Fields & Position) != 0
                ? new Point(dm.Position.X, dm.Position.Y) : new Point(0, 0),

            BitsPerPixel = (dm.Fields & BitsPerPixel) != 0
                ? dm.BitsPerPel : 0,

            Pels = (dm.Fields & (PixelsWidth | PixelsHeight)) != 0
                ? new Size(dm.PixelsWidth, dm.PixelsHeight) : new Size(1, 1),

            DisplayFlags = (int)((dm.Fields & DisplayFlags) != 0
                ? dm.DisplayFlags : 0),

            DisplayFrequency = (int)((dm.Fields & DisplayFrequency) != 0
                ? dm.DisplayFrequency : 0),

            DisplayFixedOutput = (int)((dm.Fields & DisplayFixedOutput) != 0
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

    public static DeviceState BuildDeviceState(DisplayDeviceStateFlags state)
    {
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

    public static DeviceCaps BuildDeviceCaps(string deviceName)
    {
        var hdc = CreateDC("DISPLAY", deviceName, null, 0);
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

    public static void UpdateFromMonitorInfo(this PhysicalAdapter @this, MonitorInfoEx mi)
    {
        @this.Primary = mi.Flags == 1;
        @this.MonitorArea = mi.Monitor.ToRect();
        @this.WorkArea = mi.WorkArea.ToRect();
    }

    public static void UpdateFromMonitorInfo(this PhysicalAdapter @this, MonitorInfo mi)
    {
        @this.Primary = mi.Flags == 1;
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
            Fields = Position |
                     PixelsHeight |
                     PixelsWidth |
                     DisplayOrientation |
                     BitsPerPixel
        };

        var flag =
            ChangeDisplaySettingsFlags.UpdateRegistry |
            ChangeDisplaySettingsFlags.NoReset;

        if (primary) flag |= ChangeDisplaySettingsFlags.SetPrimary;


        var ch = ChangeDisplaySettingsEx(deviceName, ref devMode, 0, flag, 0);

        if (ch == DispChange.Successful && apply)
            ApplyDesktop();
    }

    public static bool DetachFromDesktop(string deviceName, bool apply = true)
    {

        var devMode = new DevMode
        {
            //DeviceName = "cdd", //= deviceName,
            //PixelsHeight = 0,
            //PixelsWidth = 0,
            Fields = 0
             | PixelsWidth
             | PixelsHeight
             | BitsPerPixel
             | Position
             | DisplayFrequency
             | DisplayFlags
        };

        //devMode.DeviceName = deviceName;
        //devMode.PixelsHeight = 0;
        //devMode.PixelsWidth = 0;

        devMode.BitsPerPel = 32;

        var ch = ChangeDisplaySettingsEx(
            deviceName,
            ref devMode,
            0,
            ChangeDisplaySettingsFlags.UpdateRegistry
            | ChangeDisplaySettingsFlags.NoReset
            , 0);

        Debug.WriteLine($"DetachFromDesktop {deviceName} {ch}");

        if (ch == DispChange.Successful && apply)
        {
            ApplyDesktop();
            return true;
        }

        return false;
    }


    public static void ApplyDesktop() => ChangeDisplaySettingsEx(null, 0, 0, 0, 0);

    public static RegistryKey OpenMonitorRegKey(this MonitorDevice @this, RegistryKey key, bool create = false)
    {
        if (key == null) return null;
        return create ? key.CreateSubKey(@"monitors\" + @this.PhysicalId) : key.OpenSubKey(@"monitors\" + @this.PhysicalId);
    }

    public static void UpdateDpi(this PhysicalAdapter @this, nint hMonitor)
    {
        {
            var hResult = ShellScalingApi.GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Effective, out var x, out var y);
            if (hResult != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
            }

            @this.EffectiveDpi = new Vector(x, y);
        }
        {
            if (ShellScalingApi.GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Angular, out var x, out var y) != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
            }
            @this.AngularDpi = new Vector(x, y);
        }
        {
            if (ShellScalingApi.GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Raw, out var x, out var y) != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
            }
            @this.RawDpi = new Vector(x, y);
        }
        {
            var factor = 100;
            if (ShellScalingApi.GetScaleFactorForMonitor(hMonitor, ref factor) != 0)
            {
                var errorCode = Marshal.GetLastWin32Error();
                Debug.WriteLine("GetScaleFactorForMonitor failed with error code: {0}", errorCode);
            }
            @this.ScaleFactor = factor / 100.0;
        }

        @this.CapabilitiesString = Gdi32.DDCCIGetCapabilitiesString(hMonitor);
    }


    public static string GetPnpCodeFromId(string deviceId)
    {
        var id = deviceId.Split('\\');
        return id.Length > 1 ? id[1] : id[0];
    }

    public static string GetPhysicalId(string deviceId, Edid edid)
    {
        var pnpCode = GetPnpCodeFromId(deviceId);
        return edid == null
            ? $"NOEDID_{pnpCode}_{deviceId.Split('\\').Last()}"
            : $"{pnpCode}{edid.SerialNumber}_{edid.Week:X2}_{edid.Year:X4}";
    }


    public static string GetSourceId(string deviceId, Edid edid)
    {
        var pnpCode = GetPnpCodeFromId(deviceId);
        return edid == null
            ? $"NOEDID_{pnpCode}_{deviceId.Split('\\').Last()}"
            : $"{pnpCode}{edid.SerialNumber}_{edid.Week:X2}_{edid.Year:X4}_{edid.Checksum:X2}";
    }

    public static Edid GetEdid(string deviceId)
    {
        var devInfo = SetupDiGetClassDevsEx(
            ref GUID_CLASS_MONITOR, //class GUID
            null, //enumerator
            0, //HWND
            DIGCF_PRESENT | DIGCF_PROFILE, // Primary //DIGCF_ALLCLASSES|
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

            var devInfoData = new SP_DEVINFO_DATA();

            uint i = 0;

            do
            {
                if (SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {

                    var hEdidRegKey = SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                        DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ);

                    try
                    {
                        if (hEdidRegKey != 0 && (int)hEdidRegKey != -1)
                        {
                            using var key = RegistryKey(hEdidRegKey, 1);
                            var value = key?.GetValue("HardwareID");
                            if (value is string[] { Length: > 0 } s)
                            {
                                var id = s[0] + "\\" +
                                         key.GetValue("Driver");

                                if (id == deviceId)
                                {
                                    var hKeyName = GetHKeyName(hEdidRegKey);
                                    using var keyEdid = RegistryKey(hEdidRegKey);

                                    var edid = (byte[])keyEdid.GetValue("EDID");
                                    return edid != null ? new Edid(hKeyName, edid) : null;
                                }
                            }
                        }
                    }
                    finally
                    {
                        var result = RegCloseKey(hEdidRegKey);
                        if (result > 0)
                            throw new Exception(GetLastErrorString());
                    }
                }


                i++;
            } while (ErrorNoMoreItems != GetLastError());
        }
        finally
        {
            SetupDiDestroyDeviceInfoList(devInfo);
        }

        return null;
    }

    public static DisplayDevice GetDisplayDevices()
    {
        var root = DeviceFactory.BuildDisplayDeviceAndChildren(null, new WinGdi.DisplayDevice()
        { DeviceID = "ROOT", DeviceName = null });

        var hdc = 0;//GetDCEx(0, 0, DeviceContextValues.Window);

        var monitors = root.AllChildren<PhysicalAdapter>().ToList();


        // GetMonitorInfo
        EnumDisplayMonitors(hdc, 0,
            (nint hMonitor, nint hdcMonitor, ref WinDef.Rect lprcMonitor, nint dwData) =>
            {
                var mi = new MonitorInfoEx();//.Default;
                var success = GetMonitorInfo(hMonitor, ref mi);
                if (!success) // Continue
                {
                    var errorCode = Marshal.GetLastWin32Error();
                    Debug.WriteLine("GetMonitorInfo failed with error code: {0}", errorCode);
                    return true;
                }

                var matches = monitors.Where(d => d.DeviceName == new string(mi.DeviceName)).ToList();
                foreach (var monitor in matches)
                {
                    monitor.HMonitor = hMonitor;
                    monitor.UpdateFromMonitorInfo(mi);
                    monitor.UpdateDpi(hMonitor);
                }

                return true; // Continue
            }, 0);

        //ReleaseDC(0, hdc);

        root.UpdateWallpaper();


        var list = root.AllMonitorDevices().ToList();

        // Some monitors may have generic serial so we fallback to DeviceId to descriminiate them
        // Reason wy we don't use DeviceId as primary key is that it may change when monitors are plugged/unplugged

        var lastSourceId = "";
        MonitorDevice last = null;
        foreach (var monitor in list)
        {
            if (last != null && monitor.SourceId == lastSourceId)
            {
                if (last.SourceId == lastSourceId)
                    last.SourceId = $"{lastSourceId}_{last.Id.Split('\\').Last()}";

                if (monitor.SourceId == lastSourceId)
                    monitor.SourceId = $"{lastSourceId}_{monitor.Id.Split('\\').Last()}";
            }
            else
            {
                lastSourceId = monitor.SourceId;
            }

            last = monitor;
        }

        ParseWindowsConfig(list);

        return root;
    }

    public static void UpdateWallpaper(this DisplayDevice root)
    {
        var adapters = root.AllChildren<PhysicalAdapter>().ToList();

        var info = WindowsWallpaperHelper.ParseWallpapers(wp =>
        {
            var r = new Rect(wp.Rect.X, wp.Rect.Y, wp.Rect.Width, wp.Rect.Height);

            var adapter = adapters.FirstOrDefault(m => m.MonitorArea == r);
            if (adapter == null) return;

            adapter.WallpaperPath = wp.FilePath;
            adapter.WallpaperPosition = wp.Position;
            adapter.Background = wp.Background;
            adapters.Remove(adapter);
        });

        foreach (var adapter in adapters)
        {
            adapter.WallpaperPath = null;
        }
    }

    public static void UpdateWallpaper2(this DisplayDevice root)
    {
        string path, id;

        var todo = root.AllChildren<PhysicalAdapter>().ToList();
        using var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\\Desktop");

        // retrieve per monitor wallpaper if it exists
        if (key?.GetValue("TranscodedImageCount") is int nb)
        {
            for (var i = 0; i < nb; i++)
            {
                if (key.GetValue($"TranscodedImageCache_{i:000}") is not byte[] imgCacheN) continue;

                (path, id) = GetTranscodedImageCache(imgCacheN);

                var monitors = root.AllChildren<MonitorDeviceConnection>().Where(m => m.Monitor.Edid.HKeyName.Contains(id)).ToList();

                if (!monitors.Any()) continue;

                foreach (var monitor in monitors)
                {
                    var mirrors = root.AllChildren<PhysicalAdapter>().Where(m => m.DeviceName == monitor.Parent.DeviceName);
                    foreach (var mirror in mirrors)
                    {
                        mirror.WallpaperPath = path;
                        if (todo.Contains(mirror)) todo.Remove(mirror);
                    }

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


    static bool MatchConfig(string setId, IEnumerable<MonitorDevice> monitors)
    {
        var remaining = monitors.ToList();

        var ids = setId.Split('^');
        foreach (var id in ids)
        {
            var result = monitors.Where(m => m.SourceId == id).ToList();

            if (result.Count == 0) return false;

            foreach (var monitor in result)
            {
                remaining.Remove(monitor);
            }
        }
        return !remaining.Any();
    }

    static RegistryKey GetConnectivityKey(IEnumerable<MonitorDevice> monitors)
    {
#pragma warning disable CA1416 // Valider la compatibilité de la plateforme
        using var key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Control\GraphicsDrivers\Connectivity");
        if (key == null) return null;

        foreach (var configurationKeyName in key.GetSubKeyNames())
        {
            var configurationKey = key.OpenSubKey(configurationKeyName);
            if (configurationKey?.GetValue("SetId") is string setId && MatchConfig(setId.Trim('\0'), monitors))
            {
                return configurationKey;
            }
        }
        return null;
    }

    static bool ParseSetId(RegistryKey key, string keyName, List<MonitorDevice> remaining, ref int number)
    {
        switch (remaining.Count)
        {
            case 0:
                return true;
            case 1:
                remaining[0].MonitorNumber = $"{number++}";
                return true;
        }

        var setId = key.GetValue(keyName) as string;
        if (!string.IsNullOrEmpty(setId))
        {
            setId = setId.Trim('\0');
            //displays are separated by +, clones are separated by *
            foreach (var displayId in setId.Split('+'))
            {
                // all monitors in the same clone group have the same number
                foreach (var monitorId in displayId.Split('*'))
                {
                    var monitors = remaining.Where(m => m.SourceId == monitorId).ToList();
                    foreach (var monitor in monitors)
                    {
                        monitor.MonitorNumber = $"{number++}";
                        remaining.Remove(monitor);
                    }
                }
            }
        }
        switch (remaining.Count)
        {
            case 0:
                return true;
            case 1:
                remaining[0].MonitorNumber = $"{number++}";
                return true;
            default:
                return false;
        }
    }

    /// <summary>
    /// Try to gess monitor number from windows configuration
    /// </summary>
    /// <param name="monitors"></param>
    /// <returns></returns>
    static bool ParseWindowsConfig(IEnumerable<MonitorDevice> monitors)
    {
        var number = 1;

        var remaining = monitors.ToList();

        using var key = GetConnectivityKey(remaining);
        if (key is not null)
        {
            if (ParseSetId(key, "Internal", remaining, ref number)) return true;
            if (ParseSetId(key, "External", remaining, ref number)) return true;
            if (ParseSetId(key, "eXtend", remaining, ref number)) return true;
            if (ParseSetId(key, "Clone", remaining, ref number)) return true;
            if (ParseSetId(key, "Recent", remaining, ref number)) return true;
        }

        foreach (var monitor in remaining)
        {
            monitor.MonitorNumber = $"{number++}";
        }

        return false;
    }

}