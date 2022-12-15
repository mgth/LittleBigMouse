using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using Microsoft.Win32;

using System;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using Avalonia;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout;

public class MonitorSource : ReactiveObject
{
    public Monitor Monitor { get; }
    public MonitorDevice Device { get; }

    public MonitorSource(Monitor monitor, MonitorDevice device)
    {
        Monitor = monitor;
        Device = device;

        this.WhenAnyValue(
            e => e.InPixel.Width,
            e => e.InPixel.Height,
            (w, h) => $"{w}x{h}"
        ).ToProperty(this, e => e.IdResolution,out _idResolution);

        this.WhenAnyValue(
            e => e.Device.Primary
        ).ToProperty(this, e => e.Primary,out _primary);

        InPixel = new ScreenSizeInPixels(this);

        this.WhenAnyValue(
            e => e.InPixel,
            e => e.EffectiveDpi,
            e => e.Monitor.Layout,
            (px,dpi,layout) => px.ScaleDip(dpi,layout)
        ).ToProperty(this, e => e.InDip,out _inDip);

        this.WhenAnyValue(
            e => e.Monitor.PhysicalRotated.Width,
            e => e.Monitor.PhysicalRotated.Height,
            e => e.InPixel.Width,
            e => e.InPixel.Height,  
            (pw,ph,w,h) => new DisplayRatioValue(pw/w,ph/h)
        ).ToProperty(this, e => e.RealPitch,out _realPitch);

        this.WhenAnyValue(
            e => e.RealPitch,
            e => e.Monitor.PhysicalRatio,
            (p,r) => p.Multiply(r)
        ).ToProperty(this, e => e.Pitch,out _pitch);

        this.WhenAnyValue(
            e => e.DipToPixelRatio,
            (r) => r.Inverse()
        ).ToProperty(this, e => e.PixelToDipRatio,out _pixelToDipRatio);

        this.WhenAnyValue(
            e => e.Monitor.Layout.DpiAwarenessContext, 
            e => e.RealDpi.X,
            e => e.RealDpi.Y,
            e => e.DpiAwareAngularDpi.X,
            e => e.DpiAwareAngularDpi.Y,
            e => e.Monitor.Layout.PrimarySource.EffectiveDpi.X,
            e => e.Monitor.Layout.PrimarySource.EffectiveDpi.Y,
            e => e.EffectiveDpi.X,
            e => e.EffectiveDpi.Y,
            UpdateDipToPixelRatio
        ).ToProperty(this, e => e.RealPitch,out _realPitch);

        this.WhenAnyValue(
            e => e.Monitor.PhysicalRatio, 
            e => e.PhysicalToPixelRatio,
            e => e.DipToPixelRatio,
            (phy,phy2px,dip2px) => phy2px.Multiply(dip2px.Inverse()).Multiply(phy)
        ).ToProperty(this, e => e.MmToDipRatio,out _mmToDipRatio);

        this.WhenAnyValue(
            e => e.RealPitch, 
            (pitch) => Inch.Multiply(pitch.Inverse())
        ).ToProperty(this, e => e.RealDpi,out _realDpi);

        this.WhenAnyValue(
            e => e.Pitch, 
            (pitch) => Inch.Multiply(pitch.Inverse())
        ).ToProperty(this, e => e.DpiX,out _dpiX);

        this.WhenAnyValue(
            e => e.Pitch, 
            (pitch) => pitch.Inverse()
        ).ToProperty(this, e => e.PhysicalToPixelRatio,out _physicalToPixelRatio);

        this.WhenAnyValue(
            e => e.RealDpi.X,
            e => e.RealDpi.Y,
            GetRealDpiAvg
        ).ToProperty(this, e => e.RealDpiAvg,out _realDpiAvg);

        this.WhenAnyValue(
            e => e.EffectiveDpi.X
        ).ToProperty(this, e => e.WinDpiX,out _winDpiX);

        this.WhenAnyValue(
            e => e.EffectiveDpi.Y
        ).ToProperty(this, e => e.WinDpiY,out _winDpiY);

        this.WhenAnyValue(
            e => e.Device.RawDpi,
            dpi => new DisplayRatioValue(dpi)
        ).ToProperty(this, e => e.RawDpi,out _rawDpi);

        this.WhenAnyValue(
            e => e.Device.EffectiveDpi,
            dpi => new DisplayRatioValue(dpi)
        ).ToProperty(this, e => e.EffectiveDpi,out _effectiveDpi);

        this.WhenAnyValue(
            e => e.Device.AngularDpi,
            dpi => new DisplayRatioValue(dpi)
        ).ToProperty(this, e => e.DpiAwareAngularDpi,out _dpiAwareAngularDpi);
    }

    [DataMember] public string IdResolution => _idResolution.Get();

    readonly ObservableAsPropertyHelper<string> _idResolution;

    [DataMember]
    public bool Primary => _primary.Get();

    readonly ObservableAsPropertyHelper<bool> _primary;

    //Pixel
    [DataMember] public IDisplaySize InPixel { get; }

    // Dip
    [DataMember] public IDisplaySize InDip => _inDip.Get();
    readonly ObservableAsPropertyHelper<IDisplaySize> _inDip;

    [DataMember]
    public IDisplayRatio RealPitch
    {
        get => _realPitch.Get();
        set
        {
            Monitor.PhysicalRotated.Width = InPixel.Width * value.X;
            Monitor.PhysicalRotated.Height = InPixel.Height * value.Y;
        }
    }
    readonly ObservableAsPropertyHelper<IDisplayRatio> _realPitch;

