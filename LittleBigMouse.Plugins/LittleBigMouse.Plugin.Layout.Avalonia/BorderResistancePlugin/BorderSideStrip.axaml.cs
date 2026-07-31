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
    /// <summary>
    /// The gesture in progress. Rebuilt whenever the band is bound to another edge,
    /// since it edits through that edge's view model.
    /// </summary>
    BorderSectionGesture? _gesture;

    BorderSideViewModel? _gestureSide;

    BorderSectionGesture Gesture(BorderSideViewModel vm)
    {
        if (_gesture == null || !ReferenceEquals(_gestureSide, vm))
        {
            _gesture = new BorderSectionGesture(vm);
            _gestureSide = vm;
        }

        return _gesture;
    }

    /// <summary>Our layer in the presenter's shared panel, holding the reference line.</summary>
    Canvas? _reference;

    public BorderSideStrip()
    {
        InitializeComponent();

        // Focusable so Delete reaches the band that was last clicked, on the layout
        // map and on the real screen alike.
        Focusable = true;

        SizeChanged += (_, _) => UpdateGeometry();
        DataContextChanged += (_, _) => UpdateGeometry();
    }

    // The guide is published for the edge, not owned by the band that draws it: the
    // bands on the real screens come and go with the overlay, and one of them may be
    // the very one leading the gesture the layout map has to show.
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);

        BorderSectionGuide.Changed += OnGuideChanged;
        OnGuideChanged(BorderSectionGuide.Current);
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        BorderSectionGuide.Changed -= OnGuideChanged;
        ClearGuide();

        base.OnDetachedFromVisualTree(e);
    }

    /// <summary>
    /// Double click takes all the free room there is: a section grows until it meets
    /// its neighbours, and empty space becomes a section filling the same stretch.
    /// One gesture, one meaning, whether or not it lands on something.
    /// </summary>
    protected override void OnDoubleTapped(TappedEventArgs e)
    {
        var vm = ViewModel;
        if (vm == null) return;

        var mm = AlongMm(e.GetPosition(this));

        var target = vm.Sections.FirstOrDefault(s => mm >= s.Model.From && mm < s.Model.To);
        if (target != null)
        {
            vm.Expand(target.Model);
            SelectInParent(target);
        }
        else
        {
            var created = vm.CreateFilling(mm);
            if (created != null)
            {
                SelectInParent(vm.Sections.FirstOrDefault(s => ReferenceEquals(s.Model, created)));
            }
        }

        (this.GetLayout() ?? vm.Monitor.Layout)?.Compact();
        e.Handled = true;
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
    double AlongMm(PointerEventArgs e) => AlongMm(e.GetPosition(this));

    double AlongMm(Point p)
    {
        var vm = ViewModel;
        if (vm == null) return 0;

        return vm.ToMm(vm.IsVertical ? p.Y : p.X);
    }

    static bool SnapEnabled(KeyModifiers modifiers) => (modifiers & KeyModifiers.Control) == 0;

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        var vm = ViewModel;
        if (vm == null) return;
        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed) return;

        var gesture = Gesture(vm);
        var target = gesture.Press(AlongMm(e), SnapEnabled(e.KeyModifiers));

        SelectInParent(target == null
            ? null
            : vm.Sections.FirstOrDefault(s => ReferenceEquals(s.Model, target)));

        Focus();
        e.Pointer.Capture(this);
        e.Handled = true;
    }

    protected override void OnPointerMoved(PointerEventArgs e)
    {
        var vm = ViewModel;
        if (vm == null || _gesture is not { Active: true } gesture) return;
        if (e.Pointer.Captured != this) return;

        var snap = SnapEnabled(e.KeyModifiers);

        Render(vm, gesture.Move(AlongMm(e), snap), snap);

        e.Handled = true;
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        var vm = ViewModel;

        Overlay.Children.Clear();
        BorderSectionGuide.Clear();

        var created = _gesture?.Release(AlongMm(e), SnapEnabled(e.KeyModifiers));

        if (vm != null && created != null)
        {
            foreach (var section in vm.Sections)
            {
                if (!ReferenceEquals(section.Model, created)) continue;
                SelectInParent(section);
                break;
            }
        }

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

    /// <summary>
    /// Turn what the gesture reports into the outline, and publish the guide. The
    /// gesture has already applied its edit; this only says so on screen.
    /// </summary>
    void Render(BorderSideViewModel vm, BorderGestureFeedback feedback, bool snap)
    {
        if (feedback is { PreviewFromMm: { } from, PreviewToMm: { } to })
        {
            DrawPreview(vm, from, to);
            // A section is swept out from a fixed anchor, so only the moving end
            // can catch anything.
            PublishGuide(vm, snap, to);
            return;
        }

        // A moved section keeps its length, so at most one of its ends can have
        // caught a target; the far one is tried when the near one found nothing.
        PublishGuide(vm, snap, feedback.GuideMm, feedback.SecondGuideMm);
    }

    /// <summary>
    /// Say where the boundary caught something, for every band showing this edge —
    /// this one included, which draws it from the same notification as the others.
    /// </summary>
    void PublishGuide(BorderSideViewModel vm, bool snap, params double?[] candidates)
    {
        if (snap)
        {
            foreach (var candidate in candidates)
            {
                if (candidate is not { } mm) continue;
                if (vm.MatchedTarget(mm, _gesture?.Target) is not { } target) continue;

                // Also in layout coordinates: the bands match this guide by their
                // own edge, the screens by where it falls across them.
                BorderSectionGuide.Show(vm.Side, mm, target.Kind, vm.OriginMm + mm, vm.IsVertical);
                return;
            }
        }

        BorderSectionGuide.Clear();
    }

    void OnGuideChanged(BorderGuide? guide)
    {
        var vm = ViewModel;

        ClearGuide();

        if (vm == null || guide is not { } g) return;
        if (!ReferenceEquals(g.Side, vm.Side)) return;

        DrawGuide(vm, g.Mm, g.Kind);
    }

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
    }

    /// <summary>
    /// Mark a boundary that landed exactly on a snap target — the same visual
    /// language as the anchor lines shown when dragging a monitor.
    /// </summary>
    /// <summary>
    /// Carry the guide across the whole layout, towards whatever the boundary lined
    /// up with. The band alone can only say "this caught something"; the line running
    /// through the neighbouring screens is what shows it met the section over there,
    /// or that screen's edge.
    /// <para>
    /// Drawn into the presenter's panel, the one surface spanning every monitor —
    /// the same one the anchor lines use when a screen is dragged around. A band
    /// clips itself to its mitred shape and a monitor's overlay stops at that
    /// monitor, so neither can reach past its own screen.
    /// </para>
    /// <para>
    /// The bands live on real screens too, one window each, where no such surface
    /// exists — there the colour carries the whole message.
    /// </para>
    /// </summary>
    void ShowReference(BorderSideViewModel vm, double at, SnapKind kind)
    {
        ClearReference();

        // The edge's own ends need no explaining: the band already ends there.
        if (kind == SnapKind.EdgeEnd) return;

        var panel = this.GetPresenter()?.MainPanel;
        if (panel == null) return;

        // Let the framework map the band's coordinate into the shared surface rather
        // than re-deriving it from ratios and origins.
        var here = vm.IsVertical ? new Point(0, at) : new Point(at, 0);
        if (this.TranslatePoint(here, panel) is not { } origin) return;

        _reference = new Canvas { IsHitTestVisible = false };
        _reference.Children.Add(new Line
        {
            StartPoint = vm.IsVertical ? new Point(0, origin.Y) : new Point(origin.X, 0),
            EndPoint = vm.IsVertical
                ? new Point(panel.Bounds.Width, origin.Y)
                : new Point(origin.X, panel.Bounds.Height),
            Stroke = SnapGuideBrushes.For(kind),
            StrokeThickness = 1,
            StrokeDashArray = [4, 4],
            Opacity = 0.85
        });

        panel.Children.Add(_reference);
    }

    void ClearReference()
    {
        if (_reference == null) return;

        (_reference.Parent as Panel)?.Children.Remove(_reference);
        _reference = null;
    }

    void ClearGuide()
    {
        Guides.Children.Clear();
        ClearReference();
    }

    /// <summary>
    /// Mark a boundary that landed on a snap target — the same visual language as
    /// the anchor lines shown when dragging a monitor. Drawn on every band showing
    /// this edge, whichever one the hand is on.
    /// </summary>
    void DrawGuide(BorderSideViewModel vm, double mm, SnapKind kind)
    {
        var at = vm.ToPixels(mm);

        // Across the band, in the colour of whatever the boundary caught.
        Guides.Children.Add(new Line
        {
            StartPoint = vm.IsVertical ? new Point(0, at) : new Point(at, 0),
            EndPoint = vm.IsVertical
                ? new Point(vm.PixelThickness, at)
                : new Point(at, vm.PixelThickness),
            Stroke = SnapGuideBrushes.For(kind),
            StrokeThickness = 3
        });

        ShowReference(vm, at, kind);
    }
}
