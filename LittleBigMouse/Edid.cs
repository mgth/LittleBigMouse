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
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows;

namespace LittleBigMouse
{
    public class Edid
    {
        enum KEY_INFORMATION_CLASS
        {
            KeyBasicInformation,            // A KEY_BASIC_INFORMATION structure is supplied.
            KeyNodeInformation,             // A KEY_NODE_INFORMATION structure is supplied.
            KeyFullInformation,             // A KEY_FULL_INFORMATION structure is supplied.
            KeyNameInformation,             // A KEY_NAME_INFORMATION structure is supplied.
            KeyCachedInformation,           // A KEY_CACHED_INFORMATION structure is supplied.
            KeyFlagsInformation,            // Reserved for system use.
            KeyVirtualizationInformation,   // A KEY_VIRTUALIZATION_INFORMATION structure is supplied.
            KeyHandleTagsInformation,       // Reserved for system use.
            MaxKeyInfoClass                 // The maximum value in this enumeration type.
        }
        [StructLayout(LayoutKind.Sequential)]
        public struct KEY_NAME_INFORMATION
        {
            public UInt32 NameLength;     // The size, in bytes, of the key name string in the Name array.
            public char[] Name;           // An array of wide characters that contains the name of the key.
                                          // This character string is not null-terminated.
                                          // Only the first element in this array is included in the
                                          //    KEY_NAME_INFORMATION structure definition.
                                          //    The storage for the remaining elements in the array immediately
                                          //    follows this element.
        }

