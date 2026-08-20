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

    /// <summary>
    /// The window this band fills, when it is on a real screen rather than on the
    /// layout map — set by that window, since it is the only one that knows.
    /// <para>
    /// Not looked up through the tree: the visual root of a band is a
    /// <c>TopLevelHost</c> either way, so a band cannot tell from below whether it
    /// is a window of its own or one of four siblings in the map.
    /// </para>
    /// </summary>
    public EdgeOverlayWindow? Host { get; set; }

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
        if (e.Key == Key.Escape)
        {
            // Only while something is going on: otherwise Escape belongs to whatever
            // is above, which may want to close the overlay it is shown in.
            if (_gesture is not { Active: true }) return;

            CancelGesture();
            e.Handled = true;
            return;
        }

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
    /// overlay. The corner squares the four bands share out are cut by
    /// <see cref="BorderBandShape"/>.
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

        var points = BorderBandShape.For(vm.Kind, w, h, vm.PixelThickness);

        // Backdrop.Data drives hit testing; Clip additionally trims a section drawn
        // right up to a corner so it follows the mitre instead of squaring it off.
        Backdrop.Data = new PolylineGeometry(points, true);
        Clip = new PolylineGeometry(points, true);

        // On a real screen this band IS a window, and a window catches presses over
        // its whole rectangle however its content is clipped — so the half of each
        // corner it gave up would swallow what belongs to the band next to it. Give
        // the window the same shape. On the layout map the bands are siblings in one
        // window and hit testing already sorts the corners out.
        Host?.ShapeTo(points);

        foreach (var section in vm.Sections) section.Refresh();
    }

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

        Render(vm, gesture.Move(AlongMm(e), SnapEnabled(e.KeyModifiers)));

        e.Handled = true;
    }

    /// <summary>
    /// The pointer was taken away mid-gesture — another window came up, or the device
    /// went. The section goes back where it was found: losing the pointer is not a
    /// decision to keep it wherever it happened to be at that instant.
    /// <para>
    /// Every ordinary release comes through here too, since letting go of the button
    /// drops the capture — but only after the release itself: Avalonia raises
    /// PointerReleased and clears the capture in that call's finally, in that order.
    /// So the gesture has already ended by then, and cancelling an ended gesture does
    /// nothing. Were it the other way round, this would undo every drag ever made.
    /// </para>
    /// </summary>
    protected override void OnPointerCaptureLost(PointerCaptureLostEventArgs e)
    {
        CancelGesture();
        base.OnPointerCaptureLost(e);
    }

    /// <summary>
    /// Drop the gesture and take back what it was showing. The edge is left exactly
    /// as the press found it, so there is nothing to compact or push.
    /// </summary>
    void CancelGesture()
    {
        if (_gesture is not { Active: true } gesture) return;

        gesture.Cancel();
        ClearFeedback();
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        var vm = ViewModel;

        ClearFeedback();

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
    /// gesture has already applied its edit and already decided which boundary — if
    /// any — caught a target; this only says so on screen.
    /// <para>
    /// The guide is published rather than drawn here: every band showing this edge
    /// draws it from that notification, this one included.
    /// </para>
    /// </summary>
    void Render(BorderSideViewModel vm, BorderGestureFeedback feedback)
    {
        if (feedback.Preview is { } preview) DrawPreview(vm, preview);

        if (feedback.Guide is not { } guide)
        {
            BorderSectionGuide.Clear();
            return;
        }

        // Also in layout coordinates: the bands match this guide by their own edge,
        // the screens by where it falls across them.
        BorderSectionGuide.Show(
            vm.Side, guide.Mm, guide.Kind, vm.OriginMm + guide.Mm, vm.IsVertical);
    }

    /// <summary>Take back everything a gesture in progress was showing.</summary>
    void ClearFeedback()
    {
        Overlay.Children.Clear();
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

    void DrawPreview(BorderSideViewModel vm, BorderSpan span)
    {
        Overlay.Children.Clear();

        // The span runs from the anchor towards the pointer, so it may well be
        // backwards.
        var from = vm.ToPixels(System.Math.Min(span.From, span.To));
        var length = vm.ToPixels(System.Math.Abs(span.Length));

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
