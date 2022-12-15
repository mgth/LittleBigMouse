/*
  HLab.Windows.MonitorVcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;

namespace HLab.Sys.Windows.MonitorVcp
{
    using H = H<VcpControl>;

    public enum VcpComponent { None, Red, Green, Blue, Brightness, Contrast }

    public class VcpControl : NotifierBase
    {
        private Dxva2.PHYSICAL_MONITOR[] _pPhysicalMonitorArray;

        public MonitorDevice Monitor { get; }
        private readonly LevelParser _levelParser;
        public VcpControl(MonitorDevice monitor, LevelParser levelParser)
        {
            Monitor = monitor;
            _levelParser = levelParser;

            _pPhysicalMonitorArray = Dxva2.GetPhysicalMonitorsFromHMONITOR(monitor.HMonitor);

            H.Initialize(this);

            int retry = 1;
            while (retry > 0)
            {
                if (Dxva2.GetMonitorCapabilities(HPhysical, out var capabilities, out var colorTemperatures))
                {
                    if (capabilities.HasFlag(Dxva2.MonitorCapabilities.MC_CAPS_BRIGHTNESS))
                        _brightness.Set(new MonitorLevel(monitor, levelParser, GetBrightness, SetBrightness));

                    if (capabilities.HasFlag(Dxva2.MonitorCapabilities.MC_CAPS_CONTRAST))
                        _contrast.Set(new MonitorLevel(monitor, levelParser, GetContrast, SetContrast));

                    if (capabilities.HasFlag(Dxva2.MonitorCapabilities.MC_CAPS_RED_GREEN_BLUE_GAIN))
                        _gain.Set(new MonitorRgbLevel(monitor, levelParser, GetGain, SetGain));

                    if (capabilities.HasFlag(Dxva2.MonitorCapabilities.MC_CAPS_RED_GREEN_BLUE_DRIVE))
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
                Dxva2.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
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

        public bool Power
        {
            get => _power.Get();
            set
            {
                if (!_power.Set(value)) return;

                if (value)
                {
                    if(AlternatePower)
                        Dxva2.SetVCPFeature(HPhysical, 0xE1, 0);
                    else
                        Dxva2.SetVCPFeature(HPhysical, 0xD6, 1);

                }
                else
                {
                    if(AlternatePower)
                        Dxva2.SetVCPFeature(HPhysical, 0xE1, 1);
                    else
                        Dxva2.SetVCPFeature(HPhysical, 0xD6, 4);
                }
            }
        }
        private readonly IProperty<bool> _power = H.Property<bool>();

        public void SetSource(uint source)
        {
            Dxva2.GetVCPFeatureAndVCPFeatureReply(HPhysical, 0x60, out var pvct, out var current, out var max);


            Dxva2.GetCapabilitiesString(HPhysical, out var result);



            Dxva2.SetVCPFeature(HPhysical, 0x60, 0x0F);
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
            var result = Dxva2.GetMonitorBrightness(HPhysical, ref min, ref value, ref max);
            if (result) return true;
            var r = Kernel32.GetLastError();
            return false;
        }

        private bool SetBrightness(uint value, uint component = 0) 
            => Dxva2.SetMonitorBrightness(HPhysical, value);

        private bool GetContrast(ref uint min, ref uint value, ref uint max, uint component = 0) 
            => Dxva2.GetMonitorContrast(HPhysical, ref min, ref value, ref max);

        private bool SetContrast(uint value, uint component = 0) 
            => Dxva2.SetMonitorContrast(HPhysical, value);

        private bool GetGain(ref uint min, ref uint value, ref uint max, uint component) 
            => Dxva2.GetMonitorRedGreenOrBlueGain(HPhysical, component, ref min, ref value, ref max);

        private bool SetGain(uint value, uint component) 
            => Dxva2.SetMonitorRedGreenOrBlueGain(HPhysical, component, value);

        private bool GetDrive(ref uint min, ref uint value, ref uint max, uint component) 
            => Dxva2.GetMonitorRedGreenOrBlueDrive(HPhysical, component, ref min, ref value, ref max);

        private bool SetDrive(uint component, uint value) 
            => Dxva2.SetMonitorRedGreenOrBlueDrive(HPhysical, component, value);
    }

    public delegate bool VcpGetter(ref uint min, ref uint value, ref uint max, uint component = 0);
    public delegate bool VcpSetter(uint value, uint component = 0);
}
