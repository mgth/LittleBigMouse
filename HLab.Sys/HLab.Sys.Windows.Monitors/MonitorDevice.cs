using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.Serialization;
using Avalonia;
using Avalonia.Controls;
using DynamicData;
using Microsoft.Win32;

namespace HLab.Sys.Windows.Monitors;

public class MonitorDevice {
    public nint HMonitor { get; set; }

    public string DeviceId { get; set; }

    public string IdPhysicalMonitor { get; set; }

    public string PnpCode { get; set; }

    public string IdMonitor { get; set; }

    public string DeviceKey { get; set; }

    public string DeviceString { get; set; }

    public IObservableCache<DisplayDevice, string> Devices { get; internal set; }

    public DisplayDevice AttachedDevice { get; set; }

    public DisplayDevice AttachedDisplay { get; set; }

    public Rect MonitorArea { get; set; }// MONITORINFOEX 

    public Rect WorkArea { get; set; }// MONITORINFOEX

    public bool AttachedToDesktop { get; set; }// EnumDisplayDevices

    public bool Primary { get; set; }// MONITORINFOEX

    public void SetPrimary(IEnumerable<MonitorDevice> monitors, bool value) {
        if (value) {
            // Must remove old primary screen before setting this one
            foreach (var monitor in monitors.Where(m => !m.Equals(this))) {
                monitor.Primary = false;
            }
        }

        Primary = value;
    }

    public IEdid Edid { get; internal set; }

    [DataMember] public Vector EffectiveDpi { get; set; }//GetDpiForMonitor

    [DataMember] public Vector AngularDpi { get; set; }//GetDpiForMonitor

    [DataMember] public Vector RawDpi { get; set; }//GetDpiForMonitor

    //https://msdn.microsoft.com/fr-fr/library/windows/desktop/dn302060.aspx
    [DataMember] public double ScaleFactor { get; set; }//GetScaleFactorForMonitor

    [DataMember] public string CapabilitiesString { get; set; }//DDCCIGetCapabilitiesString

    [DataMember] public int MonitorNumber { get; set; }

    [DataMember] public string WallpaperPath { get; set; }



    class EdidDesign : IEdid
    {
        public EdidDesign() 
        {        
            if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
        }

        public string HKeyName => "HKLM://";
        public string ManufacturerCode => "SAM";
        public string ProductCode { get; }
        public string Serial { get; }
        public int Week => 42;
        public int Year { get; }
        public string Version { get; }
        public bool Digital { get; }
        public int BitDepth { get; }
        public string VideoInterface { get; }
        public Size PhysicalSize => new Size(600, 340);
        public string Model => "S24D300";
        public string SerialNumber => "S/N: 123456789";
        public double Gamma => 2.2;
        public bool DpmsStandbySupported => true;
        public bool DpmsSuspendSupported => true;
        public bool DpmsActiveOffSupported => true;
        public bool YCrCb444Support => true;
        public bool YCrCb422Support => true;
        public double sRGB => 0.98;
        public double RedX => 0.64;
        public double RedY => 0.33;
        public double GreenX => 0.3;
        public double GreenY => 0.6;
        public double BlueX => 0.15;
        public double BlueY => 0.06;
        public double WhiteX => 0.3127;
        public double WhiteY => 0.3127; 
        public int Checksum => int.MinValue;
    }

    public static MonitorDevice MonitorDesign
    {
        get
        {
            if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");

            return new MonitorDevice
            {
                Edid = new EdidDesign()
            };
        }
    }

    string _hKeyName;

    //------------------------------------------------------------------------
    public override bool Equals(object obj) => obj is MonitorDevice other ? DeviceId == other.DeviceId : base.Equals(obj);


    public override int GetHashCode() {
        return ("DisplayMonitor" + DeviceId).GetHashCode();
    }

