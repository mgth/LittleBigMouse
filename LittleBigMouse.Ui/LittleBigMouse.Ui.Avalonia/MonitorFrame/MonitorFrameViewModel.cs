using System;
using System.Reactive.Linq;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Media;
using HLab.Base.Avalonia.Extensions;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using ReactiveUI;
using Color = Avalonia.Media.Color;

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
            e => e.Model.ActiveSource.Source.Orientation,
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
            e => e.Model.ActiveSource.Source.Orientation,
            (ratio, mmu, o) => mmu.ScaleWithLocation(ratio)
        ).Log(this, "_unrotated").ToProperty(this, e => e.Unrotated);

        var cmd = ReactiveCommand.CreateFromTask<(string, WallpaperStyle, Color)>(p => SetWallpaper(p.Item1, p.Item2, p.Item3));

        this.WhenAnyValue(
            e => e.Model.ActiveSource.Source.WallpaperPath,
            e => e.Model.ActiveSource.Source.WallpaperStyle,
            e => e.Model.ActiveSource.Source.BackgroundColor)
            .InvokeCommand(cmd).DisposeWith(this);

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

        Disposer.OnDispose(() =>
        {
            if (_wallpaper is IDisposable bmp)
            {
                bmp.Dispose();
            }
            _wallpaper = null;
        });
    }

    Rect GetBounds(int shrink)
    {
        var r = new Rect();
        foreach (var s in Model.Layout.PhysicalSources)
        {
            r = r.Union(s.Source.InPixel.Bounds);
        }

        return new Rect(r.X/shrink,r.Y/shrink,r.Width/shrink,r.Height/shrink);
    }

    async Task SetWallpaper(string path, WallpaperStyle style, Color color)
    {
        if(string.IsNullOrWhiteSpace(path))
        {
            Wallpaper = null;
            return;
        }

        //All dimensions are divided by this value to reduce memory usage
        const int shrink = 4;

        var r = Model.ActiveSource.Source.InPixel.Bounds;


        if (r.Width < shrink || r.Height < shrink)
        {
            Wallpaper = null;
            return;
        }

        var monitor = new Rect(r.X/shrink,r.Y/shrink,r.Width/shrink,r.Height/shrink);

        Wallpaper = style switch 
        {
            WallpaperStyle.Fill => await WallpaperRendererHelper.GetWallpaperFillAsync(path, monitor.Size, shrink),

            WallpaperStyle.Fit => await WallpaperRendererHelper.GetWallpaperFitAsync(path, monitor.Size, color, shrink),

            WallpaperStyle.Stretch => await WallpaperRendererHelper.GetWallpaperStretchAsync(path, monitor.Size, shrink),

            WallpaperStyle.Tile =>  await WallpaperRendererHelper.GetWallpaperTileAsync(path, monitor, GetBounds(shrink), shrink),

            WallpaperStyle.Center => await WallpaperRendererHelper.GetWallpaperCenterAsync(path, monitor.Size, color, shrink),

            WallpaperStyle.Span => await WallpaperRendererHelper.GetWallpaperSpanAsync(path, monitor, GetBounds(shrink), shrink),

            _ => throw new ArgumentOutOfRangeException()
        };

        #if DEBUG
        WallpaperRendererHelper.ImageSharpDebugStats();
        #endif
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

    public IImage? Wallpaper
    {
        get => _wallpaper;
        set => this.RaiseAndSetIfChanged(ref _wallpaper, value);
    }
    IImage? _wallpaper;

    public IFrameLocation Location
    {
        get => _location;
        set => this.RaiseAndSetIfChanged(ref _location, value);
    }
    IFrameLocation _location;

    public void ConfigureMvvmContext(IMvvmContext ctx)
    {
        ctx.AddCreator<IScreenContentViewModel>(e => e.MonitorFrameViewModel = this);
    }
}