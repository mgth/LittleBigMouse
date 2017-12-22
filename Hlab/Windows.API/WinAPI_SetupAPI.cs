/*
  HLab.Windows.API
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.API.

    HLab.Windows.API is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.API is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Runtime.InteropServices;
using System.Security;

namespace HLab.Windows.API
{
    [SuppressUnmanagedCodeSecurity]
    public static partial class NativeMethods
    {

        [StructLayout(LayoutKind.Sequential)]
        public struct SP_DEVINFO_DATA
    {
        public uint cbSize;
        public Guid classGuid;
        public uint devInst;
        public IntPtr reserved;

        public SP_DEVINFO_DATA(bool init)
        {
            cbSize = (uint)Marshal.SizeOf(typeof(SP_DEVINFO_DATA));
            classGuid = Guid.Empty;
            devInst = 0;
            reserved = IntPtr.Zero;
        }
    }

        public static Guid GUID_CLASS_MONITOR = new Guid(0x4d36e96e, 0xe325, 0x11ce, 0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18);
        const int MAX_DEVICE_ID_LEN = 200;
        const int MAX_PATH = 260;

        public const int DIGCF_PRESENT = 0x2;
        public const int DIGCF_PROFILE = 0x8;
        public const int DICS_FLAG_GLOBAL = 0x1;
        public const int DIREG_DEV = 0x1;

        public const int KEY_READ = 0x20019;

        private const Int32 NAME_SIZE = 128;
        private const UInt32 ERROR_SUCCESS = 0;

        [DllImport("setupapi.dll")]
        public static extern IntPtr SetupDiGetClassDevsEx(ref Guid ClassGuid,
            [MarshalAs(UnmanagedType.LPStr)]String enumerator,
            IntPtr hwndParent, Int32 Flags, IntPtr DeviceInfoSet,
            [MarshalAs(UnmanagedType.LPStr)]String MachineName, IntPtr Reserved);

        [DllImport("setupapi.dll", SetLastError = true)]
        public static extern bool SetupDiEnumDeviceInfo(IntPtr DeviceInfoSet, uint MemberIndex, ref SP_DEVINFO_DATA DeviceInfoData);

        [DllImport("setupapi.dll", SetLastError = true)]
        public static extern bool SetupDiDestroyDeviceInfoList
        (
             IntPtr DeviceInfoSet
        );

        [DllImport("Setupapi", CharSet = CharSet.Auto, SetLastError = true)]
        public static extern IntPtr SetupDiOpenDevRegKey(
            IntPtr hDeviceInfoSet,
            ref SP_DEVINFO_DATA deviceInfoData,
            int scope,
            int hwProfile,
            int parameterRegistryValueKind,
            int samDesired);
    }
}
