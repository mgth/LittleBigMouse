using System.Security.Cryptography;
using System.Text;
using LittleBigMouse.DisplayLayout.Wallpaper;
using LittleBigMouse.Plugins;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Processing;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

/// <summary>
/// Renders WallpaperSpanSlicer slices to PNG files. Filenames are content-addressed
/// (source + mtime + crop + output size): an unchanged config maps to the same path
/// so plasma no-ops (no flicker), any change maps to a new path so org.kde.image is
/// guaranteed to reload (it caches by path). Stale slices are swept after each render.
/// </summary>
public static class SpanRenderer
{
    public static string OutputDir => Path.Combine(LbmPaths.DataDir, "wallpapers");

    /// <summary>Slice the source image; returns screen id → generated file path.</summary>
    public static Dictionary<string, string> Render(string sourcePath, IReadOnlyList<WallpaperSpanSlicer.Slice> slices)
    {
        Directory.CreateDirectory(OutputDir);
        var mtime = File.GetLastWriteTimeUtc(sourcePath).Ticks;

        var result = new Dictionary<string, string>();
        Image? image = null;
        try
        {
            foreach (var slice in slices)
            {
                var crop = new Rectangle(
                    (int)Math.Round(slice.SourcePx.X),
                    (int)Math.Round(slice.SourcePx.Y),
                    Math.Max(1, (int)Math.Round(slice.SourcePx.Width)),
                    Math.Max(1, (int)Math.Round(slice.SourcePx.Height)));
                var outputWidth = Math.Max(1, (int)Math.Round(slice.OutputPx.Width));
                var outputHeight = Math.Max(1, (int)Math.Round(slice.OutputPx.Height));

                var hash = Hash($"{sourcePath}|{mtime}|{slice.Id}|{crop}|{outputWidth}x{outputHeight}");
                var file = Path.Combine(OutputDir, $"span_{hash}.png");

                if (!File.Exists(file))
                {
                    image ??= Image.Load(sourcePath);
                    // Rounding may push the crop 1px past the image edge: clamp.
                    crop = Rectangle.Intersect(crop, new Rectangle(0, 0, image.Width, image.Height));
                    if (crop.Width < 1 || crop.Height < 1) continue;

                    using var sliceImage = image.Clone(ctx => ctx
                        .Crop(crop)
                        .Resize(outputWidth, outputHeight));
                    var tmp = file + ".tmp";
                    sliceImage.SaveAsPng(tmp);
                    File.Move(tmp, file, overwrite: true);
                }

                result[slice.Id] = file;
            }
        }
        finally
        {
            image?.Dispose();
        }

        foreach (var stale in Directory.EnumerateFiles(OutputDir, "span_*.png").ToList())
            if (!result.ContainsValue(stale))
                try { File.Delete(stale); } catch { /* still displayed by plasma: retry next apply */ }

        return result;
    }

    static string Hash(string input)
        => Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(input)))[..16];
}
