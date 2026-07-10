#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Wallpaper;

/// <summary>
/// Slices a single source image over the whole monitor set in physical (mm) space:
/// each screen gets the exact portion of the image its panel covers, so the picture
/// stays continuous through bezels and across mixed-size/mixed-DPI monitors.
/// </summary>
public static class WallpaperSpanSlicer
{
    /// <param name="VisibleMm">Panel area in layout mm (DepthProjection.Bounds).</param>
    /// <param name="OutputPx">Native pixel size the slice will be rendered at.</param>
    public record ScreenInput(string Id, Rect VisibleMm, Size OutputPx);

    /// <param name="SourcePx">Crop rectangle in source-image pixels (clamped to the image).</param>
    public record Slice(string Id, Rect SourcePx, Size OutputPx);

    /// <summary>Bounding box of the whole monitor set, bezels included (DepthProjection.OutsideBounds).</summary>
    public static Rect ComputeBoundsMm(IEnumerable<Rect> outsideBoundsMm)
    {
        double left = double.PositiveInfinity, top = double.PositiveInfinity;
        double right = double.NegativeInfinity, bottom = double.NegativeInfinity;

        // HLab.Geo.Rect.Union is unreliable (static overload drops the result), do it by hand.
        foreach (var r in outsideBoundsMm)
        {
            if (r.IsEmpty) continue;
            left = Math.Min(left, r.Left);
            top = Math.Min(top, r.Top);
            right = Math.Max(right, r.Right);
            bottom = Math.Max(bottom, r.Bottom);
        }

        return double.IsInfinity(left) ? Rect.Empty : new Rect(left, top, right - left, bottom - top);
    }

    /// <summary>
    /// Cover-scale the image onto the mm bounding box (excess center-cropped),
    /// then crop each screen's visible area out of it.
    /// </summary>
    public static IReadOnlyList<Slice> ComputeSlices(Size imagePx, Rect boundsMm, IEnumerable<ScreenInput> screens)
    {
        if (boundsMm.IsEmpty || boundsMm.Width <= 0 || boundsMm.Height <= 0
            || imagePx.Width <= 0 || imagePx.Height <= 0)
            return [];

        // px per mm so the box fits inside the image (cover: the image overflows the
        // box on the other axis and the excess is center-cropped).
        var s = Math.Min(imagePx.Width / boundsMm.Width, imagePx.Height / boundsMm.Height);
        var offsetX = (imagePx.Width - boundsMm.Width * s) / 2;
        var offsetY = (imagePx.Height - boundsMm.Height * s) / 2;

        var image = new Rect(0, 0, imagePx.Width, imagePx.Height);

        return screens.Select(screen =>
        {
            var crop = new Rect(
                (screen.VisibleMm.X - boundsMm.X) * s + offsetX,
                (screen.VisibleMm.Y - boundsMm.Y) * s + offsetY,
                screen.VisibleMm.Width * s,
                screen.VisibleMm.Height * s);

            crop.Intersect(image);

            return new Slice(screen.Id, crop, screen.OutputPx);
        }).ToList();
    }
}
