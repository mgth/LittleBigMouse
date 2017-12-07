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
using System.Text;
// ReSharper disable InconsistentNaming
// ReSharper disable CheckNamespace

namespace WinAPI
{
    [System.Security.SuppressUnmanagedCodeSecurity]
    public static partial class NativeMethods
    {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        public struct PHYSICAL_MONITOR
        {
            public IntPtr hPhysicalMonitor;

            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
            public string szPhysicalMonitorDescription;
        }

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
