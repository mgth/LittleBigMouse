using LittleBigMouse.DisplayLayout.Dimensions;
using System.Runtime.Serialization;
using Avalonia;
using Avalonia.Media;
using HLab.Base.Avalonia;
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
public class DisplaySource : ReactiveModel
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
    public bool Primary
    {
        get => _primary;
        set => this.RaiseAndSetIfChanged(ref _primary, value) ;
    }
    bool _primary;

    /// <summary>
    /// Source Device Name
    /// </summary>
    [DataMember]
    public string DeviceName
    {
        get => _deviceName;
        set => this.RaiseAndSetIfChanged(ref _deviceName, value) ;
    }
    string _deviceName;

    /// <summary>
    /// Source Name
    /// </summary>
    [DataMember]
    public string SourceName
    {
        get => _sourceName;
        set => this.RaiseAndSetIfChanged(ref _sourceName, value) ;
    }
    string _sourceName;

    /// <summary>
    /// Display Device Name
    /// </summary>
    [DataMember]
    public string DisplayName
    {
        get => _displayName;
        set => this.RaiseAndSetIfChanged(ref _displayName, value) ;
    }
    string _displayName;

    /// <summary>
    /// Source number in windows system
    /// </summary>
    [DataMember]
    public string SourceNumber
    {
        get => _sourceNumber;
        set => this.RaiseAndSetIfChanged(ref _sourceNumber, value) ;
    }
    string _sourceNumber;

    /// <summary>
    /// Current monitor frequency
    /// </summary>
    [DataMember]
    public int DisplayFrequency
    {
        get => _displayFrequency;
        set => this.RaiseAndSetIfChanged(ref _displayFrequency, value) ;
    }
    int _displayFrequency;

    /// <summary>
    /// Display size in pixel
    /// </summary>
    [DataMember]
    public DisplaySizeInPixels InPixel { get; } = new(new Rect());

    /// <summary>
    /// Monitor orientation (0=0°, 1=90°, 2=180°, 3=270°)
    /// </summary>
    [DataMember]
    public int Orientation
    {
        get => _orientation;
        set => this.SetUnsavedValue(ref _orientation, value);
    }
    int _orientation;


    [DataMember]
    public string WallpaperPath {
        get => _wallpaperPath;
        set => this.RaiseAndSetIfChanged(ref _wallpaperPath, value);
    }
    string _wallpaperPath;

    [DataMember]
    public WallpaperStyle WallpaperStyle
    {
        get => _wallpaperStyle;
        set => this.RaiseAndSetIfChanged(ref _wallpaperStyle, value);
    }
    WallpaperStyle _wallpaperStyle;


    [DataMember]
    public Color BackgroundColor {
        get => _backgroundColor;
        set => this.RaiseAndSetIfChanged(ref _backgroundColor, value);
    }
    Color _backgroundColor;

    [DataMember]
    public string InterfaceName
    {
        get => _interfaceName;
        set => this.RaiseAndSetIfChanged(ref _interfaceName, value);
    }
    string _interfaceName;

    [DataMember]
    public string InterfaceLogo
    {
        get => _interfaceLogo;
        set => this.RaiseAndSetIfChanged(ref _interfaceLogo, value);
    }
    string _interfaceLogo;

    [DataMember]
    public bool AttachedToDesktop
    {
        get => _attachedToDesktop;
        set => this.RaiseAndSetIfChanged(ref _attachedToDesktop, value);
    }
    bool _attachedToDesktop;

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
