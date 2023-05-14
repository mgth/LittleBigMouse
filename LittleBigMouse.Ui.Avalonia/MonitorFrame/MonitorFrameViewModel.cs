using System;
using System.Reactive.Linq;
using Avalonia;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using HLab.Base.Avalonia.Extensions;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using ReactiveUI;
using SkiaSharp;
using static HLab.Sys.Windows.API.WinDef;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

public class MonitorFrameViewModel : ViewModel<PhysicalMonitor>, IMvvmContextProvider, IMonitorFrameViewModel
{
    public MonitorFrameViewModel()
    {
        _rotated = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio,
            e => e.Model.DepthProjection,
            (ratio, mm) => mm.ScaleWithLocation(ratio)
        ).Log(this, "_rotated").ToProperty(this, e => e.Rotated);

        _rotation = this.WhenAnyValue(
            e => e.Model.Orientation,
            e => e.Rotated.Height,
            e => e.Rotated.Width,
            (o, h, w) =>
            {
                if (o == 0) return null;

                var t = new TransformGroup();
                t.Children.Add(new RotateTransform(90 * o));

                //switch (o)
                //{
                //    case 1:
                //        t.Children.Add(new TranslateTransform(w, 0));
                //        break;
                //    case 2:
                //        t.Children.Add(new TranslateTransform(w, h));
                //        break;
                //    case 3:
                //        t.Children.Add(new TranslateTransform(0, h));
                //        break;
                //}
                return t;
            }
        ).Log(this, "_rotation").ToProperty(this, e => e.Rotation);

