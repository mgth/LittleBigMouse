using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using HLab.ColorTools;
using LittleBigMouse.Plugin.Wallpaper.Avalonia;
using SkiaSharp;
using System;
using System.Threading.Tasks;
using AvaloniaSize = Avalonia.Size;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

/// <summary>
/// Renders the per-monitor wallpaper preview of the layout editor. Every mode first
/// shrinks the source by <c>shrink</c>, as the monitor frames are drawn at that scale,
/// then places it the way the OS does. The geometry is the one the ImageSharp
/// version used (see the <see cref="Placement"/> functions).
/// </summary>
public static class WallpaperRendererHelper
{
    public static Task<Bitmap> GetWallpaperFillAsync(string path, AvaloniaSize size, int shrink)
        => RenderAsync(path, shrink, image => Draw(image, Fill(SizeOf(image), size.ToPixels()), SKColors.Transparent));

    public static Task<Bitmap> GetWallpaperCenterAsync(string path, AvaloniaSize size, ColorRGB<double> color, int shrink)
        => RenderAsync(path, shrink, image => Draw(image, Center(SizeOf(image), size.ToPixels()), color.ToSkia()));

    public static Task<Bitmap> GetWallpaperFitAsync(string path, AvaloniaSize size, ColorRGB<double> color, int shrink)
        => RenderAsync(path, shrink, image => Draw(image, Fit(SizeOf(image), size.ToPixels()), color.ToSkia()));

    public static Task<Bitmap> GetWallpaperStretchAsync(string path, AvaloniaSize size, int shrink)
        => RenderAsync(path, shrink, image => Draw(image, Stretch(SizeOf(image), size.ToPixels()), SKColors.Transparent));

    public static Task<Bitmap> GetWallpaperTileAsync(string path, Rect target, Rect fullArea, int shrink)
        => RenderAsync(path, shrink, image => DrawTiles(image, Tile(target.ToPixels(), fullArea.ToPixels())));

    public static Task<Bitmap> GetWallpaperSpanAsync(string path, Rect target, Rect fullArea, int shrink)
        => RenderAsync(path, shrink, image => Draw(image, Span(SizeOf(image), target.ToPixels(), fullArea.ToPixels()), SKColors.Transparent));

    static Task<Bitmap> RenderAsync(string path, int shrink, Func<SKImage, SKBitmap> render)
        => Task.Run(() =>
        {
            using var source = WallpaperImage.Load(path);
            using var shrunk = Shrink(source, shrink);
            using var result = render(shrunk);
            // The Bitmap copies the pixels: the Skia buffer can go right after.
            return new Bitmap(PixelFormat.Bgra8888, AlphaFormat.Premul, result.GetPixels(),
                new PixelSize(result.Width, result.Height), new Vector(96, 96), result.RowBytes);
        });

    /// <summary>Where to draw which part of the (shrunk) source on a canvas of a given size.</summary>
    internal readonly record struct Placement(SKSizeI Canvas, SKRect Source, SKRect Dest);

    internal static SKImage Shrink(SKImage image, int shrink)
    {
        var width = Math.Max(1, image.Width / shrink);
        var height = Math.Max(1, image.Height / shrink);
        var shrunk = WallpaperImage.CreateCanvas(width, height, SKColors.Transparent);
        WallpaperImage.Draw(shrunk, image, SKRect.Create(image.Width, image.Height), SKRect.Create(width, height));
        shrunk.SetImmutable();
        using (shrunk) return SKImage.FromBitmap(shrunk);
    }

    internal static SKBitmap Draw(SKImage image, Placement placement, SKColor background)
    {
        var bitmap = WallpaperImage.CreateCanvas(placement.Canvas.Width, placement.Canvas.Height, background);
        WallpaperImage.Draw(bitmap, image, placement.Source, placement.Dest);
        return bitmap;
    }

    /// <summary>Scales to cover the screen and crops the centred overflow.</summary>
    internal static Placement Fill(SKSizeI source, SKSizeI target)
    {
        var ratio = Math.Max(target.Width / (double)source.Width, target.Height / (double)source.Height);

        var w = (int)(target.Width / ratio);
        var h = (int)(target.Height / ratio);

        var crop = SKRect.Create((source.Width - w) / 2, (source.Height - h) / 2, w, h);
        return new Placement(target, crop, SKRect.Create(target.Width, target.Height));
    }

    /// <summary>
    /// Unscaled and centred on the background colour. A source larger than the screen
    /// is cropped from its top-left corner, as the ImageSharp version did.
    /// </summary>
    internal static Placement Center(SKSizeI source, SKSizeI target)
    {
        var w = Math.Min(source.Width, target.Width);
        var h = Math.Min(source.Height, target.Height);

        return new Placement(target, SKRect.Create(w, h),
            SKRect.Create((target.Width - w) / 2, (target.Height - h) / 2, w, h));
    }

