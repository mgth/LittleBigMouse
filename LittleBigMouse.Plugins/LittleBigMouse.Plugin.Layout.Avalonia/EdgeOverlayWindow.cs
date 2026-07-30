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
/// </summary>
public abstract class EdgeOverlayWindow : Window
{
    PixelPoint _position;
    double _pixelWidth;
    double _pixelHeight;

    protected EdgeOverlayWindow()
    {
        Opened += (_, _) => FitToPixels();
    }

    /// <summary>Position and size the window in windowing-system pixels, then show it.</summary>
    public void ShowAt(PixelPoint position, double pixelWidth, double pixelHeight, double scalingHint)
    {
        _position = position;
        _pixelWidth = pixelWidth;
        _pixelHeight = pixelHeight;

        Position = position;
        SetDipSize(scalingHint > 0 ? scalingHint : 1.0);
        Show();
    }

    void FitToPixels()
    {
        SetDipSize(DesktopScaling);
        Position = _position;
    }

    void SetDipSize(double scaling)
    {
        Width = _pixelWidth / scaling;
        Height = _pixelHeight / scaling;
    }
}