    void OpenRegKey(string keystring) {
        using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Applets\Regedit\Lastkey", true)) {
            //key;
        }
    }

    public string ConfigPath() => IdMonitor;

    public void DisplayValues(Action<string, string, Action, bool> addValue) {
        addValue("Registry", Edid.HKeyName, () => { OpenRegKey(Edid.HKeyName); }, false);
        addValue("Microsoft Id", IdMonitor, null, false);

        // EnumDisplaySettings
        addValue("", "EnumDisplaySettings", null, true);
        addValue("DisplayOrientation", AttachedDisplay?.CurrentMode?.DisplayOrientation.ToString(), null, false);
        addValue("Position", AttachedDisplay?.CurrentMode?.Position.ToString(), null, false);
        addValue("Pels", AttachedDisplay?.CurrentMode?.Pels.ToString(), null, false);
        addValue("BitsPerPixel", AttachedDisplay?.CurrentMode?.BitsPerPixel.ToString(), null, false);
        addValue("DisplayFrequency", AttachedDisplay?.CurrentMode?.DisplayFrequency.ToString(), null, false);
        addValue("DisplayFlags", AttachedDisplay?.CurrentMode?.DisplayFlags.ToString(), null, false);
        addValue("DisplayFixedOutput", AttachedDisplay?.CurrentMode?.DisplayFixedOutput.ToString(), null, false);

        // GetDeviceCaps
        addValue("", "GetDeviceCaps", null, true);
        addValue("Size", AttachedDisplay?.Capabilities.Size.ToString(), null, false);
        addValue("Res", AttachedDisplay?.Capabilities.Resolution.ToString(), null, false);
        addValue("LogPixels", AttachedDisplay?.Capabilities.LogPixels.ToString(), null, false);
        addValue("BitsPixel", AttachedDisplay?.Capabilities.BitsPixel.ToString(), null, false);
        //AddValue("Color Planes", Monitor.Adapter.DeviceCaps.Planes.ToString());
        addValue("Aspect", AttachedDisplay?.Capabilities.Aspect.ToString(), null, false);
        //AddValue("BltAlignment", Monitor.Adapter.DeviceCaps.BltAlignment.ToString());

        //GetDpiForMonitor
        addValue("", "GetDpiForMonitor", null, true);
        addValue("EffectiveDpi", EffectiveDpi.ToString(), null, false);
        addValue("AngularDpi", AngularDpi.ToString(), null, false);
        addValue("RawDpi", RawDpi.ToString(), null, false);

        // GetMonitorInfo
        addValue("", "GetMonitorInfo", null, true);
        addValue("Primary", Primary.ToString(), null, false);
        addValue("MonitorArea", MonitorArea.ToString(), null, false);
        addValue("WorkArea", WorkArea.ToString(), null, false);


        // EDID
        addValue("", "EDID", null, true);
        addValue("ManufacturerCode", Edid?.ManufacturerCode, null, false);
        addValue("ProductCode", Edid?.ProductCode, null, false);
        addValue("Serial", Edid?.Serial, null, false);
        addValue("Model", Edid?.Model, null, false);
        addValue("SerialNo", Edid?.SerialNumber, null, false);
        addValue("SizeInMm", Edid?.PhysicalSize.ToString(), null, false);

        // GetScaleFactorForMonitor
        addValue("", "GetScaleFactorForMonitor", null, true);
        addValue("ScaleFactor", ScaleFactor.ToString(), null, false);

        // EnumDisplayDevices
        addValue("", "EnumDisplayDevices", null, true);
        addValue("DeviceId", AttachedDisplay?.DeviceId, null, false);
        addValue("DeviceKey", AttachedDisplay?.DeviceKey, null, false);
        addValue("DeviceString", AttachedDisplay?.DeviceString, null, false);
        addValue("DeviceName", AttachedDisplay?.DeviceName, null, false);
        // TODO : addValue("StateFlags", AttachedDisplay?.State.ToString(), null, false);

        addValue("", "EnumDisplayDevices", null, true);
        addValue("DeviceId", DeviceId, null, false);
        addValue("DeviceKey", DeviceKey, null, false);
        addValue("DeviceString", DeviceString, null, false);
        addValue("DeviceName", AttachedDevice?.DeviceName, null, false);
        // TODO : addValue("StateFlags", AttachedDevice?.State.ToString(), null, false);

    }
    public override string ToString() => DeviceString;

}