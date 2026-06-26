using System.Drawing;
using LittleBigMouse.DisplayLayout.Dimensions;
using System.Runtime.Serialization;
using HLab.Base.ReactiveUI;
using HLab.ColorTools;
using HLab.Geo;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

public enum WallpaperStyle
{
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
    Span
}

/// <summary>
/// 
/// </summary>
public class DisplaySource : SavableReactiveModel
{
    public string Id { get; }

    public DisplaySource(string id)
    {
        Id = id;

        _idResolution = this.WhenAnyValue(
            e => e.InPixel.Width,
            e => e.InPixel.Height,
            (w, h) => $"{w}x{h}"
        ).ToProperty(this, e => e.IdResolution);

        _winDpiX = this.WhenAnyValue(
            e => e.EffectiveDpi.X
        ).ToProperty(this, e => e.WinDpiX);

        _winDpiY = this.WhenAnyValue(
            e => e.EffectiveDpi.Y
        ).ToProperty(this, e => e.WinDpiY);
    }

    /// <summary>
    /// Display ID per resolution
    /// </summary>
    [DataMember] public string IdResolution => _idResolution.Value;
    readonly ObservableAsPropertyHelper<string> _idResolution;

    /// <summary>
    /// Source is the primary display
    /// </summary>
    [DataMember]
    public bool Primary { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    /// <summary>
    /// Source Device Name
    /// </summary>
    [DataMember]
    public string DeviceName { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    /// <summary>
    /// Source Name
    /// </summary>
    [DataMember]
    public string SourceName { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    /// <summary>
    /// Display Device Name
    /// </summary>
    [DataMember]
    public string DisplayName { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    /// <summary>
    /// Source number in windows system
    /// </summary>
    [DataMember]
    public string SourceNumber { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    /// <summary>
    /// Current monitor frequency
    /// </summary>
    [DataMember]
    public int DisplayFrequency { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    /// <summary>
    /// Display size in pixel
    /// </summary>
    [DataMember]
    public DisplaySizeInPixels InPixel { get; } = new(new ());

    /// <summary>
    /// Monitor orientation (0=0°, 1=90°, 2=180°, 3=270°)
    /// </summary>
    [DataMember]
    public int Orientation { get; set => SetUnsavedValue(ref field, value); }


    [DataMember]
    public string WallpaperPath
    {
       get;
       set => this.RaiseAndSetIfChanged(ref field, value);
    }

    [DataMember]
    public WallpaperStyle WallpaperStyle { get; set => this.RaiseAndSetIfChanged(ref field, value); }


    [DataMember]
    public ColorRGB<double> BackgroundColor { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    [DataMember]
    public string InterfaceName { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    [DataMember]
    public string InterfaceLogo { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    [DataMember]
    public bool AttachedToDesktop { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    [DataMember]
    public double WinDpiX => _winDpiX.Value;
    readonly ObservableAsPropertyHelper<double> _winDpiX;

    [DataMember]
    public double WinDpiY => _winDpiY.Value;
    readonly ObservableAsPropertyHelper<double> _winDpiY;

    [DataMember] public DisplayRatioValue RawDpi { get; } = new(96);

    [DataMember] public DisplayRatioValue EffectiveDpi { get; } = new(96);

    [DataMember] public DisplayRatioValue DpiAwareAngularDpi { get; } = new(96);
}
