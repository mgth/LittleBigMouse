using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Runtime.InteropServices;
using Microsoft.Win32;
using NotifyChange;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayDevice : Notifier
    {
        static DisplayDevice()
        {
            SystemEvents.DisplaySettingsChanged += SystemEventsOnDisplaySettingsChanged;
            UpdateDevices();
        }

 
        private static void SystemEventsOnDisplaySettingsChanged(object sender, EventArgs eventArgs)
        {
            UpdateDevices();
        }

        public static ObservableCollection<DisplayMonitor> AllMonitors { get; } = new ObservableCollection<DisplayMonitor>();
        public static List<DisplayMonitor> TempMonitors { get; private set; }

        public static DisplayAdapter FromId(string id)
        {
            return (from monitor in AllMonitors where monitor.DeviceId == id select monitor.Adapter).FirstOrDefault();
        }


        public static void UpdateDevices()
        {
            // Todo: try not to clear berore updating, but it's very buggy now
            while(AllMonitors.Count>0) AllMonitors.RemoveAt(0);

            TempMonitors = new List<DisplayMonitor>();

            IList<DisplayMonitor> oldMonitors = AllMonitors.ToList();

            NativeMethods.DISPLAY_DEVICE dev = new NativeMethods.DISPLAY_DEVICE(true);
            uint i = 0;

            while (NativeMethods.EnumDisplayDevices(null, i++, ref dev, 0))
            {
                new DisplayAdapter(dev);
            }

            NativeMethods.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref NativeMethods.RECT lprcMonitor, IntPtr dwData)
                {
                    NativeMethods.MONITORINFOEX mi = new NativeMethods.MONITORINFOEX(true);
                    bool success = NativeMethods.GetMonitorInfo(hMonitor, ref mi);
                    if (success)
                    {
                        IList monitors = TempMonitors.Where(d => d.Adapter.DeviceName == mi.DeviceName).ToList();
                        foreach (DisplayMonitor ddMon in monitors)
                        {
                            ddMon.Init(hMonitor, mi);

                            if (oldMonitors.Contains(ddMon))
                                oldMonitors.Remove(ddMon);
                                
                            if(!AllMonitors.Contains(ddMon)) AllMonitors.Add(ddMon);
                        }
                        
                    }

                    return true;
                }, IntPtr.Zero);

            foreach (DisplayMonitor monitor in oldMonitors)
            {
                AllMonitors.Remove(monitor);
            }
        }
        public static RegistryKey GetKeyFromPath(string path, int parent = 0)
        {
            var keys = path.Split('\\');

            RegistryKey key;

            switch (keys[2])
            {
                case "USER": key = Registry.CurrentUser; break;
                case "CONFIG": key = Registry.CurrentConfig; break;
                default: key = Registry.LocalMachine; break;
            }

            for (var i = 3; i < (keys.Length - parent); i++)
            {
                if (key == null) return key;
                key = key.OpenSubKey(keys[i]);
            }

            return key;
        }
        public static string GetHKeyName(IntPtr hKey)
        {
            var result = string.Empty;
            var pKNI = IntPtr.Zero;

            var needed = 0;
            var status = NativeMethods.ZwQueryKey(hKey, NativeMethods.KEY_INFORMATION_CLASS.KeyNameInformation, IntPtr.Zero, 0, out needed);
            if (status != 0xC0000023) return result;

            pKNI = Marshal.AllocHGlobal(cb: sizeof(uint) + needed + 4 /*paranoia*/);
            status = NativeMethods.ZwQueryKey(hKey, NativeMethods.KEY_INFORMATION_CLASS.KeyNameInformation, pKNI, needed, out needed);
            if (status == 0)    // STATUS_SUCCESS
            {
                var bytes = new char[2 + needed + 2];
                Marshal.Copy(pKNI, bytes, 0, needed);
                // startIndex == 2  skips the NameLength field of the structure (2 chars == 4 bytes)
                // needed/2         reduces value from bytes to chars
                //  needed/2 - 2    reduces length to not include the NameLength
                result = new string(bytes, 2, (needed / 2) - 2);
            }
            Marshal.FreeHGlobal(pKNI);
            return result;
        }



        private string _deviceName = "";
        private string _deviceString = "";
        private NativeMethods.DisplayDeviceStateFlags _stateFlags;
        private string _deviceId;
        private string _deviceKey;


        public string DeviceName
        {
            get { return _deviceName; }
            internal set
            {
                if (SetProperty(ref _deviceName, value ?? ""))
                {
                    if (string.IsNullOrWhiteSpace(DeviceString))
                    {
                        string[] s = DeviceName.Split('\\');
                        if (s.Length > 3) DeviceString = s[3];
                    }
                }
            }
        }

        public string DeviceString
        {
            get { return _deviceString; }
            internal set { SetProperty(ref _deviceString, value ?? ""); }
        }

        public NativeMethods.DisplayDeviceStateFlags State
        {
            get { return _stateFlags; }
            set { SetProperty(ref _stateFlags, value); }
        }


        public string DeviceId
        {
            get { return _deviceId; }
            internal set { SetProperty(ref _deviceId, value); }
        }

        public string DeviceKey
        {
            get { return _deviceKey; }
            internal set { SetProperty(ref _deviceKey, value); }
        }
    }


}

