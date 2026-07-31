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
    void Anchor()
    {
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
}
