/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Microsoft.Win32;
using System;
using System.Runtime.InteropServices;
using System.Windows;
using WinAPI_AdvAPI32;
using WinAPI_Kernel32;
using WinAPI_Ntdll;
using WinAPI_SetupAPI;
using WinAPI_User32;

namespace LittleBigMouse
{
    public class Edid
    {

        public static String GetHKeyName(IntPtr hKey)
        {
            String result = String.Empty;
            IntPtr pKNI = IntPtr.Zero;

            int needed = 0;
            int status = Ntdll.ZwQueryKey(hKey, KEY_INFORMATION_CLASS.KeyNameInformation, IntPtr.Zero, 0, out needed);
            if ((UInt32)status == 0xC0000023)   // STATUS_BUFFER_TOO_SMALL
            {
                pKNI = Marshal.AllocHGlobal(sizeof(UInt32) + needed + 4 /*paranoia*/);
                status = Ntdll.ZwQueryKey(hKey, KEY_INFORMATION_CLASS.KeyNameInformation, pKNI, needed, out needed);
                if (status == 0)    // STATUS_SUCCESS
                {
                    char[] bytes = new char[2 + needed + 2];
                    Marshal.Copy(pKNI, bytes, 0, needed);
                    // startIndex == 2  skips the NameLength field of the structure (2 chars == 4 bytes)
                    // needed/2         reduces value from bytes to chars
                    //  needed/2 - 2    reduces length to not include the NameLength
                    result = new String(bytes, 2, (needed / 2) - 2);
                }
            }
            Marshal.FreeHGlobal(pKNI);
            return result;
        }


        private void GetEdid(RegistryKey key)
        {
            _rawData = (byte[])key.GetValue("EDID");
        }

        private byte[] _rawData = new byte[0];

        public RegistryKey GetKeyFromPath(String path, int parent=0)
        {
            String[] keys = path.Split('\\');

            RegistryKey key = Registry.LocalMachine;

            switch(keys[2])
            {
                case "MACHINE": key = Registry.LocalMachine; break;
                case "USER": key = Registry.CurrentUser; break;
                case "CONFIG": key = Registry.CurrentConfig; break;
            }

            for(int i=3;i<(keys.Length-parent);i++)
            {
                key = key.OpenSubKey(keys[i]);
            }

            return key;
        }

        public Edid(String TargetDevID)
        {
            DISPLAY_DEVICE dd = DisplayDeviceFromID(TargetDevID);


            IntPtr devInfo = SetupAPI.SetupDiGetClassDevsEx(
                    ref SetupAPI.GUID_CLASS_MONITOR, //class GUID
                    null, //enumerator
                    IntPtr.Zero, //HWND
                    SetupAPI.DIGCF_PRESENT | SetupAPI.DIGCF_PROFILE, // Flags //DIGCF_ALLCLASSES|
                    IntPtr.Zero, // device info, create a new one.
                    null, // machine name, local machine
                    IntPtr.Zero
                );// reserved

            if (devInfo == IntPtr.Zero)
                return;

            for (uint i = 0; Kernel32.ERROR_NO_MORE_ITEMS != Kernel32.GetLastError(); ++i)
            {
                SP_DEVINFO_DATA devInfoData = new SP_DEVINFO_DATA();
                devInfoData.cbSize = (uint)Marshal.SizeOf(devInfoData);

                if (SetupAPI.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {

                    IntPtr hEDIDRegKey = SetupAPI.SetupDiOpenDevRegKey(devInfo, ref devInfoData, SetupAPI.DICS_FLAG_GLOBAL, 0, SetupAPI.DIREG_DEV, SetupAPI.KEY_READ);

                    if (hEDIDRegKey == IntPtr.Zero || (hEDIDRegKey.ToInt32() == -1))
                        continue;

                    RegistryKey key = GetKeyFromPath( GetHKeyName(hEDIDRegKey) , 1 );

                    String id = ((String[])key.GetValue("HardwareID"))[0].ToString() + "\\" + key.GetValue("Driver").ToString();

                    if (id != dd.DeviceID)
                        continue;

                    GetEdid(GetKeyFromPath(GetHKeyName(hEDIDRegKey)));
                    AdvAPI32.RegCloseKey(hEDIDRegKey);
                }
            }
            SetupAPI.SetupDiDestroyDeviceInfoList(devInfo);
        }

        //TODO : implement correctly
        public bool IsValid
        {
            get
            {
                if (_rawData.Length > 0) return true;
                return false;
            }
        }

        public Size PhysicalSize
        {
            get
            {
                if (_rawData.Length >= 68)
                {
                    int w = ((_rawData[68] & 0xF0) << 4) + _rawData[66];
                    int h = ((_rawData[68] & 0x0F) << 8) + _rawData[67];
                    return new Size(w, h);
                }
                else
                {
                    // TODO: ???
                    return new Size();
                }

            }
        }

        public String ManufacturerCode
        {
            get
            {
                String code ="";
                code += (char)(64 + ((_rawData[8] >> 2) & 0x1F));
                code += (char)(64 + (((_rawData[8] << 3) | (_rawData[9] >> 5)) & 0x1F));
                code += (char)(64 + (_rawData[9] & 0x1F));
                return code;
            }
        }

        public String ProductCode
        {
            get { return (_rawData[10] + (_rawData[11] << 8)).ToString("X4"); }
        }

        public String Serial
        {
            get {
                String serial = "";
                for (int i = 12; i <= 15; i++) serial = (_rawData[i]).ToString("X2") + serial;
                return serial;
            }
        }

        public string Block(char code)
        {
            for (int i=54;i<=108;i+=18)
            {
                if (_rawData[i]==0 && _rawData[i+1]==0 && _rawData[i+2]==0 && _rawData[i+3]==code)
                {
                    String s = "";
                    for (int j = i + 5; j < i + 18; j++)
                    { char c = (char)_rawData[j];
                        if (c == (char)0x0A) break;
                        s += c;
                    }
                    return s;
                }
            }
            return "";
        }

        DISPLAY_DEVICE DisplayDeviceFromHMonitor(IntPtr hMonitor)
        {
            MONITORINFOEX mi = new MONITORINFOEX();
            mi.Init();
            User32.GetMonitorInfo(hMonitor, ref mi);

            return DisplayDeviceFromID(mi.DeviceName);
        }

        DISPLAY_DEVICE DisplayDeviceFromID(String id)
        {
                DISPLAY_DEVICE ddMon = new DISPLAY_DEVICE();
                ddMon.cb = Marshal.SizeOf(ddMon);
                uint MonIdx = 0;

                while (User32.EnumDisplayDevices(id, MonIdx, ref ddMon, 0))
                {
                    MonIdx++;
                    return ddMon; // TODO : we postulate that there is only 1 monitor per display
                }
                return ddMon;
        }
    }
}
