using LittleBigMouse.DisplayLayout.Dimensions;
using System.Runtime.Serialization;
using Avalonia;
using Avalonia.Media;
using Microsoft.Win32;
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
public class DisplaySource : ReactiveObject
{
    public string IdMonitor { get; }

    public DisplaySource(string idMonitor)
    {
        IdMonitor = idMonitor;

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
    /// Source number in windows system
    /// </summary>
    [DataMember]
    public string SourceNb
    {
        get => _sourceNb;
        set => this.RaiseAndSetIfChanged(ref _sourceNb, value) ;
    }
    string _sourceNb;

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

    public string WallpaperPath {
        get => _wallpaperPath;
        set => this.RaiseAndSetIfChanged(ref _wallpaperPath, value);
    }
    string _wallpaperPath;


    public Color BackgroundColor {
        get => _backgroundColor;
        set => this.RaiseAndSetIfChanged(ref _backgroundColor, value);
    }
    Color _backgroundColor;

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

    [DataMember]
    public double WinDpiX => _winDpiX.Value;
    readonly ObservableAsPropertyHelper<double> _winDpiX;

    [DataMember]
    public double WinDpiY => _winDpiY.Value;
    readonly ObservableAsPropertyHelper<double> _winDpiY;

    [DataMember] public DisplayRatioValue RawDpi { get; } = new(96);

    [DataMember] public DisplayRatioValue EffectiveDpi { get; } = new(96);

    [DataMember] public DisplayRatioValue DpiAwareAngularDpi { get; } = new(96);

    public void Load(RegistryKey baseKey)
    {
        using var key = baseKey.OpenSubKey("GuiLocation");

        if (key == null) return;

        var left = key.GetKey("Left", () => GuiLocation.Left);
        var width = key.GetKey("Width", () => GuiLocation.Width);
        var top = key.GetKey("Top", () => GuiLocation.Top);
        var height = key.GetKey("Height", () => GuiLocation.Height);

        GuiLocation = new Rect(new Point(left, top), new Size(width, height));
    }

}