        _logoPadding = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio.X,
            e => e.MonitorsPresenter.VisualRatio.Y,
            (x, y) => new Thickness(4 * x, 4 * y, 4 * x, 4 * y)
        ).Log(this, "_logoPadding").ToProperty(this, e => e.LogoPadding);

        _left = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio.X,
            e => e.Location.X,
            e => e.MonitorsPresenter.Model.X0,
            e => e.Model.DepthProjection.LeftBorder,

            (rx, x, x0, leftBorder) => rx * (x0 + x - leftBorder)

        ).Log(this, "_left").ToProperty(this, e => e.Left);

        _top = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio.Y,
            e => e.Location.Y,
            e => e.MonitorsPresenter.Model.Y0,
            e => e.Model.DepthProjection.TopBorder,

            (ry, y, y0, topBorder) => ry * (y0 + y - topBorder)

        ).Log(this, "_top").ToProperty(this, e => e.Top);

        _margin = this.WhenAnyValue(
            e => e.Left,
            e => e.Top, (left, top) => new Thickness(Left, Top, 0, 0)
        ).Log(this, "_margin").ToProperty(this, e => e.Margin);

        _unrotated = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio,
            e => e.Model.DepthProjectionUnrotated,
            e => e.Model.Orientation,
            (ratio, mmu, o) => mmu.ScaleWithLocation(ratio)
        ).Log(this, "_unrotated").ToProperty(this, e => e.Unrotated);

        this.WhenAnyValue(
            e => e.Model.ActiveSource.Source.WallpaperPath,
            e => e.MonitorsPresenter.Model.WallpaperStyle)
            .Do(
            e =>
            {
                var path = e.Item1;
                var style = e.Item2;
                //try
                //{
                var wallpaper = new Bitmap(e.Item1);
                IImage result = wallpaper;
                //var wallpaperWidth = wallpaper.PixelSize.Width;
                //var wallpaperHeight = wallpaper.PixelSize.Height;
                var monitorWidth = Model.ActiveSource.Source.InPixel.Width;
                var monitorHeight = Model.ActiveSource.Source.InPixel.Height;


                switch (style)
                {
                    case WallpaperStyle.Fill:
                        {
                            result = new WallpaperRenderer(path)
                            .Fill((int)monitorWidth, (int)monitorHeight)
                            .CropCenter((int)monitorWidth, (int)monitorHeight)
                            .ToBitmap();
                        }
                        break;

                    case WallpaperStyle.Fit:
                        result = new WallpaperRenderer(path)
                            .Fit((int)monitorWidth, (int)monitorHeight)
                            .CropCenter((int)monitorWidth, (int)monitorHeight)
                            .ToBitmap();
                        break;

                    case WallpaperStyle.Stretch:
                        result = new WallpaperRenderer(path)
                            .Stretch((int)monitorWidth, (int)monitorHeight)
                            .CropCenter((int)monitorWidth, (int)monitorHeight)
                            .ToBitmap()
                            ;
                        break;

                    case WallpaperStyle.Tile:
                    
                        result = new WallpaperRenderer(path)
                            .Measure(MonitorsPresenter.Model.AllSources.Items)
                            .MakeTileWall()
                            .Crop(Model.ActiveSource.Source.InPixel)
                            .ToBitmap();
                    
                        break;

                    case WallpaperStyle.Center:

                        // crop the wallpaper to the monitor size
                        result = new WallpaperRenderer(path)
                            .CropCenter((int)monitorWidth, (int)monitorHeight)
                            .ToBitmap();
                        break;

                    case WallpaperStyle.Span:
                        result = new WallpaperRenderer(path)
                            .Measure(MonitorsPresenter.Model.AllSources.Items)
                            .MakeSpanWall()
                            .Crop(Model.ActiveSource.Source.InPixel)
                            .ToBitmap();


                        break;

                    default:
                        throw new ArgumentOutOfRangeException();
                }

                Wallpaper = result;
                //}
                //catch (Exception ex)
                //{
                //}
            }
        ).Log(this, "_wallPaper").Subscribe();


        this
            .WhenAnyValue(e => e.Model)
            .Select(e => e)
            .Do(e => Location = new FrameLocation(e))
            .Subscribe().DisposeWith(this);

        _selected = this.WhenAnyValue(
            e => e.MonitorsPresenter.SelectedMonitor,
            e => e.Model,
            (selected, monitor) => selected?.Model == monitor
            )
            .ToProperty(this, e => e.Selected);

    }

    public IMonitorsLayoutPresenterViewModel? MonitorsPresenter
    {
        get => _monitorsPresenter;
        set => this.RaiseAndSetIfChanged(ref _monitorsPresenter, value);
    }
    IMonitorsLayoutPresenterViewModel? _monitorsPresenter;

    public TransformGroup? Rotation => _rotation.Value;
    readonly ObservableAsPropertyHelper<TransformGroup?> _rotation;

    public Thickness LogoPadding => _logoPadding.Value;
    readonly ObservableAsPropertyHelper<Thickness> _logoPadding;

    public Thickness Margin => _margin.Value;
    readonly ObservableAsPropertyHelper<Thickness> _margin;

    public double Left => _left.Value;
    readonly ObservableAsPropertyHelper<double> _left;

    public double Top => _top.Value;
    readonly ObservableAsPropertyHelper<double> _top;

    public IDisplaySize Rotated => _rotated.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _rotated;

    public IDisplaySize Unrotated => _unrotated.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _unrotated;

    public bool Selected => _selected.Value;
    readonly ObservableAsPropertyHelper<bool> _selected;

    //public Bitmap? Wallpaper => _wallpaper.Value;
    //readonly ObservableAsPropertyHelper<Bitmap?> _wallpaper;

    public IImage Wallpaper
    {
        get => _wallpaper;
        set => this.RaiseAndSetIfChanged(ref _wallpaper, value);
    }
    IImage _wallpaper;

    public Stretch WallpaperStretch
    {
        get => _wallpaperStretch;
        set => this.RaiseAndSetIfChanged(ref _wallpaperStretch, value);
    }
    Stretch _wallpaperStretch = Stretch.UniformToFill;


    public IFrameLocation Location
    {
        get => _location;
        set => this.RaiseAndSetIfChanged(ref _location, value);
    }
    IFrameLocation _location;

    public void ConfigureMvvmContext(IMvvmContext ctx)
    {
        ctx.AddCreator<IScreenContentViewModel>(e => e.ScreenFrameViewModel = this);
    }
}