using Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The dashed reference line, on a real screen: one window per monitor, covering
/// it, up only while a boundary is held against a target.
/// <para>
/// <see cref="EdgeOverlayWindow"/> keeps to the edges so the middle of a screen is
/// never covered and clicks reach whatever is underneath. That reasoning does not
/// reach here: this window exists only during a drag, and a drag is precisely when
/// the pointer cannot be anywhere else. Nothing is being covered from anyone.
/// </para>
/// <para>
/// Covering the screen is also what makes the line land where it should. The
/// window is placed and measured once, with the bands; from then on a guide moves
/// a line INSIDE it, at a fraction of its own size. Nothing is converted, nothing
/// is repositioned mid-gesture, and the screen's scaling never enters into it —
/// which is what a window re-placed at every snap could not manage.
/// </para>
/// </summary>
public partial class GuideLineWindow : EdgeOverlayWindow
{
    public GuideLineWindow()
    {
        InitializeComponent();

        // The window is sized twice, the second time when the real scaling is
        // known, so the line follows rather than keeping the hinted size.
        SizeChanged += (_, _) => Draw();
    }

    double _ratio;
    bool _isVertical;

    /// <summary>
    /// Mark a place along the screen, as a fraction of it.
    /// </summary>
    /// <param name="ratio">Where, from the screen's top or left, between 0 and 1.</param>
    /// <param name="isVertical">
    /// Whether the guide belongs to an edge running along Y, which marks a height —
    /// so the line drawn across it is horizontal.
    /// </param>
    /// <param name="kind">What the boundary caught, which gives the line its colour.</param>
    public void Mark(double ratio, bool isVertical, SnapKind kind)
    {
        _ratio = ratio;
        _isVertical = isVertical;
        Guide.Stroke = SnapGuideBrushes.For(kind);

        Draw();

        if (!IsVisible) Show();
    }

    void Draw()
    {
        var width = Bounds.Width;
        var height = Bounds.Height;
        if (width <= 0 || height <= 0) return;

        var at = _ratio * (_isVertical ? height : width);

        Guide.StartPoint = _isVertical ? new Point(0, at) : new Point(at, 0);
        Guide.EndPoint = _isVertical ? new Point(width, at) : new Point(at, height);
    }
}
