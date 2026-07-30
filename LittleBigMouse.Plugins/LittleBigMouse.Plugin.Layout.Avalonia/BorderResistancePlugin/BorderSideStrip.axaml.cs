using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.VisualTree;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Plugins.Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The interactive band along one monitor edge.
/// <para>
/// Drag on empty space to draw a section, drag a section's ends to resize it, drag
/// its middle to move it. Ctrl suspends snapping, exactly as it does when dragging a
/// monitor around (see <c>MonitorLocationView</c>), so the two gestures behave the
/// same way.
/// </para>
/// </summary>
public partial class BorderSideStrip : UserControl
{
    /// <summary>How close to an end counts as grabbing its handle.</summary>
    const double HandlePixels = 6;

    enum Mode { None, Draw, ResizeFrom, ResizeTo, Move }

    Mode _mode = Mode.None;
    BorderSection? _target;
    double _anchorMm;
    double _lastMm;

    public BorderSideStrip()
    {
        InitializeComponent();

        // Focusable so Delete reaches the band that was last clicked, on the layout
        // map and on the real screen alike.
        Focusable = true;

        SizeChanged += (_, _) => UpdateGeometry();
        DataContextChanged += (_, _) => UpdateGeometry();
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        var vm = ViewModel;
        if (e.Key != Key.Delete || vm == null) return;

        var selected = BorderSectionSelection.Current;
        if (selected == null) return;

        // Only the band actually holding the selection acts, so the key does not
        // delete twice when several bands have been shown for the same edge.
        if (!vm.Side.Sections.Items.Any(s => ReferenceEquals(s, selected))) return;

        vm.Delete(selected);
        BorderSectionSelection.Select(null);
        (this.GetLayout() ?? vm.Monitor.Layout)?.Compact();

        e.Handled = true;
    }

    BorderSideViewModel? ViewModel => DataContext as BorderSideViewModel;

    /// <summary>
    /// Publish the strip's own size as the edge's scale, and mitre the corners.
    /// <para>
    /// The band spans its edge end to end, so its length IS the edge length — which
    /// is why the millimetre scale is taken from here rather than from the parent
    /// overlay. The four bands therefore overlap in the corner squares; each one
    /// keeps the triangle on its own side of the square's diagonal, so the corners
    /// meet like a picture frame and a click there reaches exactly one band.
    /// </para>
    /// </summary>
    void UpdateGeometry()
    {
        var vm = ViewModel;
        if (vm == null) return;

        var w = Bounds.Width;
        var h = Bounds.Height;
        if (w <= 0 || h <= 0) return;

        vm.PixelLength = vm.IsVertical ? h : w;
        vm.PixelThickness = vm.IsVertical ? w : h;

        // Thickness of the bands meeting this one at right angles; they are all the
        // same width, so the corner square is t by t.
        var t = vm.PixelThickness;

        var points = vm.IsVertical
            ? Corners(vm, w, h, t)
            : Horizontal(vm, w, h, t);

        // Backdrop.Data drives hit testing; Clip additionally trims a section drawn
        // right up to a corner so it follows the mitre instead of squaring it off.
        Backdrop.Data = new PolylineGeometry(points, true);
        Clip = new PolylineGeometry(points, true);

        foreach (var section in vm.Sections) section.Refresh();
    }

    static Points Corners(BorderSideViewModel vm, double w, double h, double t) =>
        vm.Kind == BorderSideKind.Left
            // Outer edge on the left: keep the lower-left half of each corner square.
            ? [new Point(0, 0), new Point(t, t), new Point(t, h - t), new Point(0, h)]
            // Outer edge on the right.
            : [new Point(w, 0), new Point(w, h), new Point(0, h - t), new Point(0, t)];

    static Points Horizontal(BorderSideViewModel vm, double w, double h, double t) =>
        vm.Kind == BorderSideKind.Top
            ? [new Point(0, 0), new Point(w, 0), new Point(w - t, t), new Point(t, t)]
            : [new Point(0, h), new Point(t, 0), new Point(w - t, 0), new Point(w, h)];

    /// <summary>Position along the edge, in millimetres from its starting corner.</summary>
    double AlongMm(PointerEventArgs e)
    {
        var vm = ViewModel;
        if (vm == null) return 0;

        var p = e.GetPosition(this);
        return vm.ToMm(vm.IsVertical ? p.Y : p.X);
    }

