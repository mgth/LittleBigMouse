using System;
using System.Runtime.InteropServices;
using System.Text;

namespace WinAPI_Dxva2
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    public struct PHYSICAL_MONITOR
    {
        public IntPtr hPhysicalMonitor;

        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
        public string szPhysicalMonitorDescription;
    }

    public static class Dxva2
    {
        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetNumberOfPhysicalMonitorsFromHMONITOR(IntPtr hMonitor, ref uint pdwNumberOfPhysicalMonitors);

        [DllImport("dxva2.dll", EntryPoint = "DestroyPhysicalMonitors")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool DestroyPhysicalMonitors(
            uint dwPhysicalMonitorArraySize, ref PHYSICAL_MONITOR[] pPhysicalMonitorArray);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetPhysicalMonitorsFromHMONITOR(IntPtr hMonitor, uint dwPhysicalMonitorArraySize, [Out] PHYSICAL_MONITOR[] pPhysicalMonitorArray);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetMonitorBrightness(IntPtr hMonitor, ref uint pdwMinimumBrightness, ref uint pdwCurrentBrightness, ref uint pdwMaximumBrightness);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool SetMonitorBrightness(IntPtr hMonitor, uint dwNewBrightness);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetMonitorContrast(IntPtr hMonitor, ref uint pdwMinimumContrast, ref uint pdwCurrentContrast, ref uint pdwMaximumContrast);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool SetMonitorContrast(IntPtr hMonitor, uint dwNewContrast);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetMonitorRedGreenOrBlueGain(IntPtr hMonitor, uint component, ref uint pdwMinimumContrast, ref uint pdwCurrentContrast, ref uint pdwMaximumContrast);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool SetMonitorRedGreenOrBlueGain(IntPtr hMonitor, uint component, uint dwNewContrast);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetMonitorRedGreenOrBlueDrive(IntPtr hMonitor, uint component, ref uint pdwMinimumContrast, ref uint pdwCurrentContrast, ref uint pdwMaximumContrast);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool SetMonitorRedGreenOrBlueDrive(IntPtr hMonitor, uint component, uint dwNewContrast);



        [DllImport("dxva2.dll", EntryPoint = "GetCapabilitiesStringLength", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetCapabilitiesStringLength(
            [In] IntPtr hMonitor, ref uint pdwLength);

        [DllImport("dxva2.dll", EntryPoint = "GetCapabilitiesString", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetCapabilitiesString(
            [In] IntPtr hMonitor, StringBuilder pszString, uint dwLength);

        [DllImport("dxva2.dll", EntryPoint = "GetVCPFeatureAndVCPFeatureReply", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetVCPFeatureAndVCPFeatureReply(
            [In] IntPtr hMonitor, [In] uint dwVCPCode, out uint pvct, out uint pdwCurrentValue, out uint pdwMaximumValue);


        [DllImport("dxva2.dll", EntryPoint = "SetVCPFeature", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool SetVCPFeature(
            [In] IntPtr hMonitor, uint dwVCPCode, uint dwNewValue);
    }
}