    /// <summary>Scales to fit inside the screen and letterboxes with the background colour.</summary>
    internal static Placement Fit(SKSizeI source, SKSizeI target)
    {
        var ratio = Math.Min(target.Width / (double)source.Width, target.Height / (double)source.Height);

        var w = (int)(ratio * source.Width);
        var h = (int)(ratio * source.Height);

        return new Placement(target, SKRect.Create(source.Width, source.Height),
            SKRect.Create((target.Width - w) / 2, (target.Height - h) / 2, w, h));
    }

    internal static Placement Stretch(SKSizeI source, SKSizeI target)
        => new(target, SKRect.Create(source.Width, source.Height), SKRect.Create(target.Width, target.Height));

    /// <summary>
    /// This screen's part of the source scaled to cover the whole virtual desktop.
    /// Match how Windows "Span" actually crops the cover-scaled image over the virtual desktop.
    /// Measured against real Span output (grid wallpaper, screen capture): the horizontal
    /// overflow is centered, but the VERTICAL overflow is split 1/3 top : 2/3 bottom (not
    /// centered). Verified across image heights 1000..2400: visible band top = (H - fullArea.H)/3.
    /// </summary>
    internal static Placement Span(SKSizeI source, SKRectI target, SKRectI fullArea)
    {
        var ratio = Math.Max(fullArea.Width / (double)source.Width, fullArea.Height / (double)source.Height);
        var resized = new SKSizeI((int)(ratio * source.Width), (int)(ratio * source.Height));

        var offsetX = (resized.Width - fullArea.Width) / 2;
        var offsetY = (resized.Height - fullArea.Height) / 3;

        var crop = Clamp(Offset(target, -fullArea.Left + offsetX, -fullArea.Top + offsetY), resized);

        // The crop is in the resized image: bring it back to the source.
        var kx = source.Width / (float)resized.Width;
        var ky = source.Height / (float)resized.Height;
        return new Placement(crop.Size,
            SKRect.Create(crop.Left * kx, crop.Top * ky, crop.Width * kx, crop.Height * ky),
            SKRect.Create(crop.Width, crop.Height));
    }

    /// <summary>
    /// The tiles start at the top-left corner of the virtual desktop. The result is this
    /// screen's part of that tiled desktop, in desktop pixels.
    /// </summary>
    internal static SKRectI Tile(SKRectI target, SKRectI fullArea)
        => Clamp(Offset(target, -fullArea.Left, -fullArea.Top), fullArea.Size);

    internal static SKBitmap DrawTiles(SKImage image, SKRectI crop)
    {
        var bitmap = WallpaperImage.CreateCanvas(crop.Width, crop.Height, SKColors.Transparent);
        using var canvas = new SKCanvas(bitmap);
        // The shader maps canvas pixel p to desktop pixel p + crop.Location.
        using var shader = image.ToShader(SKShaderTileMode.Repeat, SKShaderTileMode.Repeat,
            new SKSamplingOptions(SKFilterMode.Nearest), SKMatrix.CreateTranslation(-crop.Left, -crop.Top));
        using var paint = new SKPaint { Shader = shader };
        canvas.DrawRect(SKRect.Create(crop.Width, crop.Height), paint);
        return bitmap;
    }

    // Integer truncation in the ratio/resize math above (and in the source Rect -> pixel
    // conversions) can leave the crop rectangle up to 1px past the resized image. That used
    // to throw on a background thread and take down the whole UI (see issue #492). Clamp to
    // the image bounds so the Span/Tile wallpaper modes can never crash on edge-case geometry.
    static SKRectI Clamp(SKRectI target, SKSizeI bounds)
    {
        var crop = SKRectI.Intersect(target, SKRectI.Create(bounds));
        if (crop.Width < 1 || crop.Height < 1)
            throw new ArgumentException($"The screen {target} lies outside the wallpaper ({bounds.Width}x{bounds.Height}).");
        return crop;
    }

    static SKRectI Offset(SKRectI rect, int dx, int dy)
        => SKRectI.Create(rect.Left + dx, rect.Top + dy, rect.Width, rect.Height);

    static SKSizeI SizeOf(SKImage image) => new(image.Width, image.Height);

    static SKSizeI ToPixels(this AvaloniaSize size) => new((int)size.Width, (int)size.Height);

    static SKRectI ToPixels(this Rect rect)
        => SKRectI.Create((int)rect.X, (int)rect.Y, (int)rect.Width, (int)rect.Height);

    static SKColor ToSkia(this ColorRGB<double> color)
    {
        var c = color.To<byte>();
        return new SKColor(c.Red, c.Green, c.Blue, c.Alpha);
    }
}