    static bool SnapEnabled(KeyModifiers modifiers) => (modifiers & KeyModifiers.Control) == 0;

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        var vm = ViewModel;
        if (vm == null) return;
        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed) return;

        var mm = AlongMm(e);
        var handle = vm.ToMm(HandlePixels);

        _target = null;
        _mode = Mode.Draw;

        foreach (var section in vm.Sections)
        {
            var model = section.Model;
            if (mm < model.From - handle || mm > model.To + handle) continue;

            _target = model;
            SelectInParent(section);

            if (mm <= model.From + handle) _mode = Mode.ResizeFrom;
            else if (mm >= model.To - handle) _mode = Mode.ResizeTo;
            else _mode = Mode.Move;

            break;
        }

        if (_mode == Mode.Draw) SelectInParent(null);

        _anchorMm = vm.Snap(mm, SnapEnabled(e.KeyModifiers));
        _lastMm = mm;

        Focus();
        e.Pointer.Capture(this);
        e.Handled = true;
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        var vm = ViewModel;
        if (vm == null || _mode == Mode.None) return;
        if (e.Pointer.Captured != this) return;

        var snap = SnapEnabled(e.KeyModifiers);
        var raw = AlongMm(e);
        var mm = vm.Snap(raw, snap);

        switch (_mode)
        {
            case Mode.Draw:
                DrawPreview(vm, _anchorMm, mm);
                break;

            case Mode.ResizeFrom when _target != null:
                vm.Resize(_target, mm, _target.To);
                ShowGuide(vm, mm, snap);
                break;

            case Mode.ResizeTo when _target != null:
                vm.Resize(_target, _target.From, mm);
                ShowGuide(vm, mm, snap);
                break;

            case Mode.Move when _target != null:
                vm.MoveBy(_target, raw - _lastMm);
                _lastMm = raw;
                ShowGuide(vm, _target.From, snap);
                break;
        }

        e.Handled = true;
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        var vm = ViewModel;
        Overlay.Children.Clear();

        if (vm != null && _mode == Mode.Draw)
        {
            var mm = vm.Snap(AlongMm(e), SnapEnabled(e.KeyModifiers));
            var created = vm.Create(_anchorMm, mm);

            if (created != null)
            {
                foreach (var section in vm.Sections)
                {
                    if (!ReferenceEquals(section.Model, created)) continue;
                    SelectInParent(section);
                    break;
                }
            }
        }

        _mode = Mode.None;
        _target = null;
        e.Pointer.Capture(null);

        // The layout has to be recompacted and re-pushed like any other edit. On the
        // real screen this strip has no presenter above it, so fall back to the
        // monitor's own layout rather than silently skipping the push.
        (this.GetLayout() ?? vm?.Monitor.Layout)?.Compact();
    }

    // Selection goes through the shared holder rather than an ancestor editor: on a
    // real screen this band has none, which is why clicking a section there used to
    // select nothing.
    static void SelectInParent(BorderSectionViewModel? section) =>
        BorderSectionSelection.Select(section?.Model);

    //==================//
    // Feedback         //
    //==================//

    void DrawPreview(BorderSideViewModel vm, double fromMm, double toMm)
    {
        Overlay.Children.Clear();

        var from = vm.ToPixels(System.Math.Min(fromMm, toMm));
        var length = vm.ToPixels(System.Math.Abs(toMm - fromMm));

        var preview = new Rectangle
        {
            Width = vm.IsVertical ? vm.PixelThickness : length,
            Height = vm.IsVertical ? length : vm.PixelThickness,
            Fill = new SolidColorBrush(Color.FromArgb(0x60, 0xFF, 0xFF, 0xFF)),
            Stroke = Brushes.White,
            StrokeThickness = 1
        };

        Canvas.SetLeft(preview, vm.IsVertical ? 0 : from);
        Canvas.SetTop(preview, vm.IsVertical ? from : 0);
        Overlay.Children.Add(preview);

        ShowGuide(vm, toMm, snap: true, keep: true);
    }

    /// <summary>
    /// Mark a boundary that landed exactly on a snap target — the same visual
    /// language as the anchor lines shown when dragging a monitor.
    /// </summary>
    void ShowGuide(BorderSideViewModel vm, double mm, bool snap, bool keep = false)
    {
        if (!keep) Overlay.Children.Clear();
        if (!snap) return;

        var onTarget = vm.SnapTargetsMm().Any(t => System.Math.Abs(t - mm) < 0.001);
        if (!onTarget) return;

        var at = vm.ToPixels(mm);

        var line = new Line
        {
            StartPoint = vm.IsVertical ? new Point(0, at) : new Point(at, 0),
            EndPoint = vm.IsVertical
                ? new Point(vm.PixelThickness, at)
                : new Point(at, vm.PixelThickness),
            Stroke = Brushes.Yellow,
            StrokeThickness = 2
        };

        Overlay.Children.Add(line);
    }
}
