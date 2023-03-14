/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

//#define uglyfix

using System;
using Avalonia;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Plugins;
using ReactiveUI;
using Brush = Avalonia.Media.Brush;
using Color = Avalonia.Media.Color;

namespace LittleBigMouse.Ui.Avalonia.ScreenFrame;

public class MonitorFrameViewModelDesign : MonitorFrameViewModel
{

}


public class MonitorFrameViewModel : ViewModel<Monitor>, IMvvmContextProvider, IMonitorFrameViewModel
{
    public MonitorFrameViewModel()
    {
        _rotated = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio,
            e => e.Model.InMm,
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

            switch (o)
            {
                case 1:
                    t.Children.Add(new TranslateTransform(w, 0));
                    break;
                case 2:
                    t.Children.Add(new TranslateTransform(w, h));
                    break;
                case 3:
                    t.Children.Add(new TranslateTransform(0, h));
                    break;
            }
            return t;
        }
        ).Log(this, "_rotation").ToProperty(this, e => e.Rotation);

        _logoPadding = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio.X,
            e => e.MonitorsPresenter.VisualRatio.Y,
            (x, y) => new Thickness(4 * x, 4 * y, 4 * x, 4 * y)
        ).Log(this, "_logoPadding").ToProperty(this, e => e.LogoPadding);

        _margin = this.WhenAnyValue(
            e => e.Left,
            e => e.Top, (left, top) => new Thickness(Left, Top, 0, 0)
            ).Log(this, "_margin").ToProperty(this, e => e.Margin, deferSubscription: true);

        _left = this.WhenAnyValue(
            e => e.MonitorsPresenter.VisualRatio.X,
            e => e.Model.XMoving,
            e => e.Model.Layout.X0,
            e => e.Model.InMm.LeftBorder,

                (x, xMoving, x0, leftBorder) =>
                    x * (x0 + xMoving - leftBorder)

                    ).Log(this, "_left").ToProperty(this, e => e.Left);

        _top = this.WhenAnyValue(
    e => e.MonitorsPresenter.VisualRatio.Y,
    e => e.Model.YMoving,
    e => e.Model.Layout.Y0,
    e => e.Model.InMm.TopBorder,

    (y, yMoving, y0, topBorder) =>
        y * (y0 + yMoving - topBorder)


        ).Log(this, "_top").ToProperty(this, e => e.Top);


        _unrotated = this.WhenAnyValue(
    e => e.MonitorsPresenter.VisualRatio,
    e => e.Model.InMmU,
    e => e.Model.Orientation,
    (ratio, mmu, o) => mmu.ScaleWithLocation(ratio)
             ).Log(this, "_unrotated").ToProperty(this, e => e.Unrotated);

        _wallPaperStretch = this.WhenAnyValue(
            _ => _.Model.Layout.WallpaperStyle,

            selector: style => (Stretch)(style switch
            {
                0 => Stretch.None,
                2 => Stretch.Fill,
                6 => Stretch.Uniform,
                10 => Stretch.UniformToFill,
                22 => // stretched across all screens
                    Stretch.None,
                _ => Stretch.None
            })
            ).Log(this, "_wallPaperStretch").ToProperty(this, e => e.WallPaperStretch);

        _wallPaper = this.WhenAnyValue(
e => e.Model.ActiveSource.Device.WallpaperPath,
            (path) =>
            {
                try
                {
                    return new Bitmap(path);
                }
                catch (Exception)
                {
                    return null;
                }
            }
            ).Log(this, "_wallPaper").ToProperty(this, _ => _.WallPaper);

        _backgroundColor = this.WhenAnyValue(
        _ => _.Model.Layout.BackgroundColor,
        color => new SolidColorBrush(
            Color.FromRgb(
                (byte)color[0],
                (byte)color[1],
                (byte)color[2]))
        ).Log(this, "_backgroundColor").ToProperty(this, _ => _.BackgroundColor);

        _logo = this.WhenAnyValue(
    _ => _.Model.ActiveSource.Device.Edid.ManufacturerCode,
    _ => _.Model.ActiveSource.Device.AttachedDevice.Parent.DeviceString,
            GetLogo
        ).Log(this, "_logo").ToProperty(this, _ => _.Logo);

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

    public Stretch WallPaperStretch => _wallPaperStretch.Value;
    readonly ObservableAsPropertyHelper<Stretch> _wallPaperStretch;

    public Bitmap WallPaper => _wallPaper.Value;
    readonly ObservableAsPropertyHelper<Bitmap> _wallPaper;

    public Brush BackgroundColor => _backgroundColor.Value;
    readonly ObservableAsPropertyHelper<Brush> _backgroundColor;

    public string Logo => _logo.Value;

    readonly ObservableAsPropertyHelper<string> _logo;

    static string GetLogo(string code, string? dev)
    {
        if (dev != null && dev.ToLower().Contains("spacedesk")) return "icon/Pnp/Spacedesk";

        return $"icon/Pnp/{code}";
    }

    public void ConfigureMvvmContext(IMvvmContext ctx)
    {
        ctx.AddCreator<IScreenContentViewModel>(e => e.ScreenFrameViewModel = this);
    }
}