/*
  HLab.Windows.API
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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

namespace HLab.Sys.Windows.API
{
    public static partial class NativeMethods
    {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
        public struct PHYSICAL_MONITOR
        {
            public IntPtr hPhysicalMonitor;

            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
            public string szPhysicalMonitorDescription;
        }

        [Flags]
        public enum MonitorCapabilities : uint
        {
            MC_CAPS_NONE = 0x00000000,
            MC_CAPS_MONITOR_TECHNOLOGY_TYPE = 0x00000001,
            MC_CAPS_BRIGHTNESS = 0x00000002,
            MC_CAPS_CONTRAST = 0x00000004,
            MC_CAPS_COLOR_TEMPERATURE = 0x00000008,
            MC_CAPS_RED_GREEN_BLUE_GAIN = 0x00000010,
            MC_CAPS_RED_GREEN_BLUE_DRIVE = 0x00000020,
            MC_CAPS_DEGAUSS = 0x00000040,
            MC_CAPS_DISPLAY_AREA_POSITION = 0x00000080,
            MC_CAPS_DISPLAY_AREA_SIZE = 0x00000100,
            MC_CAPS_RESTORE_FACTORY_DEFAULTS = 0x00000400,
            MC_CAPS_RESTORE_FACTORY_COLOR_DEFAULTS = 0x00000800,
            MC_RESTORE_FACTORY_DEFAULTS_ENABLES_MONITOR_SETTINGS = 0x00001000,
        }

        [Flags]
        public enum MonitorSupportedColorTemperatures : uint
        {
            MC_SUPPORTED_COLOR_TEMPERATURE_NONE = 0x00000000,
            MC_SUPPORTED_COLOR_TEMPERATURE_4000K = 0x00000001,
            MC_SUPPORTED_COLOR_TEMPERATURE_5000K = 0x00000002,
            MC_SUPPORTED_COLOR_TEMPERATURE_6500K = 0x00000004,
            MC_SUPPORTED_COLOR_TEMPERATURE_7500K = 0x00000008,
            MC_SUPPORTED_COLOR_TEMPERATURE_8200K = 0x00000010,
            MC_SUPPORTED_COLOR_TEMPERATURE_9300K = 0x00000020,
            MC_SUPPORTED_COLOR_TEMPERATURE_10000K = 0x00000040,
            MC_SUPPORTED_COLOR_TEMPERATURE_11500K = 0x00000080,
        }

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        private static extern bool GetNumberOfPhysicalMonitorsFromHMONITOR(IntPtr hMonitor, ref uint pdwNumberOfPhysicalMonitors);

        [DllImport("dxva2.dll", EntryPoint = "DestroyPhysicalMonitors")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool DestroyPhysicalMonitors(
            uint dwPhysicalMonitorArraySize, ref PHYSICAL_MONITOR[] pPhysicalMonitorArray);

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        private static extern bool GetPhysicalMonitorsFromHMONITOR(IntPtr hMonitor, uint dwPhysicalMonitorArraySize, [Out] PHYSICAL_MONITOR[] pPhysicalMonitorArray);

        public static PHYSICAL_MONITOR[] GetPhysicalMonitorsFromHMONITOR(IntPtr hMonitor)
        {
            uint pdwNumberOfPhysicalMonitors = 0;

            if (NativeMethods.GetNumberOfPhysicalMonitorsFromHMONITOR(hMonitor, ref pdwNumberOfPhysicalMonitors))
            {
                var pPhysicalMonitorArray = new NativeMethods.PHYSICAL_MONITOR[pdwNumberOfPhysicalMonitors];

                if (NativeMethods.GetPhysicalMonitorsFromHMONITOR(hMonitor, pdwNumberOfPhysicalMonitors, pPhysicalMonitorArray))
                {
                    return pPhysicalMonitorArray;
                }
            }

            return null;
        }


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

        [DllImport("Dxva2.dll", CharSet = CharSet.Auto)]
        public static extern bool GetMonitorCapabilities(
            IntPtr hMonitor,
            [Out] out MonitorCapabilities monitorCapabilities,
            [Out] out MonitorSupportedColorTemperatures supportedColorTemperatures);


        [DllImport("dxva2.dll", EntryPoint = "GetCapabilitiesStringLength", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetCapabilitiesStringLength(
            [In] IntPtr hMonitor, ref uint pdwLength);

        [DllImport("dxva2.dll", EntryPoint = "CapabilitiesRequestAndCapabilitiesReply", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool CapabilitiesRequestAndCapabilitiesReply(
            [In] IntPtr hMonitor, StringBuilder pszString, uint dwLength);

        public static bool GetCapabilitiesString(IntPtr hMonitor, out string capabilities)
        {
            uint length = 0;
            if(GetCapabilitiesStringLength(hMonitor, ref length))
            {
                var sb = new StringBuilder((int)length);
                if(CapabilitiesRequestAndCapabilitiesReply(hMonitor, sb, length))
                {
                    capabilities = sb.ToString();
                    return true;
                }
            }
            capabilities = string.Empty;
            return false;
        }


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
