using System.Reactive.Linq;
using Avalonia.Media;
using Avalonia.Media.Immutable;
using HLab.Mvvm.ReactiveUI;
using HLab.Sys.Windows.MonitorVcp;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

public class VcpSliderViewModel : ViewModel<MonitorLevel>
{
    static readonly Color RedColor = Color.FromRgb(0xE0, 0x52, 0x4E);
    static readonly Color GreenColor = Color.FromRgb(0x55, 0xB9, 0x4F);
    static readonly Color BlueColor = Color.FromRgb(0x4C, 0x8E, 0xDF);

    // All brushes immutable: a mutable SolidColorBrush is thread-affine and the
    // compositor rejects it when the binding update comes from the CommandWorker.
    static readonly IBrush MovingBrush = new ImmutableSolidColorBrush(Color.FromArgb(0x50, 0xEF, 0x9F, 0x27));

    static readonly IBrush RedTint = new ImmutableSolidColorBrush(RedColor, 0.30);
    static readonly IBrush GreenTint = new ImmutableSolidColorBrush(GreenColor, 0.30);
    static readonly IBrush BlueTint = new ImmutableSolidColorBrush(BlueColor, 0.30);

    public VcpSliderViewModel()
    {
        _channelColor = this.WhenAnyValue(
            e => e.Model.Component,
            selector: ColorFor)
            .ToProperty(this, e => e.ChannelColor);

        // Chip: channel tint at rest, amber while the DDC write is in flight.
        // Moving flips on the CommandWorker thread: marshal before the view binds.
        _chipBackground = this.WhenAnyValue(
            e => e.Model.Moving,
            e => e.Model.Component,
            (moving, component) => moving ? MovingBrush : TintFor(component))
            .ObserveOn(RxSchedulers.MainThreadScheduler)
            .ToProperty(this, e => e.ChipBackground);
    }

    public static Color? ColorFor(VcpComponent component) => component switch
    {
        VcpComponent.Red => RedColor,
        VcpComponent.Green => GreenColor,
        VcpComponent.Blue => BlueColor,
        _ => null
    };

    static IBrush TintFor(VcpComponent component) => component switch
    {
        VcpComponent.Red => RedTint,
        VcpComponent.Green => GreenTint,
        VcpComponent.Blue => BlueTint,
        _ => Brushes.Transparent
    };

    /// <summary>Channel color for R/G/B gain and drive levels, null for neutral levels.</summary>
    public Color? ChannelColor => _channelColor.Value;
    readonly ObservableAsPropertyHelper<Color?> _channelColor;

    public IBrush ChipBackground => _chipBackground.Value;
    readonly ObservableAsPropertyHelper<IBrush> _chipBackground;

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
}
