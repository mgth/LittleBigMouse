using Avalonia;
using System.Runtime.Serialization;
using HLab.Sys.Windows.API;

namespace HLab.Sys.Windows.Monitors;

public class PhysicalAdapter : DisplayDevice
{
    public nint HMonitor { get; set; }

// MONITORINFOEX 
    public Rect MonitorArea { get; set; }

    public Rect WorkArea { get; set; }

    public bool Primary { get; set; }

    [DataMember] public string CapabilitiesString { get; set; }//DDCCIGetCapabilitiesString

    [DataMember] public Vector EffectiveDpi { get; set; } //GetDpiForMonitor

    [DataMember] public Vector AngularDpi { get; set; } //GetDpiForMonitor

    [DataMember] public Vector RawDpi { get; set; } //GetDpiForMonitor

    //https://msdn.microsoft.com/fr-fr/library/windows/desktop/dn302060.aspx
    [DataMember] public double ScaleFactor { get; set; }//GetScaleFactorForMonitor

    [DataMember] public string WallpaperPath { get; set; }
    [DataMember] public DesktopWallpaperPosition WallpaperPosition { get; set; }
    [DataMember] public uint Background { get; set; }
}