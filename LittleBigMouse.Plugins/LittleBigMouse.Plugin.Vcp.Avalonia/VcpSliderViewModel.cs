using System.ComponentModel;
using Avalonia.Media;
using HLab.ColorTools;
using HLab.ColorTools.Avalonia;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.MonitorVcp;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpSliderViewModel : ViewModel<MonitorLevel>
{
    public VcpSliderViewModel()
    {
        _background = this.WhenAnyValue(
            e => e.Model.Moving,
            moving => moving ? Brushes.Orange : Brushes.LightGreen
        ).ToProperty(this, e => e.Background);

        _sliderBackground = this.WhenAnyValue(
            e => e.Model.Component,
            c => GetBackground(GetColor(c))
        ).ToProperty(this, e => e.SliderBackground);

        _sliderForeground = this.WhenAnyValue(
            e => e.Model.Component,
            c => GetForeground(GetColor(c))
        ).ToProperty(this, e => e.SliderForeground);

        _sliderBorderBrush = this.WhenAnyValue(
            e => e.Model.Component,
            c => GetBorderBrush(GetColor(c))
        ).ToProperty(this, e => e.SliderBorderBrush);
    }

    public IBrush Background => _background.Value;
    readonly ObservableAsPropertyHelper<IBrush> _background;
    public IBrush SliderBackground => _sliderBackground.Value;
    readonly ObservableAsPropertyHelper<IBrush> _sliderBackground;
    public IBrush SliderForeground => _sliderForeground.Value;
    readonly ObservableAsPropertyHelper<IBrush> _sliderForeground;
    public IBrush SliderBorderBrush => _sliderBorderBrush.Value;

    public void Up()
    {
        if(Model == null) return;
        if(Model.Value < Model.Max)
            Model.Value++;
    }

    public void Down()
    {
        if(Model == null) return;
        if(Model.Value > Model.Min)
            Model.Value--;
    }

    readonly ObservableAsPropertyHelper<IBrush> _sliderBorderBrush;

    static Color GetColor(VcpComponent component) =>
        component switch
        {
            VcpComponent.Red => Color.FromArgb(255, 255, 0, 0),
            VcpComponent.Green => Color.FromArgb(255, 0, 255, 0),
            VcpComponent.Blue => Color.FromArgb(255, 0, 0, 255),
            VcpComponent.Brightness => Color.FromArgb(255, 255, 255, 255),
            VcpComponent.Contrast => Color.FromArgb(255, 30, 30, 30),
            VcpComponent.None => Color.FromArgb(255, 128, 128, 128),
            _ => throw new ArgumentOutOfRangeException(nameof(Component), component, null)
        };

    //static Brush GetForeground(Color c) =>
    //    new SolidColorBrush(
    //        ColorDouble.FromArgb(c.A * 0.7, c.R* 0.8 , c.G* 0.8, c.B * 0.8).ToColor()
    //    );
    static Brush GetForeground(Color c) =>
        new SolidColorBrush(
            c.ToColor<double>().ToAvaloniaColor()
        );

    static Brush GetBackground(Color c) =>
        new SolidColorBrush(
//            c.ToColor<double>()
            c.ToColor<double>().ToHSL().Darken(0.2).WithAlpha(0.6).ToAvaloniaColor()
        );

    static Brush GetBorderBrush(Color c) =>
        new SolidColorBrush(
            new ColorRGB<double>(c.A * 0.9, c.R * 0.5+0.5, c.G * 0.5 + 0.5, c.B * 0.5 + 0.5).ToAvaloniaColor()
        );
}