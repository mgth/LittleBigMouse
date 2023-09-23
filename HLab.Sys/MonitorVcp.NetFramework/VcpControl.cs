/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.MonitorVcp.

    HLab.Windows.MonitorVcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.MonitorVcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Runtime.CompilerServices;
using System.Windows;
using HLab.Notify.PropertyChanged;
using HLab.Windows.API;
using HLab.Windows.Monitors;

namespace HLab.Windows.MonitorVcp
{
    public static class VcpExpendMonitor
    {
        private static readonly LevelParser LevelParser = new LevelParser(); 

        private static readonly ConditionalWeakTable<Monitor, VcpControl> AllVcp = new ConditionalWeakTable<Monitor, VcpControl>();
        public static VcpControl Vcp(this Monitor monitor) => AllVcp.GetValue(monitor, m => new VcpControl(monitor, LevelParser));
    }
    public enum VcpComponent { None, Red, Green, Blue, Brightness, Contrast }

    public class VcpControl : N<VcpControl>
    {
        private NativeMethods.PHYSICAL_MONITOR[] _pPhysicalMonitorArray;

        public Monitor Monitor { get; }
        private LevelParser _levelParser;
        public VcpControl(Monitor monitor, LevelParser levelParser)
        {
            Monitor = monitor;
            _levelParser = levelParser;

            _pPhysicalMonitorArray = NativeMethods.GetPhysicalMonitorsFromHMONITOR(monitor.HMonitor);
            if(_pPhysicalMonitorArray==null)
            {

            }

            Initialize();

            int retry = 1;
            while (retry > 0)
            {
                if (NativeMethods.GetMonitorCapabilities(HPhysical, out var capabilities, out var colorTemperatures))
                {
                    if (capabilities.HasFlag(NativeMethods.MonitorCapabilities.MC_CAPS_BRIGHTNESS))
                        _brightness.Set(new MonitorLevel(monitor, levelParser, GetBrightness, SetBrightness));

                    if (capabilities.HasFlag(NativeMethods.MonitorCapabilities.MC_CAPS_CONTRAST))
                        _contrast.Set(new MonitorLevel(monitor, levelParser, GetContrast, SetContrast));

                    if (capabilities.HasFlag(NativeMethods.MonitorCapabilities.MC_CAPS_RED_GREEN_BLUE_GAIN))
                        _gain.Set(new MonitorRgbLevel(monitor, levelParser, GetGain, SetGain));

                    if (capabilities.HasFlag(NativeMethods.MonitorCapabilities.MC_CAPS_RED_GREEN_BLUE_DRIVE))
                        _drive.Set(new MonitorRgbLevel(monitor, levelParser, GetDrive, SetDrive));

                    //NativeMethods.MonitorCapabilities.MC_CAPS_COLOR_TEMPERATURE;
                    //NativeMethods.MonitorCapabilities.MC_CAPS_DEGAUSS;
                    //NativeMethods.MonitorCapabilities.MC_CAPS_DISPLAY_AREA_POSITION;
                    //NativeMethods.MonitorCapabilities.MC_CAPS_DISPLAY_AREA_SIZE;
                    //NativeMethods.MonitorCapabilities.MC_CAPS_MONITOR_TECHNOLOGY_TYPE;
                    //NativeMethods.MonitorCapabilities.MC_CAPS_RESTORE_FACTORY_COLOR_DEFAULTS;
                    //NativeMethods.MonitorCapabilities.MC_CAPS_RESTORE_FACTORY_DEFAULTS;
                    //NativeMethods.MonitorCapabilities.MC_RESTORE_FACTORY_DEFAULTS_ENABLES_MONITOR_SETTINGS;
                    retry = 0;
                }
                else retry--;
            }

            
        }


        private IntPtr HPhysical => _pPhysicalMonitorArray[0].hPhysicalMonitor;

        private void Cleanup(bool disposing)
        {
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                NativeMethods.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }

        public void Dispose()
        {
            Cleanup(true);
            GC.SuppressFinalize(this);
        }
        ~VcpControl()
        {
            Cleanup(false);
        }

        public bool AlternatePower => Monitor.Edid?.ManufacturerCode == "DEL";

        private readonly IProperty<bool> _power = H.Property<bool>(nameof(Power));
        public bool Power
        {
            get => _power.Get();
            set
            {
                if (!_power.Set(value)) return;

                if (value)
                {
                    if(AlternatePower)
                        NativeMethods.SetVCPFeature(HPhysical, 0xE1, 0);
                    else
                        NativeMethods.SetVCPFeature(HPhysical, 0xD6, 1);

                }
                else
                {
                    if(AlternatePower)
                        NativeMethods.SetVCPFeature(HPhysical, 0xE1, 1);
                    else
                        NativeMethods.SetVCPFeature(HPhysical, 0xD6, 4);
                }
            }
        }

        public void ActivateAnyway()
        {
                if(Brightness==null) _brightness.Set(new MonitorLevel(Monitor, _levelParser, GetBrightness, SetBrightness));
                if(Contrast==null) _contrast.Set(new MonitorLevel(Monitor, _levelParser, GetContrast, SetContrast));
                if(Gain==null) _gain.Set(new MonitorRgbLevel(Monitor, _levelParser, GetGain, SetGain));
                if(Drive==null) _drive.Set(new MonitorRgbLevel(Monitor, _levelParser, GetDrive, SetDrive));
        }

        public MonitorLevel Brightness => _brightness.Get();
        private readonly IProperty<MonitorLevel> _brightness = H.Property<MonitorLevel>();
        public MonitorLevel Contrast => _contrast.Get();
        private readonly IProperty<MonitorLevel> _contrast = H.Property<MonitorLevel>();


        public MonitorRgbLevel Gain => _gain.Get();
        private readonly IProperty<MonitorRgbLevel> _gain = H.Property<MonitorRgbLevel>();
        public MonitorRgbLevel Drive => _drive.Get();
        private readonly IProperty<MonitorRgbLevel> _drive = H.Property<MonitorRgbLevel>();

        private bool GetBrightness(ref uint min, ref uint value, ref uint max, uint component = 0)
        {
            var result = NativeMethods.GetMonitorBrightness(HPhysical, ref min, ref value, ref max);
            if (result) return true;
            var r = NativeMethods.GetLastError();
            return false;
        }

        private bool SetBrightness(uint value, uint component = 0) 
            => NativeMethods.SetMonitorBrightness(HPhysical, value);

        private bool GetContrast(ref uint min, ref uint value, ref uint max, uint component = 0) 
            => NativeMethods.GetMonitorContrast(HPhysical, ref min, ref value, ref max);

        private bool SetContrast(uint value, uint component = 0) 
            => NativeMethods.SetMonitorContrast(HPhysical, value);

        private bool GetGain(ref uint min, ref uint value, ref uint max, uint component) 
            => NativeMethods.GetMonitorRedGreenOrBlueGain(HPhysical, component, ref min, ref value, ref max);

        private bool SetGain(uint value, uint component) 
            => NativeMethods.SetMonitorRedGreenOrBlueGain(HPhysical, component, value);

        private bool GetDrive(ref uint min, ref uint value, ref uint max, uint component) 
            => NativeMethods.GetMonitorRedGreenOrBlueDrive(HPhysical, component, ref min, ref value, ref max);

        private bool SetDrive(uint component, uint value) 
            => NativeMethods.SetMonitorRedGreenOrBlueDrive(HPhysical, component, value);
    }

    public delegate bool VcpGetter(ref uint min, ref uint value, ref uint max, uint component = 0);
    public delegate bool VcpSetter(uint value, uint component = 0);
}
