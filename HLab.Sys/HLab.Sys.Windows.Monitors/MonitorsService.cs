/*
  HLab.Windows.Monitors
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.Monitors.

    HLab.Windows.Monitors is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.Monitors is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Management;
using System.Reactive.Linq;
using System.Runtime.InteropServices;
using System.Text;
using Avalonia.Threading;
using DynamicData;
using HLab.Sys.Windows.API;
using Microsoft.Win32;
using ReactiveUI;
using static HLab.Sys.Windows.API.WinGdi;
using static HLab.Sys.Windows.API.WinUser;
using Rect = Avalonia.Rect;

namespace HLab.Sys.Windows.Monitors
{
 
    public interface IMonitorsService
    {
        IObservableCache<MonitorDevice, string> Monitors { get; }
        IObservableCache<DisplayDevice, string> Devices { get; }
        IObservableCache<MonitorDevice, string> AttachedMonitors { get; }

        void DetachFromDesktop(string deviceName, bool apply = true);
        void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true);
        void ApplyDesktop();
        void UpdateDevices();

        RegistryKey OpenRootRegKey(bool create = false);
        string AppDataPath(bool create);
    }

    public class MonitorsService : ReactiveObject, IMonitorsService
    {
        const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

        public event EventHandler DevicesUpdated;

        readonly DisplayChangeMonitor _listener = new DisplayChangeMonitor();

        public MonitorsService()
        {
            AttachedMonitors = Monitors
                .Connect().AutoRefresh(m => m.AttachedToDesktop)
                .Filter(m => m.AttachedToDesktop)
                .ObserveOn(RxApp.MainThreadScheduler).AsObservableCache();

            UnattachedMonitors = Monitors
                .Connect().AutoRefresh(m => m.AttachedToDesktop)
                .Filter(m => !m.AttachedToDesktop)
                .ObserveOn(RxApp.MainThreadScheduler).AsObservableCache();//B

            UpdateDevices();

            _listener.DisplayChanged += (o, a) => { UpdateDevices(); };

        }

        readonly SourceCache<PhysicalAdapter, string> _adapters = new (m => m.DeviceId);
        public IObservableCache<PhysicalAdapter,string> Adapters => _adapters;

        readonly SourceCache<DisplayDevice, string> _devices = new (m => m.DeviceName);
        public IObservableCache<DisplayDevice,string> Devices => _devices;

        readonly SourceCache<MonitorDevice, string> _monitors = new (m => m.DeviceId);
        public IObservableCache<MonitorDevice,string> Monitors => _monitors;


        public IObservableCache<MonitorDevice,string> AttachedMonitors { get; }


        //IObservableCache<MonitorDevice, string> _attachedMonitors;
        //public IObservableCache<MonitorDevice,string> AttachedMonitors => _attachedMonitors;    

        public IObservableCache<MonitorDevice,string> UnattachedMonitors { get; }

        public DisplayDevice GetOrAddDevice(string deviceId, Func<IMonitorsService,string,DisplayDevice> get) =>
            _devices.GetOrAdd(this, deviceId, get);

        public MonitorDevice GetOrAddMonitor(string deviceId, Func<IMonitorsService, string, MonitorDevice> get) =>
            _monitors.GetOrAdd(this, deviceId, get);

        public PhysicalAdapter GetOrAddAdapter(string deviceId, Func<IMonitorsService, string, PhysicalAdapter> get) =>
            _adapters.GetOrAdd(this, deviceId, get);


        public void UpdateDevices()
        {
            var oldDevices = Devices.Items.ToList();
            var oldMonitors = Monitors.Items.ToList();

            var root = new DisplayDevice(this,"ROOT");

            root.Init(null,new WinGdi.DisplayDevice(){DeviceID = "ROOT",DeviceName = null}, oldDevices, oldMonitors);

            foreach (var d in oldDevices)
            {
                _devices.Remove(d);
            }

            foreach (var m in oldMonitors)
            {
                _monitors.Remove(m);
            }

            var hdc = 0;//GetDCEx(0, 0, DeviceContextValues.Window);

            // GetMonitorInfo
            EnumDisplayMonitors(hdc, 0,
                (nint hMonitor, nint hdcMonitor,ref WinDef.Rect lprcMonitor, nint dwData)=>
                {
                    var mi = new MonitorInfoEx();//.Default;
                    mi.Init();
                    var success = GetMonitorInfo(hMonitor, ref mi);
                    if (!success) // Continue
                    {
                        var errorCode = Marshal.GetLastWin32Error();
                        Debug.WriteLine("GetMonitorInfo failed with error code: {0}", errorCode);
                        return true;
                    }
                    
                    var monitors = AttachedMonitors.Items.Where(d => d.AttachedDisplay?.DeviceName == new string(mi.DeviceName)).ToList();
                    foreach (var monitor in monitors)
                    {
                        monitor.MonitorNo = (int)dwData;
                        monitor.SetMonitorInfo(mi);
                        monitor.UpdateDpi(hMonitor);
                    }

                    return true; // Continue
                }, 0);

            //ReleaseDC(0, hdc);

            //ParseWindowsConfig();
            UpdateWallpaper();

            string FromUShort(IEnumerable<ushort> array)
            {
                var sb = new StringBuilder();
                foreach (var t in array)
                {
                    sb.Append( (char)(t) );
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


            DevicesUpdated?.Invoke(this, new EventArgs());
        }

        static (string path, string id) GetTranscodedImageCache(byte[] data)
        {
            // TODO understand what first 24 bytes stand for.
            var path = Encoding.Unicode.GetString(data[24..]).Split('\0').First();
            var id = 
                string.Join('\\',
                Encoding.Unicode.GetString(data[544..])
                .Split('\0')
                .First()
                .Replace(@"\\?\","")
                .Split('#').SkipLast(1));

            return (path, id);
        }

        public void UpdateWallpaper()
        {
            string path, id;

            var todo = Monitors.Items.ToList();
            using var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\\Desktop");

            // retrieve per monitor wallpaper if it exists
            if(key?.GetValue("TranscodedImageCount") is int nb)
            {
                for(var i = 0; i < nb; i++)
                {
                    if (key.GetValue($"TranscodedImageCache_{i:000}") is not byte[] imgCacheN) continue;

                    (path, id) = GetTranscodedImageCache(imgCacheN);

                    var monitors = Monitors.Items.Where(m => m.Edid.HKeyName.Contains(id)).ToList();

                    if (!monitors.Any()) continue;

                    foreach (var monitor in monitors)
                    {
                        var mirrors = Monitors.Items.Where(m => m.AttachedDisplay?.DeviceName == monitor.AttachedDisplay?.DeviceName);
                        foreach (var mirror in mirrors)
                        {
                           mirror.WallpaperPath = path;
                           if (todo.Contains(mirror)) todo.Remove(mirror);
                        }

                        monitor.MonitorNo = i + 1;
                    }
                }
            }

            if (!todo.Any()) return;

            // retrieve default wallpaper for other monitors
            if (key?.GetValue("TranscodedImageCache") is not byte[] imgCache) return;

            (path, id) = GetTranscodedImageCache(imgCache);

            foreach (var monitor in todo) monitor.WallpaperPath = path;
        }

        public void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true)
        {
            var devMode = new DevMode()
            {
                DeviceName = deviceName,
                Position = new WinDef.Point((int)area.X,(int)area.Y),
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

        public void DetachFromDesktop(string deviceName, bool apply = true)
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


        public void ApplyDesktop()
        {
            ChangeDisplaySettingsEx(null, 0, 0, 0, 0);
        }

        public RegistryKey OpenRootRegKey(bool create = false)
        {
            using var key = Registry.CurrentUser;
            return create ? key.CreateSubKey(ROOT_KEY) : key.OpenSubKey(ROOT_KEY);
        }
        public string AppDataPath(bool create)
        {
            var path = Path.Combine(Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse");

            if (create) Directory.CreateDirectory(path);

            return path;
        }

        bool ParseWindowsConfig()
        {
            using var configurationKey = GetConfigurationKey();
            if(configurationKey?.GetValue("SetId") is not string setId) return false;

            setId= setId.Trim('\0');
            var monitorNo = 1;

            var monitors = Monitors.Items.ToList();
            var idDisplays = setId.Split('+').Reverse();
            foreach (var idDisplay in idDisplays)
            {
                var idMonitors = idDisplay.Split('*').Reverse();
                DisplayDevice display = null;
                foreach(var idMonitor in idMonitors)
                {
                    var monitor = monitors.FirstOrDefault(m => m.IdMonitor == idMonitor);
                    if (monitor == null) return false;

                    if(display!=null)
                    {
                        if(!ReferenceEquals(display,monitor.AttachedDisplay)) return false;
                    }
                    else display = monitor.AttachedDisplay;
                    
                    monitor.MonitorNo = monitorNo++;

                    monitors.Remove(monitor);
                }
            }
            return !monitors.Any();
        }

        RegistryKey GetConfigurationKey()
        {
            using var key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Control\GraphicsDrivers\Configuration");
            foreach(var configurationKeyName in key.GetSubKeyNames())
            {
                var configurationKey = key.OpenSubKey(configurationKeyName);
                if (configurationKey?.GetValue("SetId") is string setId && MatchConfig(setId.Trim('\0')))
                {
                    return configurationKey;
                }
            }
            return null;
        }

        bool MatchConfig(string setId)
        {
            var devices = new List<DisplayDevice>();

            var monitors = Monitors.Items.ToList();
            var idDisplays = setId.Split('+');
            foreach (var idDisplay in idDisplays)
            {
                var idMonitors = idDisplay.Split('*');
                DisplayDevice display = null;
                foreach(var idMonitor in idMonitors)
                {
                    var monitor = monitors.FirstOrDefault(m => m.IdMonitor == idMonitor);
                    if (monitor == null) return false;

                    if(display!=null)
                    {
                        if(!ReferenceEquals(display,monitor.AttachedDisplay)) return false;
                    }
                    else 
                    {
                        display = monitor.AttachedDisplay; 
                        if(devices.Contains(display)) return false;
                        devices.Add(display);
                    }
                    
                    monitors.Remove(monitor);
                }
            }
            return !monitors.Any();
        }

    }
}
