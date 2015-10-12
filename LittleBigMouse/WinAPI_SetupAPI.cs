using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;

namespace WinAPI_SetupAPI
{

    [StructLayout(LayoutKind.Sequential)]
    internal struct SP_DEVINFO_DATA
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

    class SetupAPI
    {
        internal static Guid GUID_CLASS_MONITOR = new Guid(0x4d36e96e, 0xe325, 0x11ce, 0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18);
        const int MAX_DEVICE_ID_LEN = 200;
        const int MAX_PATH = 260;

        internal const int DIGCF_PRESENT = 0x2;
        internal const int DIGCF_PROFILE = 0x8;
        internal const int DICS_FLAG_GLOBAL = 0x1;
        internal const int DIREG_DEV = 0x1;

        internal const int KEY_READ = 0x20019;

        private const Int32 NAME_SIZE = 128;
        private const UInt32 ERROR_SUCCESS = 0;

        [DllImport("setupapi.dll")]
        internal static extern IntPtr SetupDiGetClassDevsEx(ref Guid ClassGuid,
            [MarshalAs(UnmanagedType.LPStr)]String enumerator,
            IntPtr hwndParent, Int32 Flags, IntPtr DeviceInfoSet,
            [MarshalAs(UnmanagedType.LPStr)]String MachineName, IntPtr Reserved);

        [DllImport("setupapi.dll", SetLastError = true)]
        internal static extern bool SetupDiEnumDeviceInfo(IntPtr DeviceInfoSet, uint MemberIndex, ref SP_DEVINFO_DATA DeviceInfoData);

        [DllImport("setupapi.dll", SetLastError = true)]
        internal static extern bool SetupDiDestroyDeviceInfoList
        (
             IntPtr DeviceInfoSet
        );

        [DllImport("Setupapi", CharSet = CharSet.Auto, SetLastError = true)]
        internal static extern IntPtr SetupDiOpenDevRegKey(
            IntPtr hDeviceInfoSet,
            ref SP_DEVINFO_DATA deviceInfoData,
            int scope,
            int hwProfile,
            int parameterRegistryValueKind,
            int samDesired);
    }
}
