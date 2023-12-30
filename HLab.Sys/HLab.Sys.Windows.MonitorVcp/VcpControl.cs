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
using HLab.Sys.Windows.API.MonitorConfiguration;
using ReactiveUI;

using HLab.Sys.Windows.Monitors;
using OneOf;

using static HLab.Sys.Windows.API.ErrHandlingApi;
using static HLab.Sys.Windows.API.MonitorConfiguration.HighLevelMonitorConfigurationApi;
using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;
using static HLab.Sys.Windows.API.MonitorConfiguration.PhysicalMonitorEnumerationApi;

namespace HLab.Sys.Windows.MonitorVcp;

public enum VcpComponent
{
    None = -1, 
    Red = 0, 
    Green = 1, 
    Blue = 2, 
    Brightness = -2, 
    Contrast = -3
}

public class VcpControl : ReactiveObject
{
    PhysicalMonitor[] _pPhysicalMonitorArray;

    public MonitorDevice Monitor { get; }
    readonly LevelParser _levelParser;
    public VcpControl(MonitorDevice monitor, LevelParser levelParser)
    {
        Monitor = monitor;
        _levelParser = levelParser;

        _pPhysicalMonitorArray = GetPhysicalMonitorsFromHMONITOR(monitor.Connections[0].Parent.HMonitor);

        var retry = 1;
        while (retry > 0)
        {
            if (GetMonitorCapabilities(HPhysical, out var capabilities, out var colorTemperatures))
            {
                if (capabilities.HasFlag(HighLevelMonitorConfigurationApi.MonitorCapabilities.Brightness))
                    _brightness = new MonitorLevel(levelParser, GetBrightness, SetBrightness);

                if (capabilities.HasFlag(HighLevelMonitorConfigurationApi.MonitorCapabilities.Contrast))
                    _contrast = new MonitorLevel(levelParser, GetContrast, SetContrast);

                if (capabilities.HasFlag(HighLevelMonitorConfigurationApi.MonitorCapabilities.RedGreenBlueGain))
                    _gain = new MonitorRgbLevel(levelParser, GetGain, SetGain);

                if (capabilities.HasFlag(HighLevelMonitorConfigurationApi.MonitorCapabilities.RedGreenBlueDrive))
                    _drive = new MonitorRgbLevel(levelParser, GetDrive, SetDrive);

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

    public VcpControl Start()
    {
        _brightness?.Start();
        _contrast?.Start();
        _gain?.Start();
        _drive?.Start();
        return this;
    }


    nint HPhysical => _pPhysicalMonitorArray[0].hPhysicalMonitor;

    ~VcpControl()
    {
        if (_pPhysicalMonitorArray is { Length: > 0 })
            DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
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
        Brightness ??= new MonitorLevel(_levelParser, GetBrightness, SetBrightness).Start();
        Contrast ??= new MonitorLevel(_levelParser, GetContrast, SetContrast).Start();
        Gain ??= new MonitorRgbLevel(_levelParser, GetGain, SetGain).Start();

        // TODO : Drive seams to never work when not officially supported
        // Drive ??= new MonitorRgbLevel(_levelParser, GetDrive, SetDrive).Start();
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

    OneOf<(uint value, uint min, uint max),int> GetBrightness(VcpComponent component = VcpComponent.None)
    {
        uint value = 0, min = 0, max = 0;
        var result = GetMonitorBrightness(HPhysical, ref min, ref value, ref max);
        if (result) return (value,min,max);
        return GetLastError();
    }

    bool SetBrightness(uint value, VcpComponent component = VcpComponent.None) 
        => SetMonitorBrightness(HPhysical, value);

    OneOf<(uint value, uint min, uint max),int> GetContrast(VcpComponent component = VcpComponent.None)
    {
        uint value = 0, min = 0, max = 0;
        if (GetMonitorContrast(HPhysical, ref min, ref value, ref max))
        {
            return (value,min,max);
        };
        return GetLastError();
    }

    bool SetContrast(uint value, VcpComponent component = VcpComponent.None) 
        => SetMonitorContrast(HPhysical, value);

    OneOf<(uint value, uint min, uint max),int> GetGain(VcpComponent component)
    {
        uint value = 0, min = 0, max = 0;
        if (GetMonitorRedGreenOrBlueGain(HPhysical, (uint)component, ref min, ref value, ref max))
        {
            return (value,min,max);
        }
        return GetLastError();
    }

    bool SetGain(uint value, VcpComponent component = VcpComponent.None) 
        => SetMonitorRedGreenOrBlueGain(HPhysical, (uint)component, value);

    OneOf<(uint value, uint min, uint max),int> GetDrive(VcpComponent component)
    {
        uint value = 0, min = 0, max = 0;
        if (GetMonitorRedGreenOrBlueDrive(HPhysical, (uint)component, ref min, ref value, ref max))
        {
            return (value,min,max);
        }
        return GetLastError();
    }

    bool SetDrive(uint value, VcpComponent component) 
        => SetMonitorRedGreenOrBlueDrive(HPhysical, (uint)component, value);
}

public delegate OneOf<(uint value, uint min, uint max),int> VcpGetter(VcpComponent component = VcpComponent.None);
public delegate bool VcpSetter(uint value, VcpComponent component = VcpComponent.None);