    //calculated
    [DataMember]
    public IDisplayRatio Pitch => _pitch.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _pitch;

    [DataMember]
    public IDisplayRatio PixelToDipRatio => _pixelToDipRatio.Get();

    readonly ObservableAsPropertyHelper<IDisplayRatio> _pixelToDipRatio;

    [DataMember]
    public IDisplayRatio DipToPixelRatio => _dipToPixelRatio.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _dipToPixelRatio;

    [DataMember]
    public IDisplayRatio MmToDipRatio => _mmToDipRatio.Get();

    readonly ObservableAsPropertyHelper<IDisplayRatio> _mmToDipRatio;


    [DataMember]
    public IDisplayRatio RealDpi => _realDpi.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _realDpi;

    [DataMember]
    public IDisplayRatio DpiX => _dpiX.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _dpiX ;

    static readonly IDisplayRatio Inch = new DisplayRatioValue(25.4);

    [DataMember]
    public IDisplayRatio PhysicalToPixelRatio => _physicalToPixelRatio.Get();

    readonly ObservableAsPropertyHelper<IDisplayRatio> _physicalToPixelRatio;

    [DataMember]
    public double RealDpiAvg => _realDpiAvg.Get();

    readonly ObservableAsPropertyHelper<double> _realDpiAvg;

    static double GetRealDpiAvg(double dpiX, double dpiY) => Math.Sqrt(Math.Pow(dpiX,2.0) + Math.Pow(dpiY,2.0)) / Math.Sqrt(2);

    [DataMember]
    public Rect GuiLocation
    {
        get => _guiLocation;
        //{
        //    Width = 0.5,
        //    Height = (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
        //    Y = 1 - (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
        //    X = 1 - 0.5
        //});
        set => this.RaiseAndSetIfChanged(ref _guiLocation, value);
    }
    Rect _guiLocation;

    public enum DpiType
    {
        Effective = 0,
        Angular = 1,
        Raw = 2
    } //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

    [DllImport("Shcore.dll")]
    static extern IntPtr GetDpiForMonitor([In] IntPtr hmonitor, [In] DpiType dpiType, [Out] out uint dpiX,
        [Out] out uint dpiY);


    [DataMember]
    public double WinDpiX => _winDpiX.Get();
    readonly ObservableAsPropertyHelper<double> _winDpiX;

    [DataMember]
    public double WinDpiY => _winDpiY.Get();
    readonly ObservableAsPropertyHelper<double> _winDpiY;

    [DataMember] public IDisplayRatio RawDpi => _rawDpi.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _rawDpi;

    [DataMember] public IDisplayRatio EffectiveDpi => _effectiveDpi.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _effectiveDpi;

    [DataMember] public IDisplayRatio DpiAwareAngularDpi => _dpiAwareAngularDpi.Get();
    readonly ObservableAsPropertyHelper<IDisplayRatio> _dpiAwareAngularDpi;

    public static IDisplayRatio UpdateDipToPixelRatio(
        User32.DPI_Awareness_Context aware,
        double dpiRealX, double dpiRealY,
        double dpiAngX, double dpiAngY,
        double srcDpiX, double srcDpiY,
        double dpiEffectiveX, double dpiEffectiveY)
    {
        switch (aware)
        {
            case User32.DPI_Awareness_Context.Unaware:
                return new DisplayRatioValue(
                    Math.Round(dpiRealX / dpiAngX * 10) / 10,
                    Math.Round(dpiRealY / dpiAngY * 10) / 10);
            //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;

            case User32.DPI_Awareness_Context.System_Aware:
                return new DisplayRatioValue(
                    srcDpiX / 96,
                    srcDpiY / 96
                );

            case User32.DPI_Awareness_Context.Per_Monitor_Aware:
                return new DisplayRatioValue(
                    dpiEffectiveX / 96,
                    dpiEffectiveY / 96
                );

            case User32.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                return new DisplayRatioValue(
                    dpiEffectiveX / 96,
                    dpiEffectiveY / 96
                //DpiAwareAngularDpi.X / 96,
                //DpiAwareAngularDpi.Y / 96
                );

            case User32.DPI_Awareness_Context.Unset:
            default:
                throw new ArgumentOutOfRangeException();
        }
    }

    public void Load(RegistryKey baseKey)
    {
        using (var key = baseKey.OpenSubKey("GuiLocation"))
        {
            if (key != null)
            {
                var left = key.GetKey("Left", () => GuiLocation.Left);
                var width = key.GetKey("Width", () => GuiLocation.Width);
                var top = key.GetKey("Top", () => GuiLocation.Top);
                var height = key.GetKey("Height", () => GuiLocation.Height);
                GuiLocation = new Rect(new Point(left, top), new Size(width, height));
            }
        }
    }
    public void Save(RegistryKey baseKey)
    {
        using (var key = baseKey.CreateSubKey("GuiLocation"))
        {
            if(key!=null)
            {
                key.SetKey("Left", GuiLocation.Left);
                key.SetKey("Width", GuiLocation.Width);
                key.SetKey("Top", GuiLocation.Top);
                key.SetKey("Height", GuiLocation.Height);
            }
        }

        using (var key = baseKey.CreateSubKey(Device.IdMonitor))
        {
            if (key != null)
            {
                key.SetKey("PixelX", InPixel.X);
                key.SetKey("PixelY", InPixel.Y);
                key.SetKey("PixelWidth", InPixel.Width);
                key.SetKey("PixelHeight", InPixel.Height);

                key.SetKey("Primary", Primary);
            }
        }
    }

}
