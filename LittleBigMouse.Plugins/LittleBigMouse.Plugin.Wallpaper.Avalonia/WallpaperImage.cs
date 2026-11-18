using SkiaSharp;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

/// <summary>
/// The image primitives the wallpaper code needs: decoding, a resampled draw and PNG
/// encoding. They sit on the SkiaSharp that Avalonia already ships. ImageSharp did this
/// job before, but from 3.2.0 on it refuses Release builds without a Six Labors licence key.
/// </summary>
public static class WallpaperImage
{
    /// <summary>Pixel size of the image, read from its header without decoding it.</summary>
    public static SKSizeI Identify(string path)
    {
        using var codec = OpenCodec(path);
        return codec.Info.Size;
    }

    /// <summary>
    /// Decodes the first frame to BGRA. The image is opaque when its format carries no
    /// alpha, and premultiplied otherwise. The EXIF orientation is not applied (neither
    /// did ImageSharp's <c>Image.Load</c>).
    /// </summary>
    public static SKImage Load(string path)
    {
        using var codec = OpenCodec(path);
        var info = new SKImageInfo(codec.Info.Width, codec.Info.Height, SKColorType.Bgra8888,
            codec.Info.AlphaType == SKAlphaType.Opaque ? SKAlphaType.Opaque : SKAlphaType.Premul);

        using var bitmap = SKBitmap.Decode(codec, info)
            ?? throw new InvalidDataException($"Cannot decode image: {path}");
        bitmap.SetImmutable();
        // An immutable bitmap shares its pixels with the image, which keeps them alive.
        return SKImage.FromBitmap(bitmap);
    }

    /// <summary>A new BGRA canvas filled with <paramref name="background"/>.</summary>
    public static SKBitmap CreateCanvas(int width, int height, SKColor background, bool opaque = false)
    {
        var bitmap = new SKBitmap(new SKImageInfo(width, height, SKColorType.Bgra8888,
            opaque ? SKAlphaType.Opaque : SKAlphaType.Premul));
        bitmap.Erase(background);
        return bitmap;
    }

    /// <summary>Draws the <paramref name="source"/> part of the image, resampled into <paramref name="dest"/>.</summary>
    public static void Draw(SKBitmap target, SKImage image, SKRect source, SKRect dest)
    {
        using var canvas = new SKCanvas(target);
        using var paint = new SKPaint();
        canvas.DrawImage(image, source, dest, Sampling(source.Size, dest.Size), paint);
    }

    /// <summary>
    /// Mipmapped bilinear when shrinking, so that a 4× or 10× reduction does not alias.
    /// Mitchell bicubic otherwise, close to ImageSharp's default resampler.
    /// </summary>
    public static SKSamplingOptions Sampling(SKSize source, SKSize dest)
        => dest.Width < source.Width || dest.Height < source.Height
            ? new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear)
            : new SKSamplingOptions(SKCubicResampler.Mitchell);

    public static void SavePng(SKBitmap bitmap, string path)
    {
        using var data = bitmap.Encode(SKEncodedImageFormat.Png, 100)
            ?? throw new InvalidOperationException($"PNG encoding failed: {path}");
        using var file = File.Create(path);
        data.SaveTo(file);
    }

    // Skia reads BMP, GIF, ICO, JPEG, PNG, WBMP and WebP (plus HEIF/AVIF where the
    // platform provides them), but not TIFF or TGA, which ImageSharp did read.
    static SKCodec OpenCodec(string path)
        => SKCodec.Create(path) ?? throw new InvalidDataException($"Unsupported or unreadable image: {path}");
}
