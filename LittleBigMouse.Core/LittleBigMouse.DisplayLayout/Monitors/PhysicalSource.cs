using System;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using HLab.Base.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

public class PhysicalSource : SavableReactiveModel
{
    [JsonIgnore]
    public PhysicalMonitor Monitor { get; }
    public DisplaySource Source { get; }

    // One instance per source, NOT static: derived ratios (_inch.Multiply(...)) subscribe to
    // their operands' PropertyChanged, and a static operand would root every PhysicalSource
    // ever created into its subscriber list for the process lifetime (issue #412).
    readonly IDisplayRatio _inch = new DisplayRatioValue(25.4);

    static double GetRealDpiAvg(double dpiX, double dpiY) => Math.Sqrt(Math.Pow(dpiX, 2.0) + Math.Pow(dpiY, 2.0)) / Math.Sqrt(2);

    /// <summary>
    /// Calculate DPI ratio from <see cref="DpiAwarenessKind"/> and DPI values.
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
        DpiAwarenessKind aware,
        double dpiRealX, double dpiRealY,
        double dpiAngX, double dpiAngY,
        double srcDpiX, double srcDpiY,
        double dpiEffectiveX, double dpiEffectiveY)
    {
       return aware switch
       {
          DpiAwarenessKind.Unaware => new DisplayRatioValue(Math.Round(dpiRealX / dpiAngX * 10) / 10,
             Math.Round(dpiRealY / dpiAngY * 10) / 10),
          //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;
          DpiAwarenessKind.SystemAware => new(srcDpiX / 96, srcDpiY / 96),
          DpiAwarenessKind.PerMonitorAware or DpiAwarenessKind.Invalid => new (
             dpiEffectiveX / 96, dpiEffectiveY / 96),
          _ => throw new ArgumentOutOfRangeException()
       };
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
        ).ToProperty(this, e => e.InDip).DisposeWith(this);

        _realPitch = this.WhenAnyValue(
            e => e.Monitor.PhysicalRotated.Width,
            e => e.Monitor.PhysicalRotated.Height,
            e => e.Source.InPixel.Width,
            e => e.Source.InPixel.Height,
            (pw, ph, w, h) => new DisplayRatioValue(pw / w, ph / h)
        ).ToProperty(this, e => e.RealPitch).DisposeWith(this);

        _pitch = this.WhenAnyValue(
            e => e.RealPitch,
            e => e.Monitor.DepthRatio,
            (p, r) => p.Multiply(r)
        ).ToProperty(this, e => e.Pitch).DisposeWith(this);

        _realDpi = this.WhenAnyValue(
            e => e.RealPitch,
            (pitch) => _inch.Multiply(pitch.Inverse())
        ).ToProperty(this, e => e.RealDpi).DisposeWith(this);

        _realDpiAvg = this.WhenAnyValue(
            e => e.RealDpi.X,
            e => e.RealDpi.Y,
            GetRealDpiAvg
        ).ToProperty(this, e => e.RealDpiAvg).DisposeWith(this);

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
        ).ToProperty(this, e => e.DipToPixelRatio).DisposeWith(this);

        _mmToDipRatio = this.WhenAnyValue(
            e => e.Monitor.DepthRatio,
            e => e.PhysicalToPixelRatio,
            e => e.DipToPixelRatio,
            (phy, phy2px, dip2px) => phy2px.Multiply(dip2px.Inverse()).Multiply(phy)
        ).ToProperty(this, e => e.MmToDipRatio).DisposeWith(this);

        _dpi = this.WhenAnyValue(
            e => e.Pitch,
            (pitch) => _inch.Multiply(pitch.Inverse())
        ).ToProperty(this, e => e.Dpi).DisposeWith(this);

        _pixelToDipRatio = this.WhenAnyValue(
            e => e.DipToPixelRatio,
            (r) => r.Inverse()
        ).ToProperty(this, e => e.PixelToDipRatio).DisposeWith(this);

        _physicalToPixelRatio = this.WhenAnyValue(
            e => e.Pitch,
            (pitch) => pitch.Inverse()
        ).ToProperty(this, e => e.PhysicalToPixelRatio).DisposeWith(this);

    }

    /// <summary>
    /// Device ID
    /// </summary>
    public string DeviceId
    {
       get;
       set => this.RaiseAndSetIfChanged(ref field, value);
    }


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

    public override void OnDispose()
    {
        Source.Dispose();
    }
}