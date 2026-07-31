using Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The dashed reference line, on a real screen: a band the length of the screen
/// and no thicker than it must be, holding a line down its middle.
/// <para>
/// It was a window covering the screen at first, which is simpler to place and
/// unusable in practice: not being click-through — <c>WS_EX_TRANSPARENT</c> does
/// nothing on a window composed without a redirection bitmap — it swallowed every
/// click while it was up, and covering the taskbar had the shell bring the bar to
/// the front each time a boundary caught something.
/// </para>
/// <para>
/// So: a band, clear of the taskbar, and one window per orientation per screen so
/// that its size never changes. A guide only ever moves one, and a window that is
/// only moved cannot be caught between the size it asked for and the size it was
/// granted. Between guides it is parked outside every screen rather than hidden,
/// which would cost a native window each way and have the shell notice.
/// </para>
/// </summary>
public partial class GuideLineWindow : EdgeOverlayWindow
{
    /// <summary>
    /// Across the line, in DIPs. Same figure as a band, and for the same reason:
    /// under the platform's own minimum a window is given more than it asked for,
    /// and what it is given differs per axis and per screen.
    /// </summary>
    public const double ThicknessDip = 40.0;

    const int WsExNoActivate = 0x08000000;

    /// <summary>
    /// Never takes focus: a line appearing must not take the keyboard from whatever
    /// is underneath it.
    /// </summary>
    protected override int ExtraExtendedStyle => WsExNoActivate;

    public GuideLineWindow()
    {
        InitializeComponent();

        SizeChanged += (_, _) => Draw();
    }

    /// <param name="isVertical">
    /// Whether this window serves guides on edges running along Y, which mark a
    /// height — so it lies across the screen and holds a horizontal line.
    /// </param>
    public GuideLineWindow(bool isVertical) : this() => _isVertical = isVertical;

    readonly bool _isVertical;

    /// <summary>Where this window waits between guides — outside every screen.</summary>
    public PixelPoint ParkingSpot
    {
        get;
        set
        {
            field = value;
            if (!Guide.IsVisible) Park(value);
        }
    }

    /// <summary>Lay the band over <paramref name="where"/> and show the line in it.</summary>
    public void Mark(PixelRect where, SnapKind kind)
    {
        Guide.Stroke = SnapGuideBrushes.For(kind);
        Guide.IsVisible = true;

        MoveTo(where);
        Draw();
    }

    /// <summary>Take the line down and the band out of the way.</summary>
    public void Clear()
    {
        Guide.IsVisible = false;
        Park(ParkingSpot);
    }

    /// <summary>Down the middle of the band, along its length.</summary>
    void Draw()
    {
        var width = Bounds.Width;
        var height = Bounds.Height;
        if (width <= 0 || height <= 0) return;

        Guide.StartPoint = _isVertical ? new Point(0, height / 2) : new Point(width / 2, 0);
        Guide.EndPoint = _isVertical ? new Point(width, height / 2) : new Point(width / 2, height);
    }
}
