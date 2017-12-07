/*
  WindowsMonitors - Windows Monitors Enumeration for .Net
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of WindowsMonitors.

    WindowsMonitors is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    WindowsMonitors is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Hlab.Notify;
using Microsoft.Win32;
using WinAPI;

namespace HLab.Windows.Monitors
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

