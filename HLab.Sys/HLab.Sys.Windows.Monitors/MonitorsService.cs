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
using System.IO;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;
using HLab.Notify.PropertyChanged;
using Microsoft.Win32;
using NativeMethods = HLab.Sys.Windows.API.NativeMethods;

namespace HLab.Sys.Windows.Monitors
{
    using H = H<MonitorsService>;

    public interface IMonitorsService
    {
        ObservableCollectionSafe<MonitorDevice> Monitors { get; }
        ObservableCollectionSafe<DisplayDevice> Devices { get; }
        IObservableFilter<MonitorDevice> AttachedMonitors { get; }

        void DetachFromDesktop(string deviceName, bool apply = true);
        void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true);
        void ApplyDesktop();
        void UpdateDevices();

        RegistryKey OpenRootRegKey(bool create = false);
        string AppDataPath(bool create);
    }

    public class MonitorsService : NotifierBase, IMonitorsService
    {
        private const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

        public event EventHandler DevicesUpdated;

        public MonitorsService()
        {
            H.Initialize(this);

            _root = new DisplayDevice(this);

            UpdateDevices();

            var displayChanges = new DisplayChangesView();
            displayChanges.DisplayChanged += (o, a) => { UpdateDevices(); };

            displayChanges.Show();
            displayChanges.Hide();
        }

        public ObservableCollectionSafe<PhysicalAdapter> Adapters { get; } = new ObservableCollectionSafe<PhysicalAdapter>();
        public ObservableCollectionSafe<DisplayDevice> Devices { get; } = new ObservableCollectionSafe<DisplayDevice>();
        public ObservableCollectionSafe<MonitorDevice> Monitors { get; } = new ObservableCollectionSafe<MonitorDevice>();

        public IObservableFilter<MonitorDevice> AttachedMonitors { get; }
            = H.Filter<MonitorDevice>(c => c
                .On(e => e.Monitors.Item().AttachedToDesktop).Update()
                .AddFilter(m => m.AttachedToDesktop)
                .Link(e => e.Monitors)
            );

        public IObservableFilter<MonitorDevice> UnattachedMonitors { get; }
            = H.Filter<MonitorDevice>(c => c
                .On(e => e.Monitors.Item().AttachedToDesktop).Update()
                .AddFilter(m => !m.AttachedToDesktop)
                .Link(e => e.Monitors)
            );

        public MonitorDevice GetOrAddMonitor(string deviceId, Func<MonitorDevice> get) =>
            Monitors.GetOrAdd(m => m.DeviceId == deviceId, get);

        public PhysicalAdapter GetOrAddAdapter(string deviceId, Func<PhysicalAdapter> get) =>
            Adapters.GetOrAdd(m => m.DeviceId == deviceId, get);

        private readonly DisplayDevice _root;
        public void UpdateDevices()
        {
            var oldDevices = Devices.ToList();
            var oldMonitors = Monitors.ToList();

            _root.Init(null,new NativeMethods.DISPLAY_DEVICE(true){DeviceID = "ROOT",DeviceName = null}, oldDevices, oldMonitors);

            foreach (var d in oldDevices)
            {
                Devices.Remove(d);
            }

            foreach (var m in oldMonitors)
            {
                Monitors.Remove(m);
            }

            AttachedMonitors.OnTriggered();
            foreach (var m in AttachedMonitors) m.Devices.OnTriggered();

            // GetMonitorInfo
            NativeMethods.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref NativeMethods.RECT lprcMonitor, IntPtr dwData)
                {
                    var mi = new NativeMethods.MONITORINFOEX(true);
                    var success = NativeMethods.GetMonitorInfo(hMonitor, ref mi);
                    if (success)
                    {
                        var monitors = AttachedMonitors.Where(d => d.AttachedDisplay?.DeviceName == mi.DeviceName).ToList();
                        foreach (var monitor in monitors)
                        {
                            monitor.SetMonitorInfoEx(mi);
                            monitor.UpdateDpi(hMonitor);
                        }
                    }

                    return true; // Continue
                }, IntPtr.Zero);

            //ParseWindowsConfig();
            UpdateWallpaper();

            string FromUShort(ushort[] array)
            {
                var sb = new StringBuilder();
                for (int i = 0; i < array.Length; i++)
                {
                    sb.Append( (char)(array[i]) );
                }
                return sb.ToString().Split('\0').First();
            }

            try
            {
                ConnectionOptions aConnectionOptions = new();
                ManagementScope aManagementScope = new(@"\\.\root\WMI", aConnectionOptions);
                ObjectQuery aObjectQuery = new("SELECT * FROM WmiMonitorID");
                ManagementObjectSearcher aManagementObjectSearcher = new(aManagementScope, aObjectQuery);
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

        private static (string path, string id) GetTranscodedImageCache(byte[] data)
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

            var todo = Monitors.ToList();
            using var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\\Desktop");

            if(key?.GetValue("TranscodedImageCount") is int nb)
            {
                for(var i = 0; i < nb; i++)
                {
                    if (key.GetValue($"TranscodedImageCache_{i:000}") is not byte[] imgCacheN) continue;

                    (path, id) = GetTranscodedImageCache(imgCacheN);

                    var monitors = Monitors.Where(m => m.HKeyName.Contains(id)).ToList();

                    if (!monitors.Any()) continue;

                    foreach (var monitor in monitors)
                    {
                        var mirrors = Monitors.Where(m => m.AttachedDisplay?.DeviceName == monitor.AttachedDisplay?.DeviceName);
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

            if (key?.GetValue("TranscodedImageCache") is not byte[] imgCache) return;

            (path, id) = GetTranscodedImageCache(imgCache);

            foreach (var monitor in todo) monitor.WallpaperPath = path;
        }

        public void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true)
        {
            var devMode = new NativeMethods.DEVMODE(true)
            {
                DeviceName = deviceName,
                Position = new NativeMethods.POINTL { x = (int)area.X, y = (int)area.Y },
                PelsWidth = (int)area.Width,
                PelsHeight = (int)area.Height,
                DisplayOrientation = orientation,
                BitsPerPel = 32,
                Fields = NativeMethods.DM.Position | NativeMethods.DM.PelsHeight | NativeMethods.DM.PelsWidth | NativeMethods.DM.DisplayOrientation | NativeMethods.DM.BitsPerPixel
            };

            var flag =
                NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY |
                NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET;

            if (primary) flag |= NativeMethods.ChangeDisplaySettingsFlags.CDS_SET_PRIMARY;


            var ch = NativeMethods.ChangeDisplaySettingsEx(deviceName, ref devMode, IntPtr.Zero, flag, IntPtr.Zero);

            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }

        public void DetachFromDesktop(string deviceName, bool apply = true)
        {
            var devmode = new NativeMethods.DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);

            devmode.DeviceName = deviceName;
            devmode.PelsHeight = 0;
            devmode.PelsWidth = 0;
            devmode.Fields = NativeMethods.DM.PelsWidth | NativeMethods.DM.PelsHeight /*| DM.BitsPerPixel*/ | NativeMethods.DM.Position
                             | NativeMethods.DM.DisplayFrequency | NativeMethods.DM.DisplayFlags;

            var ch = NativeMethods.ChangeDisplaySettingsEx(deviceName, ref devmode, IntPtr.Zero, NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }


        public void ApplyDesktop()
        {
            NativeMethods.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
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

        private bool ParseWindowsConfig()
        {
            using var configurationKey = GetConfigurationKey();
            if (configurationKey == null) return false;
            if(!(configurationKey.GetValue("SetId") is string setId)) return false;

            setId= setId.Trim('\0');
            var monitorNo = 1;

            var monitors = Monitors.ToList();
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
            if(monitors.Any()) return false;
            return true;
        }

        private RegistryKey GetConfigurationKey()
        {
            using var key = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Control\GraphicsDrivers\Configuration");
            foreach(var configurationKeyName in key.GetSubKeyNames())
            {
                var configurationKey = key.OpenSubKey(configurationKeyName);
                if (configurationKey.GetValue("SetId") is string setId && MatchConfig(setId.Trim('\0')))
                {
                    return configurationKey;
                }
            }
            return null;
        }

        private bool MatchConfig(string setId)
        {
            List<DisplayDevice> devices = new();

            var monitors = Monitors.ToList();
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
            if(monitors.Any()) return false;
            return true;
        }

    }
}
