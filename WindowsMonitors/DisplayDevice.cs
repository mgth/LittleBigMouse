using System;
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.InteropServices;
using Erp.Notify;
using Microsoft.Win32;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayDevice : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
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



        public string DeviceName
        {
            get => this.Get<string>();
            internal set {
                if (this.Set(value ?? ""))
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
            get => this.Get<string>();
            internal set => this.Set(value ?? "");
        }

        public NativeMethods.DisplayDeviceStateFlags State
        {
            get => this.Get<NativeMethods.DisplayDeviceStateFlags>();
            set => this.Set(value);
        }

        public string DeviceId
        {
            get => this.Get<string>();
            internal set => this.Set(value);
        }

        public string DeviceKey
        {
            get => this.Get<string>();
            internal set => this.Set(value);
        }
    }
}