        [DllImport("ntdll.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern int ZwQueryKey(IntPtr hKey, KEY_INFORMATION_CLASS KeyInformationClass, IntPtr lpKeyInformation, int Length, out int ResultLength);

        public static String GetHKeyName(IntPtr hKey)
        {
            String result = String.Empty;
            IntPtr pKNI = IntPtr.Zero;

            int needed = 0;
            int status = ZwQueryKey(hKey, KEY_INFORMATION_CLASS.KeyNameInformation, IntPtr.Zero, 0, out needed);
            if ((UInt32)status == 0xC0000023)   // STATUS_BUFFER_TOO_SMALL
            {
                pKNI = Marshal.AllocHGlobal(sizeof(UInt32) + needed + 4 /*paranoia*/);
                status = ZwQueryKey(hKey, KEY_INFORMATION_CLASS.KeyNameInformation, pKNI, needed, out needed);
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

        [StructLayout(LayoutKind.Sequential)]
        public struct RECT
        {
            public int Left, Top, Right, Bottom;

            public RECT(int left, int top, int right, int bottom)
            {
                Left = left;
                Top = top;
                Right = right;
                Bottom = bottom;
            }

            public RECT(System.Drawing.Rectangle r) : this(r.Left, r.Top, r.Right, r.Bottom) { }

            public int X
            {
                get { return Left; }
                set { Right -= (Left - value); Left = value; }
            }

            public int Y
            {
                get { return Top; }
                set { Bottom -= (Top - value); Top = value; }
            }

            public int Height
            {
                get { return Bottom - Top; }
                set { Bottom = value + Top; }
            }

            public int Width
            {
                get { return Right - Left; }
                set { Right = value + Left; }
            }

            public System.Drawing.Point Location
            {
                get { return new System.Drawing.Point(Left, Top); }
                set { X = value.X; Y = value.Y; }
            }

            public System.Drawing.Size Size
            {
                get { return new System.Drawing.Size(Width, Height); }
                set { Width = value.Width; Height = value.Height; }
            }

            public static implicit operator System.Drawing.Rectangle(RECT r)
            {
                return new System.Drawing.Rectangle(r.Left, r.Top, r.Width, r.Height);
            }

            public static implicit operator RECT(System.Drawing.Rectangle r)
            {
                return new RECT(r);
            }

            public static bool operator ==(RECT r1, RECT r2)
            {
                return r1.Equals(r2);
            }

            public static bool operator !=(RECT r1, RECT r2)
            {
                return !r1.Equals(r2);
            }

            public bool Equals(RECT r)
            {
                return r.Left == Left && r.Top == Top && r.Right == Right && r.Bottom == Bottom;
            }

            public override bool Equals(object obj)
            {
                if (obj is RECT)
                    return Equals((RECT)obj);
                else if (obj is System.Drawing.Rectangle)
                    return Equals(new RECT((System.Drawing.Rectangle)obj));
                return false;
            }

            public override int GetHashCode()
            {
                return ((System.Drawing.Rectangle)this).GetHashCode();
            }

            public override string ToString()
            {
                return string.Format(System.Globalization.CultureInfo.CurrentCulture, "{{Left={0},Top={1},Right={2},Bottom={3}}}", Left, Top, Right, Bottom);
            }
        }
        [StructLayout(LayoutKind.Sequential)]
        private struct MonitorInfo
        {
            public int cbSize;
            public RECT rcMonitor;
            public RECT rcWork;
            public uint dwFlags;
        }
        // size of a device name string
        private const int CCHDEVICENAME = 32;

        /// <summary>
        /// The MONITORINFOEX structure contains information about a display monitor.
        /// The GetMonitorInfo function stores information into a MONITORINFOEX structure or a MONITORINFO structure.
        /// The MONITORINFOEX structure is a superset of the MONITORINFO structure. The MONITORINFOEX structure adds a string member to contain a name 
        /// for the display monitor.
        /// </summary>
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        internal struct MonitorInfoEx
        {
            /// <summary>
            /// The size, in bytes, of the structure. Set this member to sizeof(MONITORINFOEX) (72) before calling the GetMonitorInfo function. 
            /// Doing so lets the function determine the type of structure you are passing to it.
            /// </summary>
            public int Size;

            /// <summary>
            /// A RECT structure that specifies the display monitor rectangle, expressed in virtual-screen coordinates. 
            /// Note that if the monitor is not the primary display monitor, some of the rectangle's coordinates may be negative values.
            /// </summary>
            public RectStruct Monitor;

            /// <summary>
            /// A RECT structure that specifies the work area rectangle of the display monitor that can be used by applications, 
            /// expressed in virtual-screen coordinates. Windows uses this rectangle to maximize an application on the monitor. 
            /// The rest of the area in rcMonitor contains system windows such as the task bar and side bars. 
            /// Note that if the monitor is not the primary display monitor, some of the rectangle's coordinates may be negative values.
            /// </summary>
            public RectStruct WorkArea;

            /// <summary>
            /// The attributes of the display monitor.
            /// 
            /// This member can be the following value:
            ///   1 : MONITORINFOF_PRIMARY
            /// </summary>
            public uint Flags;

            /// <summary>
            /// A string that specifies the device name of the monitor being used. Most applications have no use for a display monitor name, 
            /// and so can save some bytes by using a MONITORINFO structure.
            /// </summary>
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = CCHDEVICENAME)]
            public string DeviceName;

            public void Init()
            {
                this.Size = 40 + 2 * CCHDEVICENAME;
                this.DeviceName = string.Empty;
            }
        }

        /// <summary>
        /// The RECT structure defines the coordinates of the upper-left and lower-right corners of a rectangle.
        /// </summary>
        /// <see cref="http://msdn.microsoft.com/en-us/library/dd162897%28VS.85%29.aspx"/>
        /// <remarks>
        /// By convention, the right and bottom edges of the rectangle are normally considered exclusive. 
        /// In other words, the pixel whose coordinates are ( right, bottom ) lies immediately outside of the the rectangle. 
        /// For example, when RECT is passed to the FillRect function, the rectangle is filled up to, but not including, 
        /// the right column and bottom row of pixels. This structure is identical to the RECTL structure.
        /// </remarks>
        [StructLayout(LayoutKind.Sequential)]
        public struct RectStruct
        {
            /// <summary>
            /// The x-coordinate of the upper-left corner of the rectangle.
            /// </summary>
            public int Left;

            /// <summary>
            /// The y-coordinate of the upper-left corner of the rectangle.
            /// </summary>
            public int Top;

            /// <summary>
            /// The x-coordinate of the lower-right corner of the rectangle.
            /// </summary>
            public int Right;

            /// <summary>
            /// The y-coordinate of the lower-right corner of the rectangle.
            /// </summary>
            public int Bottom;
        }
        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        static extern bool GetMonitorInfo(IntPtr hMonitor, ref MonitorInfoEx lpmi);
        [DllImport("user32.dll")]
        static extern bool GetMonitorInfo(IntPtr hMonitor, ref MonitorInfo lpmi);
        [Flags()]
        public enum DisplayDeviceStateFlags : int
        {
            /// <summary>The device is part of the desktop.</summary>
            AttachedToDesktop = 0x1,
            MultiDriver = 0x2,
            /// <summary>The device is part of the desktop.</summary>
            PrimaryDevice = 0x4,
            /// <summary>Represents a pseudo device used to mirror application drawing for remoting or other purposes.</summary>
            MirroringDriver = 0x8,
            /// <summary>The device is VGA compatible.</summary>
            VGACompatible = 0x10,
            /// <summary>The device is removable; it cannot be the primary display.</summary>
            Removable = 0x20,
            /// <summary>The device has more display modes than its output devices support.</summary>
            ModesPruned = 0x8000000,
            Remote = 0x4000000,
            Disconnect = 0x2000000
        }
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
        public struct DISPLAY_DEVICE
        {
            [MarshalAs(UnmanagedType.U4)]
            public int cb;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)]
            public string DeviceName;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
            public string DeviceString;
            [MarshalAs(UnmanagedType.U4)]
            public DisplayDeviceStateFlags StateFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
            public string DeviceID;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
            public string DeviceKey;
        }
        [DllImport("user32.dll")]
        static extern bool EnumDisplayDevices(string lpDevice, uint iDevNum, ref DISPLAY_DEVICE lpDisplayDevice, uint dwFlags);

