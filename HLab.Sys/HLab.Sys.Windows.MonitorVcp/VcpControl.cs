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
using ReactiveUI;

using HLab.Sys.Windows.Monitors;

using static HLab.Sys.Windows.API.ErrHandlingApi;
using static HLab.Sys.Windows.API.MonitorConfiguration.HighLevelMonitorConfigurationApi;
using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;
using static HLab.Sys.Windows.API.MonitorConfiguration.PhysicalMonitorEnumerationApi;


namespace HLab.Sys.Windows.MonitorVcp
{
    public enum VcpComponent { None, Red, Green, Blue, Brightness, Contrast }

    public class VcpControl : ReactiveObject
    {
        PhysicalMonitor[] _pPhysicalMonitorArray;

        public MonitorDevice Monitor { get; }
        readonly LevelParser _levelParser;
        public VcpControl(MonitorDevice monitor, LevelParser levelParser)
        {
            Monitor = monitor;
            _levelParser = levelParser;

            // TODO
            //_pPhysicalMonitorArray = GetPhysicalMonitorsFromHMONITOR(monitor.HMonitor);

            var retry = 1;
            while (retry > 0)
            {
                if (GetMonitorCapabilities(HPhysical, out var capabilities, out var colorTemperatures))
                {
                    if (capabilities.HasFlag(MonitorCapabilities.Brightness))
                        _brightness = new MonitorLevel(monitor, levelParser, GetBrightness, SetBrightness);

                    if (capabilities.HasFlag(MonitorCapabilities.Contrast))
                        _contrast = new MonitorLevel(monitor, levelParser, GetContrast, SetContrast);

                    if (capabilities.HasFlag(MonitorCapabilities.RedGreenBlueGain))
                        _gain = new MonitorRgbLevel(monitor, levelParser, GetGain, SetGain);

                    if (capabilities.HasFlag(MonitorCapabilities.RedGreenBlueDrive))
                        _drive = new MonitorRgbLevel(monitor, levelParser, GetDrive, SetDrive);

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


        nint HPhysical => _pPhysicalMonitorArray[0].hPhysicalMonitor;

        void Cleanup(bool disposing)
        {
            if (_pPhysicalMonitorArray is { Length: > 0 })
                DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
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
            get => _power;
            set
            {
                if (value == _power) return;
                if (value)
                {
                    if(AlternatePower)
                        SetVCPFeature(HPhysical, 0xE1, 0);
                    else
                        SetVCPFeature(HPhysical, 0xD6, 1);
                }
                else
                {
                    if(AlternatePower)
                        SetVCPFeature(HPhysical, 0xE1, 1);
                    else
                        SetVCPFeature(HPhysical, 0xD6, 4);
                }
                this.RaiseAndSetIfChanged(ref _power, value);

            }
        }
        bool _power;

        public void SetSource(uint source)
        {
            GetVCPFeatureAndVCPFeatureReply(HPhysical, 0x60, out var pvct, out var current, out var max);

            GetCapabilitiesString(HPhysical, out var result);

            SetVCPFeature(HPhysical, 0x60, 0x0F);
        }

        public void ActivateAnyway()
        {
                Brightness ??= new MonitorLevel(Monitor, _levelParser, GetBrightness, SetBrightness);
                Contrast ??= new MonitorLevel(Monitor, _levelParser, GetContrast, SetContrast);
                Gain ??= new MonitorRgbLevel(Monitor, _levelParser, GetGain, SetGain);
                Drive ??= new MonitorRgbLevel(Monitor, _levelParser, GetDrive, SetDrive);
        }

        public MonitorLevel Brightness
        {
            get => _brightness;
            private set => this.RaiseAndSetIfChanged(ref _brightness, value);
        }
        MonitorLevel _brightness;

        public MonitorLevel Contrast
        {
            get => _contrast;
            private set => this.RaiseAndSetIfChanged(ref _contrast, value);
        }
        MonitorLevel _contrast;

        public MonitorRgbLevel Gain
        {
            get => _gain;
            private set => this.RaiseAndSetIfChanged(ref _gain, value);
        }
        MonitorRgbLevel _gain;

        public MonitorRgbLevel Drive
        {
            get => _drive;
            private set => this.RaiseAndSetIfChanged(ref _drive, value);
        }
        MonitorRgbLevel _drive;

        bool GetBrightness(ref uint min, ref uint value, ref uint max, uint component = 0)
        {
            var result = GetMonitorBrightness(HPhysical, ref min, ref value, ref max);
            if (result) return true;
            var r = GetLastError();
            return false;
        }

        bool SetBrightness(uint value, uint component = 0) 
            => SetMonitorBrightness(HPhysical, value);

        bool GetContrast(ref uint min, ref uint value, ref uint max, uint component = 0) 
            => GetMonitorContrast(HPhysical, ref min, ref value, ref max);

        bool SetContrast(uint value, uint component = 0) 
            => SetMonitorContrast(HPhysical, value);

        bool GetGain(ref uint min, ref uint value, ref uint max, uint component) 
            => GetMonitorRedGreenOrBlueGain(HPhysical, component, ref min, ref value, ref max);

        bool SetGain(uint value, uint component) 
            => SetMonitorRedGreenOrBlueGain(HPhysical, component, value);

        bool GetDrive(ref uint min, ref uint value, ref uint max, uint component) 
            => GetMonitorRedGreenOrBlueDrive(HPhysical, component, ref min, ref value, ref max);

        bool SetDrive(uint component, uint value) 
            => SetMonitorRedGreenOrBlueDrive(HPhysical, component, value);
    }

    public delegate bool VcpGetter(ref uint min, ref uint value, ref uint max, uint component = 0);
    public delegate bool VcpSetter(uint value, uint component = 0);
}
