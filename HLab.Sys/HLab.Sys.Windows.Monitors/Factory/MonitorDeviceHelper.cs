using System;
using HLab.Sys.Monitors;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using HLab.Geo;
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

    //==============================================================================
    // Attach / detach by monitor identity (CCD API).
    //
    // ChangeDisplaySettingsEx addresses an adapter *source* (\\.\DISPLAYn). Once a
    // monitor is detached, Windows lists it as a binding candidate under EVERY
    // inactive source: with several detached monitors they all report the same
    // first source, and activating that source lets the driver bind whichever
    // candidate it prefers — attaching HEC0030 would light up SAME035 (#404, #391).
    // The CCD path array identifies the *target* (monitor) explicitly, so we
    // (de)activate the source->target path of that exact monitor instead.
    //==============================================================================

    static (DisplayConfigPathInfo[] Paths, DisplayConfigModeInfo[] Modes)? QueryDisplayConfigPaths()
    {
        uint nPath = 0, nMode = 0;
        if (GetDisplayConfigBufferSizes(QDC_ALL_PATHS, ref nPath, ref nMode) != 0) return null;

        var paths = new DisplayConfigPathInfo[nPath];
        var modes = new DisplayConfigModeInfo[nMode];
        if (QueryDisplayConfig(QDC_ALL_PATHS, ref nPath, paths, ref nMode, modes, 0) != 0) return null;

        // QueryDisplayConfig may return fewer elements than allocated
        if (nPath < paths.Length) Array.Resize(ref paths, (int)nPath);
        if (nMode < modes.Length) Array.Resize(ref modes, (int)nMode);

        return (paths, modes);
    }

    static string GetTargetDevicePath(Luid adapterId, uint targetId)
    {
        var name = new DisplayConfigTargetDeviceName
        {
            Type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            Size = (uint)Marshal.SizeOf<DisplayConfigTargetDeviceName>(),
            AdapterId = adapterId,
            Id = targetId,
        };
        return DisplayConfigGetDeviceInfo(ref name) == 0 ? name.MonitorDevicePath : "";
    }

    static string GetSourceGdiName(Luid adapterId, uint sourceId)
    {
        var name = new DisplayConfigSourceDeviceName
        {
            Type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            Size = (uint)Marshal.SizeOf<DisplayConfigSourceDeviceName>(),
            AdapterId = adapterId,
            Id = sourceId,
        };
        return DisplayConfigGetDeviceInfo(ref name) == 0 ? name.ViewGdiDeviceName : "";
    }

    static bool SourceInUse(DisplayConfigPathInfo[] paths, int idx)
    {
        var source = paths[idx].SourceInfo;
        for (var j = 0; j < paths.Length; j++)
        {
            if (j == idx) continue;
            if ((paths[j].Flags & DISPLAYCONFIG_PATH_ACTIVE) == 0) continue;
            if (paths[j].SourceInfo.AdapterId.Equals(source.AdapterId)
                && paths[j].SourceInfo.Id == source.Id) return true;
        }
        return false;
    }

    /// <summary>
    /// Attach the monitor designated by its device interface path (\\?\DISPLAY#...)
    /// to the desktop, then apply the requested position/resolution/orientation on
    /// the source it got bound to.
    /// </summary>
    public static bool AttachToDesktop(string monitorDevicePath, bool primary, Rect area, int orientation, bool apply = true)
    {
        if (string.IsNullOrEmpty(monitorDevicePath)) return false;
        if (QueryDisplayConfigPaths() is not { } cfg) return false;
        var (paths, modes) = cfg;

        var pathIdx = -1;
        for (var i = 0; i < paths.Length; i++)
        {
            if (!paths[i].TargetInfo.TargetAvailable) continue;
            if (!string.Equals(
                    GetTargetDevicePath(paths[i].TargetInfo.AdapterId, paths[i].TargetInfo.Id),
                    monitorDevicePath, StringComparison.OrdinalIgnoreCase)) continue;

            // already active : no need to change the topology, just apply the mode
            if ((paths[i].Flags & DISPLAYCONFIG_PATH_ACTIVE) != 0)
            {
                pathIdx = i;
                break;
            }

            if (SourceInUse(paths, i)) continue;

            pathIdx = i;
            break;
        }

        if (pathIdx < 0)
        {
            Debug.WriteLine($"AttachToDesktop: no available path for {monitorDevicePath}");
            return false;
        }

        if ((paths[pathIdx].Flags & DISPLAYCONFIG_PATH_ACTIVE) == 0)
        {
            paths[pathIdx].Flags |= DISPLAYCONFIG_PATH_ACTIVE;
            paths[pathIdx].SourceInfo.ModeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
            paths[pathIdx].TargetInfo.ModeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;

            var r = SetDisplayConfig((uint)paths.Length, paths, (uint)modes.Length, modes,
                SetDisplayConfigFlags.Apply
                | SetDisplayConfigFlags.UseSuppliedDisplayConfig
                | SetDisplayConfigFlags.AllowChanges
                | SetDisplayConfigFlags.SaveToDatabase);

            if (r != 0)
            {
                Debug.WriteLine($"AttachToDesktop: SetDisplayConfig failed ({r}) for {monitorDevicePath}");
                return false;
            }
        }

        // The monitor is now bound to a known source : place it at the wanted
        // position/mode (this part is the former ChangeDisplaySettingsEx attach).
        var sourceName = GetSourceGdiName(paths[pathIdx].SourceInfo.AdapterId, paths[pathIdx].SourceInfo.Id);
        if (string.IsNullOrEmpty(sourceName)) return false;

        var devMode = new DevMode()
        {
            DeviceName = sourceName,
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

        var ch = ChangeDisplaySettingsEx(sourceName, ref devMode, 0, flag, 0);

        if (ch == DispChange.Successful && apply)
            ApplyDesktop();

        return ch == DispChange.Successful;
    }

    /// <summary>
    /// Detach the monitor designated by its device interface path (\\?\DISPLAY#...)
    /// from the desktop, by deactivating its active CCD path.
    /// </summary>
    public static bool DetachFromDesktop(string monitorDevicePath)
    {
        if (string.IsNullOrEmpty(monitorDevicePath)) return false;
        if (QueryDisplayConfigPaths() is not { } cfg) return false;
        var (paths, modes) = cfg;

        var found = false;
        for (var i = 0; i < paths.Length; i++)
        {
            if ((paths[i].Flags & DISPLAYCONFIG_PATH_ACTIVE) == 0) continue;
            if (!string.Equals(
                    GetTargetDevicePath(paths[i].TargetInfo.AdapterId, paths[i].TargetInfo.Id),
                    monitorDevicePath, StringComparison.OrdinalIgnoreCase)) continue;

            paths[i].Flags &= ~DISPLAYCONFIG_PATH_ACTIVE;
            found = true;
            break;
        }

        if (!found)
        {
            Debug.WriteLine($"DetachFromDesktop: no active path for {monitorDevicePath}");
            return false;
        }

        var r = SetDisplayConfig((uint)paths.Length, paths, (uint)modes.Length, modes,
            SetDisplayConfigFlags.Apply
            | SetDisplayConfigFlags.UseSuppliedDisplayConfig
            | SetDisplayConfigFlags.AllowChanges
            | SetDisplayConfigFlags.SaveToDatabase);

        Debug.WriteLine($"DetachFromDesktop {monitorDevicePath} {r}");

        return r == 0;
    }

    /// <summary>
    /// Make the monitor designated by its device interface path (\\?\DISPLAY#...)
    /// the primary display. Windows defines the primary as the display at (0,0):
    /// every attached display is shifted accordingly, pending (NoReset), then the
    /// whole layout is applied at once so intermediate overlaps never exist.
    /// </summary>
    public static bool SetPrimary(string monitorDevicePath)
    {
        if (string.IsNullOrEmpty(monitorDevicePath)) return false;
        if (QueryDisplayConfigPaths() is not { } cfg) return false;
        var (paths, _) = cfg;

        string sourceName = null;
        for (var i = 0; i < paths.Length; i++)
        {
            if ((paths[i].Flags & DISPLAYCONFIG_PATH_ACTIVE) == 0) continue;
            if (!string.Equals(
                    GetTargetDevicePath(paths[i].TargetInfo.AdapterId, paths[i].TargetInfo.Id),
                    monitorDevicePath, StringComparison.OrdinalIgnoreCase)) continue;

            sourceName = GetSourceGdiName(paths[i].SourceInfo.AdapterId, paths[i].SourceInfo.Id);
            break;
        }

        if (string.IsNullOrEmpty(sourceName))
        {
            Debug.WriteLine($"SetPrimary: no active path for {monitorDevicePath}");
            return false;
        }

        var mode = GetCurrentMode(sourceName);
        if (mode == null) return false;

        var offset = mode.Position;
        if (offset.X == 0 && offset.Y == 0) return true; // already at origin, hence primary

        // The new primary must be written FIRST, at (0,0) with SetPrimary: once
        // another display has already been moved pending, the SetPrimary write
        // gets refused by the driver (verified on AMD).
        var ok = SetModePending(sourceName, mode, offset, true);

        if (ok)
        {
            var device = new WinGdi.DisplayDevice();
            uint n = 0;
            while (EnumDisplayDevices(null, n++, ref device, 0))
            {
                if ((device.StateFlags & DisplayDeviceStateFlags.AttachedToDesktop) == 0) continue;
                if (device.DeviceName == sourceName) continue;

                var current = GetCurrentMode(device.DeviceName);
                if (current == null) continue;

                if (SetModePending(device.DeviceName, current, offset, false)) continue;

                ok = false;
                break;
            }
        }

        if (!ok)
        {
            ResaveCurrentConfiguration();
            return false;
        }

        ApplyDesktop();
        return true;
    }

    /// <summary>
    /// Write a display mode to the registry (NoReset), shifted by the given offset.
    /// The full current mode is carried along with the new position: a position-only
    /// DevMode gets refused by some drivers, and an aborted pending write leaves the
    /// registry configuration inconsistent, failing every further UpdateRegistry call.
    /// </summary>
    static bool SetModePending(string deviceName, DisplayMode current, Point offset, bool primary)
    {
        var devMode = new DevMode
        {
            Position = new WinDef.Point(
                (int)(current.Position.X - offset.X),
                (int)(current.Position.Y - offset.Y)),
            PixelsWidth = (uint)current.Pels.Width,
            PixelsHeight = (uint)current.Pels.Height,
            DisplayFrequency = (uint)current.DisplayFrequency,
            BitsPerPel = 32,
            Fields = Position |
                     PixelsWidth |
                     PixelsHeight |
                     DisplayFrequency |
                     BitsPerPixel
        };

        var flag = ChangeDisplaySettingsFlags.UpdateRegistry | ChangeDisplaySettingsFlags.NoReset;
        if (primary) flag |= ChangeDisplaySettingsFlags.SetPrimary;

        var ch = ChangeDisplaySettingsEx(deviceName, ref devMode, 0, flag, 0);
        if (ch == DispChange.Successful) return true;

        Debug.WriteLine($"SetModePending failed on {deviceName} ({ch})");
        return false;
    }

    /// <summary>
    /// Re-apply and re-save the current active configuration as-is. Rewrites a
    /// coherent registry state when pending display writes were aborted halfway,
    /// which would otherwise fail every further UpdateRegistry call.
    /// </summary>
    static void ResaveCurrentConfiguration()
    {
        if (QueryDisplayConfigPaths() is not { } cfg) return;
        var (paths, modes) = cfg;

        SetDisplayConfig((uint)paths.Length, paths, (uint)modes.Length, modes,
            SetDisplayConfigFlags.Apply
            | SetDisplayConfigFlags.UseSuppliedDisplayConfig
            | SetDisplayConfigFlags.AllowChanges
            | SetDisplayConfigFlags.SaveToDatabase);
    }

    // Former source-based implementation, kept for reference. It detached whatever
    // monitor was bound to the given \\.\DISPLAYn source by applying an empty mode:
    //
    //public static bool DetachFromDesktop(string deviceName, bool apply = true)
    //{
    //    var devMode = new DevMode
    //    {
    //        Fields = 0
    //         | PixelsWidth
    //         | PixelsHeight
    //         | BitsPerPixel
    //         | Position
    //         | DisplayFrequency
    //         | DisplayFlags
    //    };
    //
    //    devMode.BitsPerPel = 32;
    //
    //    var ch = ChangeDisplaySettingsEx(
    //        deviceName,
    //        ref devMode,
    //        0,
    //        ChangeDisplaySettingsFlags.UpdateRegistry
    //        | ChangeDisplaySettingsFlags.NoReset
    //        , 0);
    //
    //    Debug.WriteLine($"DetachFromDesktop {deviceName} {ch}");
    //
    //    if (ch == DispChange.Successful && apply)
    //    {
    //        ApplyDesktop();
    //        return true;
    //    }
    //
    //    return false;
    //}


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

    public static Edid? GetEdid(string deviceId)
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
                                    return edid != null ? EdidParser.Parse(hKeyName, edid) : null;
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

    /// <summary>
    /// Cheap fingerprint of the current display configuration — the attached monitor set with
    /// each one's virtual-screen rectangle, effective DPI and primary flag — read directly via
    /// GetMonitorInfo/GetDpiForMonitor, WITHOUT the full device-tree build of
    /// <see cref="GetDisplayDevices"/>. Used to coalesce the WM_DISPLAYCHANGE burst and to detect
    /// when the OS has finished reconfiguring (two identical consecutive reads ⇒ settled), so the
    /// layout rebuild no longer relies on a fixed delay.
    /// </summary>
    public static string DisplaySignature()
    {
        var parts = new List<string>();
        EnumDisplayMonitors(0, 0,
            (nint hMonitor, nint hdcMonitor, ref WinDef.Rect lprcMonitor, nint dwData) =>
            {
                var mi = new MonitorInfoEx();
                if (!GetMonitorInfo(hMonitor, ref mi)) return true;
                var r = mi.Monitor.ToRect();
                var dpi = ShellScalingApi.GetDpiForMonitor(hMonitor, ShellScalingApi.DpiType.Effective, out var dx, out _) == 0 ? dx : 0u;
                parts.Add($"{new string(mi.DeviceName).TrimEnd('\0')}[{r.X},{r.Y} {r.Width}x{r.Height}]{(mi.Flags == 1 ? "*" : "")}d{dpi}");
                return true;
            }, 0);
        parts.Sort();
        return string.Join("|", parts);
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

        UpdateMonitorNumbers(list);
        UpdateSpecializedMonitors(list);

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


    static bool IsSpecializedTarget(Luid adapterId, uint targetId)
    {
        var request = new DisplayConfigGetMonitorSpecialization
        {
            Type = DISPLAYCONFIG_DEVICE_INFO_GET_MONITOR_SPECIALIZATION,
            Size = (uint)Marshal.SizeOf<DisplayConfigGetMonitorSpecialization>(),
            AdapterId = adapterId,
            Id = targetId,
        };
        // the query is unsupported before Windows 11: treat failures as not specialized
        return DisplayConfigGetDeviceInfo(ref request) == 0 && request.IsSpecializationEnabled;
    }

    /// <summary>
    /// Flag monitors bound to a specialized target (VR headsets like Windows Mixed
    /// Reality): they are driven by a dedicated runtime and hidden from the desktop
    /// by Windows, so they must not take part in the mouse layout (#364).
    /// </summary>
    static void UpdateSpecializedMonitors(IEnumerable<MonitorDevice> monitors)
    {
        if (QueryDisplayConfigPaths() is not { } cfg) return;

        foreach (var path in cfg.Paths)
        {
            if (!path.TargetInfo.TargetAvailable) continue;
            if (!IsSpecializedTarget(path.TargetInfo.AdapterId, path.TargetInfo.Id)) continue;

            var devicePath = GetTargetDevicePath(path.TargetInfo.AdapterId, path.TargetInfo.Id);
            if (string.IsNullOrEmpty(devicePath)) continue;

            foreach (var monitor in monitors)
            {
                if (monitor.InterfacePath != devicePath) continue;

                // Windows removes specialized displays from desktop composition, so a
                // monitor EnumDisplayMonitors still accounts for (its adapter carries
                // an HMONITOR, set earlier in GetDisplayDevices) is part of the desktop
                // no matter what the query said. Some systems report specialization
                // enabled on regular active monitors, which hid every real display and
                // left only the detached virtual one (#506) — believe the desktop, not
                // the query. Hidden displays like the WMR headset of #364 have no
                // HMONITOR and still get flagged.
                if (monitor.Connections.Any(c => c.Parent.HMonitor != 0)) continue;

                monitor.IsSpecialized = true;
            }
        }
    }

    /// <summary>
    /// Assign each monitor the number Windows shows in Settings > System > Display.
    /// That number is the rank of the monitor's CCD target (its physical GPU
    /// connector) among all connected targets, sorted by target id. It ignores
    /// desktop position, attach state (a detached monitor keeps its number),
    /// attach order and primary — unlike the GDI source number (\\.\DISPLAYn)
    /// which follows attach history.
    /// Adapters are ordered by LUID: verified with a dGPU + iGPU rig where the
    /// iGPU monitor came first in the path array (that order is not stable
    /// across queries), had the lowest target id, and was primary — Settings
    /// still numbered it last, after the lower-LUID dGPU targets.
    /// </summary>
    static void UpdateMonitorNumbers(IEnumerable<MonitorDevice> monitors)
    {
        var remaining = monitors.ToList();
        var number = 1;

        if (QueryDisplayConfigPaths() is { } cfg)
        {
            var targets = new HashSet<(Luid Adapter, uint Id)>();

            foreach (var path in cfg.Paths)
            {
                if (!path.TargetInfo.TargetAvailable) continue;

                targets.Add((path.TargetInfo.AdapterId, path.TargetInfo.Id));
            }

            var adapters = targets
                .Select(t => t.Adapter)
                .Distinct()
                .OrderBy(a => a.HighPart).ThenBy(a => a.LowPart);

            foreach (var adapter in adapters)
            {
                foreach (var target in targets.Where(t => t.Adapter.Equals(adapter)).OrderBy(t => t.Id))
                {
                    // the number is consumed even when no device matches: Settings
                    // numbers every connected target
                    var n = number++;

                    var devicePath = GetTargetDevicePath(target.Adapter, target.Id);
                    if (string.IsNullOrEmpty(devicePath)) continue;

                    var monitor = remaining.FirstOrDefault(m => m.InterfacePath == devicePath);
                    if (monitor == null) continue;

                    monitor.MonitorNumber = $"{n}";
                    remaining.Remove(monitor);
                }
            }
        }

        // monitors CCD could not account for (virtual or remote sessions)
        foreach (var monitor in remaining)
        {
            monitor.MonitorNumber = $"{number++}";
        }
    }

}