using System;
using System.Runtime.Serialization;
using Avalonia;
using Avalonia.Controls;
using Microsoft.Win32;

namespace HLab.Sys.Windows.Monitors;

public class MonitorDevice : DisplayDevice
{
    public new PhysicalAdapter Parent
    {
        get => base.Parent as PhysicalAdapter;
        set => base.Parent = value;
    }

    [DataMember] public string PnpCode { get; init; }

    [DataMember] public string PhysicalId { get; set; }
    [DataMember] public string SourceId { get; set; }

    [DataMember] public IEdid Edid { get; init; }

    [DataMember] public string MonitorNumber { get; set; }

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

    public string ConfigPath() => PhysicalId;

    public void DisplayValues(Action<string, string, Action, bool> addValue) {
        addValue("Registry", Edid.HKeyName, () => { OpenRegKey(Edid.HKeyName); }, false);
        addValue("Microsoft Id", PhysicalId, null, false);

        // EnumDisplaySettings
        addValue("", "EnumDisplaySettings", null, true);
        addValue("DisplayOrientation", Parent?.CurrentMode?.DisplayOrientation.ToString(), null, false);
        addValue("Position", Parent?.CurrentMode?.Position.ToString(), null, false);
        addValue("Pels", Parent?.CurrentMode?.Pels.ToString(), null, false);
        addValue("BitsPerPixel", Parent?.CurrentMode?.BitsPerPixel.ToString(), null, false);
        addValue("DisplayFrequency", Parent?.CurrentMode?.DisplayFrequency.ToString(), null, false);
        addValue("DisplayFlags", Parent?.CurrentMode?.DisplayFlags.ToString(), null, false);
        addValue("DisplayFixedOutput", Parent?.CurrentMode?.DisplayFixedOutput.ToString(), null, false);

        // GetDeviceCaps
        addValue("", "GetDeviceCaps", null, true);
        addValue("Size", Parent.Capabilities.Size.ToString(), null, false);
        addValue("Res", Parent.Capabilities.Resolution.ToString(), null, false);
        addValue("LogPixels", Parent.Capabilities.LogPixels.ToString(), null, false);
        addValue("BitsPixel", Parent.Capabilities.BitsPixel.ToString(), null, false);
        //AddValue("Color Planes", Monitor.Adapter.DeviceCaps.Planes.ToString());
        addValue("Aspect", Parent.Capabilities.Aspect.ToString(), null, false);
        //AddValue("BltAlignment", Monitor.Adapter.DeviceCaps.BltAlignment.ToString());

        //GetDpiForMonitor
        addValue("", "GetDpiForMonitor", null, true);
        addValue("EffectiveDpi", Parent.EffectiveDpi.ToString(), null, false);
        addValue("AngularDpi", Parent.AngularDpi.ToString(), null, false);
        addValue("RawDpi", Parent.RawDpi.ToString(), null, false);

        // GetMonitorInfo
        addValue("", "GetMonitorInfo", null, true);
        addValue("Primary", Parent.Primary.ToString(), null, false);
        addValue("MonitorArea", Parent.MonitorArea.ToString(), null, false);
        addValue("WorkArea", Parent.WorkArea.ToString(), null, false);


        // EDID
        addValue("", "EDID", null, true);
        addValue("ManufacturerCode", Edid?.ManufacturerCode, null, false);
        addValue("ProductCode", Edid?.ProductCode, null, false);
        addValue("Serial", Edid?.Serial, null, false);
        addValue("Model", Edid?.Model, null, false);
        addValue("SerialNo", Edid?.SerialNumber, null, false);
        addValue("SizeInMm", Edid?.PhysicalSize.ToString(), null, false);
        addValue("VideoInterface", Edid?.VideoInterface.ToString(), null, false);

        // GetScaleFactorForMonitor
        addValue("", "GetScaleFactorForMonitor", null, true);
        addValue("ScaleFactor", Parent.ScaleFactor.ToString(), null, false);

        // EnumDisplayDevices
        addValue("", "EnumDisplayDevices", null, true);
        addValue("DeviceId", Parent?.DeviceId, null, false);
        addValue("DeviceKey", Parent?.DeviceKey, null, false);
        addValue("DeviceString", Parent?.DeviceString, null, false);
        addValue("DeviceName", Parent?.DeviceName, null, false);
        addValue("StateFlags", Parent?.State.ToString(), null, false);

        addValue("", "EnumDisplayDevices", null, true);
        addValue("DeviceId", DeviceId, null, false);
        addValue("DeviceKey", DeviceKey, null, false);
        addValue("DeviceString", DeviceString, null, false);
        addValue("DeviceName", DeviceName, null, false);
        addValue("StateFlags", State.ToString(), null, false);

    }
    public override string ToString() => DeviceString;

}