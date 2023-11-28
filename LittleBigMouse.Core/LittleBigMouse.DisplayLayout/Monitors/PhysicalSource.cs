using System;
using System.Runtime.Serialization;
using HLab.Base.Avalonia;
using HLab.Sys.Windows.API;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

public class PhysicalSource : ReactiveModel
{
    public PhysicalMonitor Monitor { get; }
    public DisplaySource Source { get; }

    static readonly IDisplayRatio Inch = new DisplayRatioValue(25.4);

    static double GetRealDpiAvg(double dpiX, double dpiY) => Math.Sqrt(Math.Pow(dpiX, 2.0) + Math.Pow(dpiY, 2.0)) / Math.Sqrt(2);

    /// <summary>
    /// Calculate DPI ratio from <see cref="WinDef.DpiAwareness"/> and DPI values.
    /// </summary>
    /// <param name="aware"></param>
    /// <param name="dpiRealX"></param>
    /// <param name="dpiRealY"></param>
    /// <param name="dpiAngX"></param>
    /// <param name="dpiAngY"></param>
    /// <param name="srcDpiX"></param>
    /// <param name="srcDpiY"></param>
    /// <param name="dpiEffectiveX"></param>
    /// <param name="dpiEffectiveY"></param>
    /// <returns></returns>
    /// <exception cref="ArgumentOutOfRangeException"></exception>
    /// 
    public static IDisplayRatio UpdateDipToPixelRatio(
        WinDef.DpiAwareness aware,
        double dpiRealX, double dpiRealY,
        double dpiAngX, double dpiAngY,
        double srcDpiX, double srcDpiY,
        double dpiEffectiveX, double dpiEffectiveY)
    {
        switch (aware)
        {
            case WinDef.DpiAwareness.Unaware:
                return new DisplayRatioValue(
                    Math.Round(dpiRealX / dpiAngX * 10) / 10,
                    Math.Round(dpiRealY / dpiAngY * 10) / 10);
            //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;

            case WinDef.DpiAwareness.SystemAware:
                return new DisplayRatioValue(
                    srcDpiX / 96,
                    srcDpiY / 96
                );

            case WinDef.DpiAwareness.PerMonitorAware:
                return new DisplayRatioValue(
                    dpiEffectiveX / 96,
                    dpiEffectiveY / 96
                );

            case WinDef.DpiAwareness.Invalid:
                return new DisplayRatioValue(
                    dpiEffectiveX / 96,
                    dpiEffectiveY / 96
                );

            default:
                throw new ArgumentOutOfRangeException();
        }
    }

    public PhysicalSource(string deviceId, PhysicalMonitor monitor, DisplaySource source)
    {
        DeviceId = deviceId;

        Monitor = monitor;
        Source = source;

        _inDip = this.WhenAnyValue(
            e => e.Source.InPixel,
            e => e.Source.EffectiveDpi,
            e => e.Monitor.Layout,
            (px, dpi, layout) => px.ScaleDip(dpi, layout)
        ).ToProperty(this, e => e.InDip);

        _realPitch = this.WhenAnyValue(
            e => e.Monitor.PhysicalRotated.Width,
            e => e.Monitor.PhysicalRotated.Height,
            e => e.Source.InPixel.Width,
            e => e.Source.InPixel.Height,
            (pw, ph, w, h) => new DisplayRatioValue(pw / w, ph / h)
        ).ToProperty(this, e => e.RealPitch);

        _pitch = this.WhenAnyValue(
            e => e.RealPitch,
            e => e.Monitor.DepthRatio,
            (p, r) => p.Multiply(r)
        ).ToProperty(this, e => e.Pitch);

        _realDpi = this.WhenAnyValue(
            e => e.RealPitch,
            (pitch) => Inch.Multiply(pitch.Inverse())
        ).ToProperty(this, e => e.RealDpi);

        _realDpiAvg = this.WhenAnyValue(
            e => e.RealDpi.X,
            e => e.RealDpi.Y,
            GetRealDpiAvg
        ).ToProperty(this, e => e.RealDpiAvg);

        _dipToPixelRatio = this.WhenAnyValue(
            e => e.Monitor.Layout.DpiAwareness,
            e => e.RealDpi.X,
            e => e.RealDpi.Y,
            e => e.Source.DpiAwareAngularDpi.X,
            e => e.Source.DpiAwareAngularDpi.Y,
            e => e.Monitor.Layout.PrimarySource.EffectiveDpi.X,
            e => e.Monitor.Layout.PrimarySource.EffectiveDpi.Y,
            e => e.Source.EffectiveDpi.X,
            e => e.Source.EffectiveDpi.Y,
            UpdateDipToPixelRatio
        ).ToProperty(this, e => e.DipToPixelRatio);

        _mmToDipRatio = this.WhenAnyValue(
            e => e.Monitor.DepthRatio,
            e => e.PhysicalToPixelRatio,
            e => e.DipToPixelRatio,
            (phy, phy2px, dip2px) => phy2px.Multiply(dip2px.Inverse()).Multiply(phy)
        ).ToProperty(this, e => e.MmToDipRatio);

        _dpi = this.WhenAnyValue(
            e => e.Pitch,
            (pitch) => Inch.Multiply(pitch.Inverse())
        ).ToProperty(this, e => e.Dpi);

        _pixelToDipRatio = this.WhenAnyValue(
            e => e.DipToPixelRatio,
            (r) => r.Inverse()
        ).ToProperty(this, e => e.PixelToDipRatio);

        _physicalToPixelRatio = this.WhenAnyValue(
            e => e.Pitch,
            (pitch) => pitch.Inverse()
        ).ToProperty(this, e => e.PhysicalToPixelRatio);

    }

