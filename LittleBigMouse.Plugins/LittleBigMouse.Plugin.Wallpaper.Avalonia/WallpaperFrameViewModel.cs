using System.Reactive.Linq;
using Avalonia.Media;
using HLab.Base.ReactiveUI;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

public class WallpaperFrameViewModel : ViewModel<PhysicalMonitor>
{
    readonly WallpaperManager _manager;

    public WallpaperFrameViewModel(WallpaperManager manager)
    {
        _manager = manager;

        // Mode and span image are layout-wide: edited from any frame, reflected in all.
        manager.WhenAnyValue(m => m.Mode)
            .Subscribe(_ =>
            {
                this.RaisePropertyChanged(nameof(IsPerScreenMode));
                this.RaisePropertyChanged(nameof(IsSpanMode));
            }).DisposeWith(this);

        manager.WhenAnyValue(m => m.SpanImagePath)
            .Subscribe(_ => this.RaisePropertyChanged(nameof(SpanImageName)))
            .DisposeWith(this);

        this.WhenAnyValue(e => e.Model)
            .Where(m => m != null)
            .Subscribe(_ => RaiseScreenProperties())
            .DisposeWith(this);
    }

    ScreenWallpaperSettings? Screen => Model == null ? null : _manager.GetScreen(Model.Id);

    void RaiseScreenProperties()
    {
        this.RaisePropertyChanged(nameof(IsImageKind));
        this.RaisePropertyChanged(nameof(IsColorKind));
        this.RaisePropertyChanged(nameof(ImageName));
        this.RaisePropertyChanged(nameof(SelectedStyle));
        this.RaisePropertyChanged(nameof(ColorHex));
        this.RaisePropertyChanged(nameof(ColorBrush));
    }

    public bool IsPerScreenMode
    {
        get => _manager.Mode == WallpaperMode.PerScreen;
        set { if (value) _manager.Mode = WallpaperMode.PerScreen; }
    }

    public bool IsSpanMode
    {
        get => _manager.Mode == WallpaperMode.Span;
        set { if (value) _manager.Mode = WallpaperMode.Span; }
    }

    public bool IsImageKind
    {
        get => Screen?.Kind != ScreenWallpaperKind.Color;
        set { if (value) SetKind(ScreenWallpaperKind.Image); }
    }

    public bool IsColorKind
    {
        get => Screen?.Kind == ScreenWallpaperKind.Color;
        set { if (value) SetKind(ScreenWallpaperKind.Color); }
    }

    void SetKind(ScreenWallpaperKind kind)
    {
        if (Screen is not { } screen || screen.Kind == kind) return;
        screen.Kind = kind;
        RaiseScreenProperties();
        _manager.Commit();
    }

    public string ImageName
        => Path.GetFileName(Screen?.ImagePath) is { Length: > 0 } name ? name : "Choose image…";

    public void SetImage(string path)
    {
        if (Screen is not { } screen) return;
        screen.ImagePath = path;
        screen.Kind = ScreenWallpaperKind.Image;
        RaiseScreenProperties();
        _manager.Commit();
    }

    public static WallpaperStyle[] Styles { get; } =
        [WallpaperStyle.Fill, WallpaperStyle.Fit, WallpaperStyle.Stretch, WallpaperStyle.Tile, WallpaperStyle.Center];

    public WallpaperStyle SelectedStyle
    {
        get => Screen?.Style ?? WallpaperStyle.Fill;
        set
        {
            if (Screen is not { } screen || screen.Style == value) return;
            screen.Style = value;
            this.RaisePropertyChanged();
            _manager.Commit();
        }
    }

    public string ColorHex
    {
        get => Screen?.Color ?? "#204060";
        set
        {
            if (Screen is not { } screen || screen.Color == value) return;
            if (WallpaperManager.ParseColor(value) == null) return; // incomplete input, keep typing
            screen.Color = value;
            this.RaisePropertyChanged();
            this.RaisePropertyChanged(nameof(ColorBrush));
            _manager.Commit();
        }
    }

    public IBrush ColorBrush
        => Color.TryParse(ColorHex, out var color) ? new SolidColorBrush(color) : Brushes.Transparent;

    public string SpanImageName
        => Path.GetFileName(_manager.SpanImagePath) is { Length: > 0 } name ? name : "Choose image…";

    public void SetSpanImage(string path) => _manager.SpanImagePath = path;
}
