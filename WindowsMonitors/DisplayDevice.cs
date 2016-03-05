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

        public static ObservableCollection<DisplayAdapter> AllAdapters { get; } = new ObservableCollection<DisplayAdapter>();
        public static ObservableCollection<DisplayMonitor> AllMonitors { get; } = new ObservableCollection<DisplayMonitor>();

        public static DisplayAdapter FromId(string id)
        {
            return (from monitor in AllMonitors where monitor.DeviceId == id select monitor.Adapter).FirstOrDefault();
        }


        public static void UpdateDevices()
        {
            NativeMethods.DISPLAY_DEVICE dev = new NativeMethods.DISPLAY_DEVICE(true);
            uint i = 0;

            while (NativeMethods.EnumDisplayDevices(null, i++, ref dev, 0))
            {
                DisplayAdapter adapter = AllAdapters.FirstOrDefault(d => d.DeviceName == dev.DeviceName);
                if (adapter == null) adapter = new DisplayAdapter(dev);
                else adapter.Init(dev);
            }

            IList<DisplayMonitor> existing = AllMonitors.ToList();

            NativeMethods.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref NativeMethods.RECT lprcMonitor, IntPtr dwData)
                {
                    NativeMethods.MONITORINFOEX mi = new NativeMethods.MONITORINFOEX(true);
                    bool success = NativeMethods.GetMonitorInfo(hMonitor, ref mi);
                    if (success)
                    {
                        var ddDev = AllAdapters.FirstOrDefault(d => d.DeviceName == mi.DeviceName);
                        if (ddDev != null)
                        {
                            if (ddDev.Monitors.Count == 0)
                            {
                                DisplayMonitor dummy = new DisplayMonitor(ddDev,
                                    new NativeMethods.DISPLAY_DEVICE { DeviceName = ddDev.DeviceName + @"\Monitor0" });
                                AllMonitors.Add(dummy);
                            }

                            foreach (var ddMon in ddDev.Monitors)
                            {
                                ddMon.Init(hMonitor, mi);
                                if (existing.Contains(ddMon))
                                    existing.Remove(ddMon);
                                else
                                    AllMonitors.Add(ddMon);
                            }
                        }
                    }

                    return true;
                }, IntPtr.Zero);

            foreach (DisplayMonitor monitor in existing)
            {
                AllMonitors.Remove(monitor);
            }

            UpdateEdid();
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

        public static void UpdateEdid()
        {
            IntPtr devInfo = NativeMethods.SetupDiGetClassDevsEx(
                ref NativeMethods.GUID_CLASS_MONITOR, //class GUID
                null, //enumerator
                IntPtr.Zero, //HWND
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_PROFILE, // Primary //DIGCF_ALLCLASSES|
                IntPtr.Zero, // device info, create a new one.
                null, // machine name, local machine
                 IntPtr.Zero
            );// reserved

            if (devInfo == IntPtr.Zero)
                return;

            NativeMethods.SP_DEVINFO_DATA devInfoData = new NativeMethods.SP_DEVINFO_DATA(true);

            uint i = 0;
            //            string s = screen.DeviceName.Substring(11);
            //            uint i = 3-uint.Parse(s);

            do
            {
                if (NativeMethods.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {

                    IntPtr hEdidRegKey = NativeMethods.SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                        NativeMethods.DICS_FLAG_GLOBAL, 0, NativeMethods.DIREG_DEV, NativeMethods.KEY_READ);

                    if (hEdidRegKey != IntPtr.Zero && (hEdidRegKey.ToInt32() != -1))
                    {
                        using (RegistryKey key = GetKeyFromPath(GetHKeyName(hEdidRegKey), 1))
                        {
                            string id = ((string[])key.GetValue("HardwareID"))[0] + "\\" + key.GetValue("Driver");

                            DisplayMonitor mon = AllMonitors.FirstOrDefault(m => m.DeviceId == id);
                            if (mon != null)
                            {
                                mon.HKeyName = GetHKeyName(hEdidRegKey);
                                using (RegistryKey keyEdid = GetKeyFromPath(mon.HKeyName))
                                {
                                    mon.Edid = (byte[])keyEdid.GetValue("EDID");
                                }
                            }
                        }
                        NativeMethods.RegCloseKey(hEdidRegKey);
                    }
                }
                i++;
            } while (NativeMethods.ERROR_NO_MORE_ITEMS != NativeMethods.GetLastError());

            NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
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