        delegate bool EnumMonitorsDelegate(IntPtr hMonitor, IntPtr hdcMonitor, ref Rect lprcMonitor, IntPtr dwData);

        [DllImport("user32.dll")]
        static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip,
           EnumMonitorsDelegate lpfnEnum, IntPtr dwData);



        [DllImport("setupapi.dll")]
        internal static extern IntPtr SetupDiGetClassDevsEx(ref Guid ClassGuid,
            [MarshalAs(UnmanagedType.LPStr)]String enumerator,
            IntPtr hwndParent, Int32 Flags, IntPtr DeviceInfoSet,
            [MarshalAs(UnmanagedType.LPStr)]String MachineName, IntPtr Reserved);

        [DllImport("setupapi.dll", SetLastError = true)]
        static extern bool SetupDiEnumDeviceInfo(IntPtr DeviceInfoSet, uint MemberIndex, ref SP_DEVINFO_DATA DeviceInfoData);

        [DllImport("setupapi.dll", SetLastError = true)]
        public static extern bool SetupDiDestroyDeviceInfoList
        (
             IntPtr DeviceInfoSet
        );

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();

        [DllImport("Setupapi", CharSet = CharSet.Auto, SetLastError = true)]
        internal static extern IntPtr SetupDiOpenDevRegKey(
            IntPtr hDeviceInfoSet,
            ref SP_DEVINFO_DATA deviceInfoData,
            int scope,
            int hwProfile,
            int parameterRegistryValueKind,
            int samDesired);

        [DllImport("advapi32.dll", SetLastError = true)]
        static extern uint RegEnumValue(
              IntPtr hKey,
              uint dwIndex,
              StringBuilder lpValueName,
              ref uint lpcValueName,
              IntPtr lpReserved,
              ref UInt32 lpType,
              IntPtr lpData,
              ref UInt32 lpcbData);

        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern int RegCloseKey(
            IntPtr hKey);

