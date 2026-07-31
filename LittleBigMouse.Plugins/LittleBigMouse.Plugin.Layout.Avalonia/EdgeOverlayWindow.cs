using Avalonia;
using Avalonia.Controls;

namespace LittleBigMouse.Plugin.Layout.Avalonia;

/// <summary>
/// Borderless topmost window covering a single screen edge.
/// <para>
/// One window per edge rather than one fullscreen window per screen, so the
/// middle of the screen is never covered and clicks reach the applications
/// below without any OS-specific window-region cutting.
/// </para>
/// <para>
/// Callers work in windowing-system pixels; the DIP conversion happens here,
/// twice: once with the caller's hint so the window is roughly right before it
/// is mapped, and again on Opened, because the scaling actually applied by the
/// windowing system is only known then and may differ from the hint.
/// </para>
/// <para>
/// The size asked for is not always the size granted. A window is refused less
/// than the platform's minimum — the size of decorations it does not have
/// (AvaloniaUI/Avalonia#20251), <c>SM_CYMINTRACK</c> on Windows: 56 px at 144 dpi
/// and 103 at 288 — and nothing above the platform can waive it, neither the
/// layout, which asks for exactly the right size, nor an explicit placement
/// through the platform's own API. So an overlay is anchored rather than
/// positioned: it is told which edge of the screen it measures, and keeps flush
/// with it at whatever thickness it ends up with.
/// </para>
/// <para>
/// Its callers stay clear of that floor — 40 DIP for a band, 100 for a ruler — so
/// the request is normally honoured to the pixel. The anchoring is what makes that
/// a guarantee rather than a coincidence: placing an overlay from the thickness it
/// asked for, on the day the platform grants another, leaves it hanging past the
/// bottom of the screen or spilling onto the monitor next door.
/// </para>
/// </summary>
public abstract class EdgeOverlayWindow : Window
{
    PixelRect _target;
    bool _anchorRight;
    bool _anchorBottom;

