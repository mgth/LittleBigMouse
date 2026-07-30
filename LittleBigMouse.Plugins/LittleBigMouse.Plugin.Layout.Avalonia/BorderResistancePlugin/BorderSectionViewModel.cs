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
    static readonly IBrush SelectedStroke = Brushes.White;
    static readonly IBrush NormalStroke = Brushes.Transparent;

    public BorderSectionViewModel(BorderSection model, BorderSideViewModel side)
    {
        Model = model;
        Side = side;

        // Everything is driven from the model, never pushed in by whoever made the
        // edit. The same section is shown by more than one view model at a time —
        // the layout map and each on-screen band — and only the one performing a
        // drag would know to refresh itself; the others would keep drawing the
        // section where it used to be.
        this.WhenAnyValue(e => e.Model.From, e => e.Model.To)
            .Subscribe(_ => Refresh());

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

    public IBrush Fill => BorderSectionBrushes.For(Model);

    public string Description =>
        BorderSectionBrushes.Describe(Model.Move, Model.MoveBlock, Model.Drag, Model.DragBlock);

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