        [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Auto)]
        static extern bool SetupDiGetDeviceInstanceId(
           IntPtr DeviceInfoSet,
           ref SP_DEVINFO_DATA DeviceInfoData,
           StringBuilder DeviceInstanceId,
           int DeviceInstanceIdSize,
           out int RequiredSize
        );


        [StructLayout(LayoutKind.Sequential)]
        internal struct SP_DEVICE_INTERFACE_DATA
        {
            public Int32 cbSize;
            public Guid interfaceClassGuid;
            public Int32 flags;
            private UIntPtr reserved;
        }

        [DllImport("Setupapi", CharSet = CharSet.Auto, SetLastError = true)]
        internal static extern IntPtr SetupDiOpenDeviceInterfaceRegKey(
            IntPtr hDeviceInfoSet,
            ref SP_DEVICE_INTERFACE_DATA deviceInfoData,
            int Reserved,
            int samDesired);


        const int MAX_DEVICE_ID_LEN = 200;
        const int MAX_PATH = 260;

        const int DIGCF_PRESENT = 0x2;
        const int DIGCF_PROFILE = 0x8;
        const int DICS_FLAG_GLOBAL = 0x1;
        const int DIREG_DEV = 0x1;

        const int KEY_READ = 0x20019;

        static Guid GUID_CLASS_MONITOR = new Guid(0x4d36e96e, 0xe325, 0x11ce, 0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18);

        const uint ERROR_NO_MORE_ITEMS = 259;
        [StructLayout(LayoutKind.Sequential)]
        internal struct SP_DEVINFO_DATA
        {
            public uint cbSize;
            public Guid classGuid;
            public uint devInst;
            public IntPtr reserved;
        }

        private const Int32 NAME_SIZE = 128;
        private const UInt32 ERROR_SUCCESS = 0;

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


            IntPtr devInfo = SetupDiGetClassDevsEx(
        ref GUID_CLASS_MONITOR, //class GUID
        null, //enumerator
        IntPtr.Zero, //HWND
        DIGCF_PRESENT | DIGCF_PROFILE, // Flags //DIGCF_ALLCLASSES|
        IntPtr.Zero, // device info, create a new one.
        null, // machine name, local machine
        IntPtr.Zero);// reserved

            if (devInfo == IntPtr.Zero)
                return;

            for (uint i = 0; ERROR_NO_MORE_ITEMS != GetLastError(); ++i)
            {
                SP_DEVINFO_DATA devInfoData = new SP_DEVINFO_DATA();
                //memset(&devInfoData, 0, sizeof(devInfoData));
                devInfoData.cbSize = (uint)Marshal.SizeOf(devInfoData);

                SP_DEVICE_INTERFACE_DATA InterfaceData = new SP_DEVICE_INTERFACE_DATA();
                InterfaceData.cbSize = Marshal.SizeOf(InterfaceData);

                if (SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {
                    //StringBuilder Instance = new StringBuilder(MAX_DEVICE_ID_LEN);
                    //int reqSize = 0;
                    //SetupDiGetDeviceInstanceId(devInfo, ref devInfoData, Instance, MAX_PATH, out reqSize);

                    IntPtr hEDIDRegKey = SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ);

                    if (hEDIDRegKey == IntPtr.Zero || (hEDIDRegKey.ToInt32() == -1))
                              continue;

                    RegistryKey key = GetKeyFromPath( GetHKeyName(hEDIDRegKey) , 1 );

                    String id = ((String[])key.GetValue("HardwareID"))[0].ToString() + "\\" + key.GetValue("Driver").ToString();

                    //String sInstance = Instance.ToString();

                    if (id != dd.DeviceID)
                        continue;

                    GetEdid(GetKeyFromPath(GetHKeyName(hEDIDRegKey)));
                    RegCloseKey(hEDIDRegKey);
                }
            }
            SetupDiDestroyDeviceInfoList(devInfo);
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

        public int ProductCode
        {
            get { return _rawData[10] + (_rawData[11] << 8); }
        }

        public int Serial
        {
            get { return _rawData[12] + (_rawData[13] << 8) + (_rawData[14] << 16) + (_rawData[15] << 24); }
        }

        private IntPtr GetHMonitor(String deviceName)
        {
            IntPtr h = IntPtr.Zero;

            EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref Rect lprcMonitor, IntPtr dwData)
                {
                    MonitorInfoEx mi = new MonitorInfoEx();
                    mi.Size = Marshal.SizeOf(mi);
                    bool success = GetMonitorInfo(hMonitor, ref mi);
                    if (success && mi.DeviceName == deviceName)
                    {
                        h = hMonitor;
                        return true;
                    }
                    return false;
                }, IntPtr.Zero);
            return h;
        }
        DISPLAY_DEVICE DisplayDeviceFromHMonitor(IntPtr hMonitor)
        {
            MonitorInfoEx mi = new MonitorInfoEx();
            mi.Init();
            GetMonitorInfo(hMonitor, ref mi);

            return DisplayDeviceFromID(mi.DeviceName);
        }

        DISPLAY_DEVICE DisplayDeviceFromID(String id)
        {
                DISPLAY_DEVICE ddMon = new DISPLAY_DEVICE();
                ddMon.cb = Marshal.SizeOf(ddMon);
                uint MonIdx = 0;

                while (EnumDisplayDevices(id, MonIdx, ref ddMon, 0))
                {
                    MonIdx++;
                    return ddMon; // TODO : we postulate that there is only 1 monitor per display
                }
                return ddMon;
        }
    }
}