    protected EdgeOverlayWindow()
    {
        Opened += async (_, _) =>
        {
            FitToPixels();

            // The size settles a moment after the window is mapped, and the changes
            // seen on the way there are not all this window's. Anchoring only on
            // them leaves a band wherever its first, unadjusted placement landed —
            // which on the screen carrying the taskbar is a whole taskbar away.
            await System.Threading.Tasks.Task.Delay(500);
            Anchor();
        };
    }

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        // The granted size arrives after the request, and can change again with the
        // scaling; re-anchoring on every one of them is what keeps the band on its
        // edge without anyone having to know what the platform will allow.
        if (change.Property == ClientSizeProperty) Anchor();
    }

    /// <summary>
    /// Show the window over <paramref name="target"/>, in windowing-system pixels.
    /// </summary>
    /// <param name="anchorRight">Keep the window's right edge on the target's, not its left.</param>
    /// <param name="anchorBottom">Keep its bottom edge on the target's, not its top.</param>
    public void ShowAt(PixelRect target, double scalingHint, bool anchorRight = false, bool anchorBottom = false)
    {
        _target = target;
        _anchorRight = anchorRight;
        _anchorBottom = anchorBottom;

        Position = target.Position;
        SetDipSize(scalingHint > 0 ? scalingHint : 1.0);
        Show();

        MarkAsToolWindow();
        Anchor();
    }

    void FitToPixels()
    {
        SetDipSize(DesktopScaling);
        Anchor();
    }

    void SetDipSize(double scaling)
    {
        Width = _target.Width / scaling;
        Height = _target.Height / scaling;
    }

    /// <summary>
    /// Keep the anchored edges on the target's, at the size the platform granted.
    /// <para>
    /// Only a size that spans the edge is believed. While the windows are being
    /// brought up, the client size briefly reports figures belonging to another of
    /// them — half of one, or the main window's — and anchoring on those puts a band
    /// wherever the arithmetic lands. Which one arrived last then decided where the
    /// band stayed, so it moved from one run to the next.
    /// </para>
    /// </summary>
    /// <summary>
    /// Put the window away without hiding it: out beyond every screen, where it
    /// covers nothing and takes no clicks.
    /// <para>
    /// Hiding would be the obvious way, and it is the wrong one. Avalonia remakes
    /// the native window on the way back up, as an ordinary application window —
    /// the shell keeps a place for it in the taskbar and brings the bar to the
    /// front — and every extended style set on the old one goes with it. Parking
    /// keeps one window from beginning to end.
    /// </para>
    /// </summary>
    protected void Park(PixelPoint spot)
    {
        _parked = true;

        Position = spot;
        MoveThere(spot.X, spot.Y);
    }

    /// <summary>Bring it back over the screen it belongs to.</summary>
    protected void Unpark()
    {
        _parked = false;
        Anchor();
    }

    /// <summary>
    /// Send the window somewhere else on the desktop, at the size it already has.
    /// Only the place changes, which is what keeps a window that moves often out of
    /// the negotiation over its size.
    /// </summary>
    protected void MoveTo(PixelRect target)
    {
        _target = target;
        Unpark();
    }

    bool _parked;

    void Anchor()
    {
        if (_parked) return;

        var granted = PixelSize.FromSize(ClientSize, DesktopScaling);
        if (granted.Width <= 0 || granted.Height <= 0) return;

        // The long side is the one we know: it is the screen edge itself.
        if (_anchorBottom && granted.Width != _target.Width) return;
        if (_anchorRight && granted.Height != _target.Height) return;

        var x = _anchorRight ? _target.Right - granted.Width : _target.X;
        var y = _anchorBottom ? _target.Bottom - granted.Height : _target.Y;

        Position = new PixelPoint(x, y);
        MoveThere(x, y);
    }

    const uint SwpNoSize = 0x0001;
    const uint SwpNoZOrder = 0x0004;
    const uint SwpNoActivate = 0x0010;

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern bool SetWindowPos(nint window, nint after, int x, int y, int cx, int cy, uint flags);

    /// <summary>
    /// Move the window through the platform as well.
    /// <para>
    /// Setting <see cref="Window.Position"/> is not always enough: the band along the
    /// bottom of the screen carrying the taskbar ended up exactly one taskbar higher
    /// than asked, with Avalonia reporting the position it had been given while the
    /// window sat elsewhere. A plain window placed there by hand is honoured, so the
    /// place is allowed — it is the way of asking that was not getting through.
    /// </para>
    /// <para>
    /// Only the position is set; the size is left as granted, which is the whole
    /// point of anchoring. Elsewhere, <see cref="Window.Position"/> stands alone.
    /// </para>
    /// </summary>
    void MoveThere(int x, int y)
    {
        if (!OperatingSystem.IsWindows()) return;
        if (TryGetPlatformHandle()?.Handle is not { } window) return;

        SetWindowPos(window, 0, x, y, 0, 0, SwpNoSize | SwpNoZOrder | SwpNoActivate);
    }

    [System.Runtime.InteropServices.DllImport("gdi32.dll")]
    static extern nint CreatePolygonRgn(System.Drawing.Point[] points, int count, int mode);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern int SetWindowRgn(nint window, nint region, bool redraw);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern bool GetWindowRect(nint window, out WindowRect rect);

    [System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
    struct WindowRect { public int Left, Top, Right, Bottom; }

    /// <summary>
    /// Cut the window down to the shape its content actually occupies.
    /// <para>
    /// Four bands meet in each corner of a screen, and each keeps the triangle on
    /// its own side of the square's diagonal so the corners meet like a picture
    /// frame. Inside one window that is enough: the clipped-away half belongs to a
    /// sibling and hit testing finds it. Across windows it is not — a window covers
    /// its whole rectangle whatever its content does, so a press on the half it
    /// gave up landed on it, missed everything, and died there instead of reaching
    /// the band underneath. That is what made a handle in a corner unclickable.
    /// </para>
    /// <para>
    /// The region is the mitre itself, so nothing is given up twice: what a band
    /// does not draw, it no longer catches. Windows only — X11 has an input shape
    /// and Wayland a region, neither reachable from here, so the corners there stay
    /// as they are.
    /// </para>
    /// </summary>
    /// <param name="shape">The outline, in this window's own device-independent pixels.</param>
    public void ShapeTo(IReadOnlyList<Point> shape)
    {
        if (!OperatingSystem.IsWindows()) return;
        if (shape.Count < 3) return;
        if (TryGetPlatformHandle()?.Handle is not { } window) return;

        // Scale from what the window was actually given rather than from the
        // scaling it reports: the two have been seen to disagree while a window is
        // still settling, and a region is silent when it is wrong.
        if (!GetWindowRect(window, out var rect)) return;

        // The client area, which is what the band fills and what the shape is
        // expressed in. A window's own Bounds is not that, and is 0 here.
        var width = ClientSize.Width;
        var height = ClientSize.Height;
        if (width <= 0 || height <= 0) return;

        var scaleX = (rect.Right - rect.Left) / width;
        var scaleY = (rect.Bottom - rect.Top) / height;

        var points = new System.Drawing.Point[shape.Count];
        for (var i = 0; i < shape.Count; i++)
        {
            points[i] = new System.Drawing.Point(
                (int)System.Math.Round(shape[i].X * scaleX),
                (int)System.Math.Round(shape[i].Y * scaleY));
        }

        // The window owns the region once it is set, and frees the one it held.
        var region = CreatePolygonRgn(points, points.Length, 1);
        if (region == 0) return;

        SetWindowRgn(window, region, true);
    }

    const int GwlExStyle = -20;
    const int WsExToolWindow = 0x00000080;

    /// <summary>
    /// Extended window bits this overlay needs beyond being a tool window, on
    /// Windows. Applied with it, once, on the first showing.
    /// </summary>
    protected virtual int ExtraExtendedStyle => 0;

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern int GetWindowLongW(nint window, int index);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    static extern int SetWindowLongW(nint window, int index, int value);

    /// <summary>
    /// Say what these windows are: tools, not applications.
    /// <para>
    /// They already ask to stay out of the taskbar, and Avalonia obliges by having
    /// the shell drop their button. That holds for a window shown once and left
    /// alone — but the guide line is hidden and shown again at every snap, and the
    /// button came back with it each time, bringing the taskbar to the front over
    /// everything else. A tool window is never given a button at all, whatever it
    /// does afterwards, which is the property actually wanted here.
    /// </para>
    /// <para>
    /// Set on the first showing, while the window is still on its way up: Windows
    /// keeps an existing button until the window is hidden and shown again, and the
    /// overlays that stay up are past that point by the time anyone sees them. An
    /// overlay that comes and goes must not be hidden at all — the native window is
    /// remade on the way back, without any of this — which is why the guide line
    /// stays up and empties itself instead.
    /// </para>
    /// </summary>
    protected void MarkAsToolWindow()
    {
        if (!OperatingSystem.IsWindows()) return;
        if (TryGetPlatformHandle()?.Handle is not { } window) return;

        var style = GetWindowLongW(window, GwlExStyle);
        if (style == 0) return;

        var wanted = style | WsExToolWindow | ExtraExtendedStyle;
        if (wanted == style) return;

        SetWindowLongW(window, GwlExStyle, wanted);
    }
}
