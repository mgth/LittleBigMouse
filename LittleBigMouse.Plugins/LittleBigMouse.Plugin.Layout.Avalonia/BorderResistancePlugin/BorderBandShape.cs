using Avalonia;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// The outline of a band along one monitor edge, mitred at both ends.
/// <para>
/// A band spans its edge end to end, so the four bands of a monitor overlap in the
/// corner squares. Each one keeps the triangle on its own side of the square's
/// diagonal, so the corners meet like a picture frame and a click there reaches
/// exactly one band.
/// </para>
/// <para>
/// Nothing here but arithmetic on a rectangle: the band's own size in, its corner
/// points out. <see cref="Point"/> is a plain value, so this needs no display.
/// </para>
/// </summary>
public static class BorderBandShape
{
    /// <param name="thickness">
    /// The thickness of the bands meeting this one at right angles. They are all the
    /// same width, so the corner square is <paramref name="thickness"/> square.
    /// </param>
    public static Point[] For(BorderSideKind kind, double width, double height, double thickness)
    {
        var w = width;
        var h = height;
        var t = thickness;

        return kind switch
        {
            // Outer edge on the left: keep the lower-left half of each corner square.
            BorderSideKind.Left =>
                [new Point(0, 0), new Point(t, t), new Point(t, h - t), new Point(0, h)],

            BorderSideKind.Right =>
                [new Point(w, 0), new Point(w, h), new Point(0, h - t), new Point(0, t)],

            BorderSideKind.Top =>
                [new Point(0, 0), new Point(w, 0), new Point(w - t, t), new Point(t, t)],

            _ =>
                [new Point(0, h), new Point(t, 0), new Point(w - t, 0), new Point(w, h)]
        };
    }
}
