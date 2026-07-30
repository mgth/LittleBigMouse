using System;
using Avalonia.Media;
using Avalonia.Threading;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// One <see cref="BorderSection"/> projected onto its edge strip as a Canvas
/// rectangle — same shape as <c>ProbeStripViewModel</c>, which is how every other
/// overlay in the layout view positions things. The fill says at a glance what the
/// section does.
/// </summary>
public class BorderSectionViewModel : ReactiveObject, IDisposable
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
            .Subscribe(_ =>
            {
                Refresh();
                // Moving a section can take it off the stretch that faces a
                // neighbour, or bring it back onto one.
                this.RaisePropertyChanged(nameof(CanMirror));
            });

        // …and so can moving or resizing any monitor, which is what makes the
        // button follow a rearranged layout instead of going stale.
        side.LayoutGeometryChanged
            .Subscribe(_ => this.RaisePropertyChanged(nameof(CanMirror)));

        OnSelectionChanged(BorderSectionSelection.Current);
        BorderSectionSelection.Changed += OnSelectionChanged;

        this.WhenAnyValue(
                e => e.Model.Move,
                e => e.Model.MoveBlock,
                e => e.Model.Drag,
                e => e.Model.DragBlock)
            .Subscribe(_ =>
            {
                this.RaisePropertyChanged(nameof(Move));
                this.RaisePropertyChanged(nameof(Drag));
                this.RaisePropertyChanged(nameof(Fill));
                this.RaisePropertyChanged(nameof(Description));
            });
    }

    public BorderSection Model { get; }

    /// <summary>
    /// The editor writes through here rather than straight to the model.
    /// <para>
    /// The model floors resistances at zero, but its refusal is announced while the
    /// binding is still writing target to source, and a two-way binding ignores a
    /// source change raised inside its own update — so the box went on displaying a
    /// number the model had already dropped. Announcing it again once that update is
    /// over puts the editor back in step.
    /// </para>
    /// </summary>
    /// <summary>
    /// Whether "Mirror" would do anything: a monitor across this edge, room left on
    /// its facing edge, and more than a sliver of free space to land in.
    /// </summary>
    public bool CanMirror => Side.CanMirror(Model);

    public double Move
    {
        get => Model.Move;
        set => Write(value, v => Model.Move = v, () => Model.Move, nameof(Move));
    }

    public double Drag
    {
        get => Model.Drag;
        set => Write(value, v => Model.Drag = v, () => Model.Drag, nameof(Drag));
    }

    void Write(double requested, Action<double> apply, Func<double> read, string property)
    {
        apply(requested);
        if (read().Equals(requested)) return;

        Dispatcher.UIThread.Post(() => this.RaisePropertyChanged(property));
    }

    public BorderSideViewModel Side { get; }

    /// <summary>Distance in UI pixels from the edge's starting corner.</summary>
    public double Offset => Side.ToPixels(Model.From);

    /// <summary>Extent in UI pixels along the edge.</summary>
    public double Length => Side.ToPixels(Model.To - Model.From);

    public double Left => Side.IsVertical ? 0 : Offset;
    public double Top => Side.IsVertical ? Offset : 0;
    public double Width => Side.IsVertical ? Side.PixelThickness : Length;
    public double Height => Side.IsVertical ? Length : Side.PixelThickness;

    /// <summary>
    /// Derived from the shared selection, so the same section lights up in the map
    /// and on the screen at once, whichever one was clicked.
    /// </summary>
    public bool IsSelected
    {
        get;
        private set
        {
            this.RaiseAndSetIfChanged(ref field, value);
            this.RaisePropertyChanged(nameof(Stroke));
        }
    }

    void OnSelectionChanged(BorderSection? selected) =>
        IsSelected = ReferenceEquals(selected, Model);

    public void Dispose() => BorderSectionSelection.Changed -= OnSelectionChanged;

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
