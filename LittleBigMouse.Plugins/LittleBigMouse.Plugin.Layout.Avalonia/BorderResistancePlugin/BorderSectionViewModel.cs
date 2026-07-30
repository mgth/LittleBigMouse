using Avalonia.Media;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// One <see cref="BorderSection"/> projected onto its edge strip as a Canvas
/// rectangle — same shape as <c>ProbeStripViewModel</c>, which is how every other
/// overlay in the layout view positions things. The fill says at a glance what the
/// section does.
/// </summary>
public class BorderSectionViewModel : ReactiveObject
{
    static readonly IBrush BlockedBrush = new SolidColorBrush(Color.FromArgb(0xC0, 0xC0, 0x39, 0x2B));
    static readonly IBrush HalfBlockedBrush = new SolidColorBrush(Color.FromArgb(0xC0, 0xE0, 0x8E, 0x2B));
    static readonly IBrush ResistingBrush = new SolidColorBrush(Color.FromArgb(0xC0, 0x2B, 0x84, 0xC0));
    static readonly IBrush FreeBrush = new SolidColorBrush(Color.FromArgb(0x80, 0x70, 0x70, 0x70));

    static readonly IBrush SelectedStroke = Brushes.White;
    static readonly IBrush NormalStroke = Brushes.Transparent;

    public BorderSectionViewModel(BorderSection model, BorderSideViewModel side)
    {
        Model = model;
        Side = side;

        // The look is derived from four independent settings, so watch them all
        // rather than refreshing only when the geometry changes.
        this.WhenAnyValue(
                e => e.Model.Move,
                e => e.Model.MoveBlock,
                e => e.Model.Drag,
                e => e.Model.DragBlock)
            .Subscribe(_ =>
            {
                this.RaisePropertyChanged(nameof(Fill));
                this.RaisePropertyChanged(nameof(Description));
            });
    }

    public BorderSection Model { get; }

    public BorderSideViewModel Side { get; }

    /// <summary>Distance in UI pixels from the edge's starting corner.</summary>
    public double Offset => Side.ToPixels(Model.From);

    /// <summary>Extent in UI pixels along the edge.</summary>
    public double Length => Side.ToPixels(Model.To - Model.From);

    public double Left => Side.IsVertical ? 0 : Offset;
    public double Top => Side.IsVertical ? Offset : 0;
    public double Width => Side.IsVertical ? Side.PixelThickness : Length;
    public double Height => Side.IsVertical ? Length : Side.PixelThickness;

    public bool IsSelected
    {
        get;
        set
        {
            this.RaiseAndSetIfChanged(ref field, value);
            this.RaisePropertyChanged(nameof(Stroke));
        }
    }

    public IBrush Stroke => IsSelected ? SelectedStroke : NormalStroke;

    public IBrush Fill
    {
        get
        {
            if (Model.MoveBlock && Model.DragBlock) return BlockedBrush;
            if (Model.MoveBlock || Model.DragBlock) return HalfBlockedBrush;
            if (Model.Move > 0 || Model.Drag > 0) return ResistingBrush;
            return FreeBrush;
        }
    }

    public string Description
    {
        get
        {
            var move = Model.MoveBlock ? "blocked" : $"{Model.Move:0.#} mm";
            var drag = Model.DragBlock ? "blocked" : $"{Model.Drag:0.#} mm";
            return $"Move: {move}\nDrag: {drag}";
        }
    }

    /// <summary>Recompute the pixel projection after a move, a resize or a zoom.</summary>
    public void Refresh()
    {
        this.RaisePropertyChanged(nameof(Offset));
        this.RaisePropertyChanged(nameof(Length));
        this.RaisePropertyChanged(nameof(Left));
        this.RaisePropertyChanged(nameof(Top));
        this.RaisePropertyChanged(nameof(Width));
        this.RaisePropertyChanged(nameof(Height));
    }
}