/*
# include <atlstr.h>
# include <SetupApi.h>
#pragma comment(lib, "setupapi.lib")

#define NAME_SIZE 128

const GUID GUID_CLASS_MONITOR = { 0x4d36e96e, 0xe325, 0x11ce, 0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18 };

// Assumes hDevRegKey is valid
bool GetMonitorSizeFromEDID(const HKEY hDevRegKey, short& WidthMm, short& HeightMm)
{
    DWORD dwType, AcutalValueNameLength = NAME_SIZE;
    TCHAR valueName[NAME_SIZE];

    BYTE EDIDdata[1024];
    DWORD edidsize = sizeof(EDIDdata);

    for (LONG i = 0, retValue = ERROR_SUCCESS; retValue != ERROR_NO_MORE_ITEMS; ++i)
    {
        retValue = RegEnumValue(hDevRegKey, i, &valueName[0],
            &AcutalValueNameLength, NULL, &dwType,
            EDIDdata, // buffer
            &edidsize); // buffer size

        if (retValue != ERROR_SUCCESS || 0 != _tcscmp(valueName, _T("EDID")))
            continue;

        WidthMm = ((EDIDdata[68] & 0xF0) << 4) + EDIDdata[66];
        HeightMm = ((EDIDdata[68] & 0x0F) << 8) + EDIDdata[67];

        return true; // valid EDID found
    }

    return false; // EDID not found
}

bool GetSizeForDevID(const CString& TargetDevID, short& WidthMm, short& HeightMm)
{
    HDEVINFO devInfo = SetupDiGetClassDevsEx(
        &GUID_CLASS_MONITOR, //class GUID
        NULL, //enumerator
        NULL, //HWND
        DIGCF_PRESENT, // Flags //DIGCF_ALLCLASSES|
        NULL, // device info, create a new one.
        NULL, // machine name, local machine
        NULL);// reserved

    if (NULL == devInfo)
        return false;

    bool bRes = false;

    for (ULONG i = 0; ERROR_NO_MORE_ITEMS != GetLastError(); ++i)
    {
        SP_DEVINFO_DATA devInfoData;
        memset(&devInfoData, 0, sizeof(devInfoData));
        devInfoData.cbSize = sizeof(devInfoData);

        if (SetupDiEnumDeviceInfo(devInfo, i, &devInfoData))
        {
            HKEY hDevRegKey = SetupDiOpenDevRegKey(devInfo, &devInfoData,
                DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ);

            if (!hDevRegKey || (hDevRegKey == INVALID_HANDLE_VALUE))
                continue;

            bRes = GetMonitorSizeFromEDID(hDevRegKey, WidthMm, HeightMm);

            RegCloseKey(hDevRegKey);
        }
    }
    SetupDiDestroyDeviceInfoList(devInfo);
    return bRes;
}

int _tmain(int argc, _TCHAR* argv[])
{
    short WidthMm, HeightMm;

    DISPLAY_DEVICE dd;
    dd.cb = sizeof(dd);
    DWORD dev = 0; // device index
    int id = 1; // monitor number, as used by Display Properties > Settings

    CString DeviceID;
    bool bFoundDevice = false;
    while (EnumDisplayDevices(0, dev, &dd, 0) && !bFoundDevice)
    {
        DISPLAY_DEVICE ddMon;
        ZeroMemory(&ddMon, sizeof(ddMon));
        ddMon.cb = sizeof(ddMon);
        DWORD devMon = 0;

        while (EnumDisplayDevices(dd.DeviceName, devMon, &ddMon, 0) && !bFoundDevice)
        {
            if (ddMon.StateFlags & DISPLAY_DEVICE_ACTIVE &&
                !(ddMon.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER))
            {
                DeviceID.Format(L"%s", ddMon.DeviceID);
                DeviceID = DeviceID.Mid(8, DeviceID.Find(L"\\", 9) - 8);

                bFoundDevice = GetSizeForDevID(DeviceID, WidthMm, HeightMm);
            }
            devMon++;

            ZeroMemory(&ddMon, sizeof(ddMon));
            ddMon.cb = sizeof(ddMon);
        }

        ZeroMemory(&dd, sizeof(dd));
        dd.cb = sizeof(dd);
        dev++;
    }

    return 0;
}
*/