    /// <summary>
    /// Device ID
    /// </summary>
    public string DeviceId {
        get => _deviceId;
        set => this.RaiseAndSetIfChanged(ref _deviceId, value);
    }
    string _deviceId;


    /// <summary>
    /// Display size in DIP (Device Independent Pixel)
    /// </summary>
    [DataMember] public IDisplaySize InDip => _inDip.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _inDip;

    /// <summary>
    /// Actual Pitch calculated from <see cref="DisplaySource.InPixel"/> and <see cref="PhysicalMonitor.PhysicalRotated"/>
    /// </summary>
    [DataMember] public IDisplayRatio RealPitch
    {
        get => _realPitch.Value;
        set
        {
            Monitor.PhysicalRotated.Width = Source.InPixel.Width * value.X;
            Monitor.PhysicalRotated.Height = Source.InPixel.Height * value.Y;
        }
    }
    readonly ObservableAsPropertyHelper<IDisplayRatio> _realPitch;

    /// <summary>
    /// Relative pitch with distance to viewer corrected <see cref="RealPitch"/> and <see cref="PhysicalMonitor.DepthRatio"/>
    /// </summary>
    [DataMember]
    public IDisplayRatio Pitch => _pitch.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _pitch;


    /// <summary>
    /// Real physical DPI
    /// </summary>
    [DataMember]
    public IDisplayRatio RealDpi => _realDpi.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _realDpi;

    /// <summary>
    /// Squared average of <see cref="RealDpi"/> in x and y direction
    /// </summary>
    [DataMember]
    public double RealDpiAvg => _realDpiAvg.Value;
    readonly ObservableAsPropertyHelper<double> _realDpiAvg;

    /// <summary>
    /// Dip (Device Independent ) to pixel ratio
    /// </summary>

    [DataMember]
    public IDisplayRatio DipToPixelRatio => _dipToPixelRatio?.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _dipToPixelRatio;


    /// <summary>
    /// Millimeter to DIP ratio
    /// </summary>
    [DataMember]
    public IDisplayRatio MmToDipRatio => _mmToDipRatio?.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _mmToDipRatio;

    /// <summary>
    /// Dpi (Dot per inch) calculated from <see cref="Pitch"/>
    /// </summary>
    [DataMember]
    public IDisplayRatio Dpi => _dpi.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _dpi;

    /// <summary>
    /// Pixel to DIP ratio
    /// </summary>
    [DataMember]
    public IDisplayRatio PixelToDipRatio => _pixelToDipRatio.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _pixelToDipRatio;

    /// <summary>
    /// Physical (mm) to pixel ratio
    /// </summary>
    [DataMember]
    public IDisplayRatio PhysicalToPixelRatio => _physicalToPixelRatio?.Value;
    readonly ObservableAsPropertyHelper<IDisplayRatio> _physicalToPixelRatio;

}