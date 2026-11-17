using LittleBigMouse.Plugin.Wallpaper.Avalonia;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
using SkiaSharp;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The wallpaper drawn behind each monitor in the editor, and the slices written for a
/// spanned wallpaper. The placements are the geometry ImageSharp used, worked out by
/// hand. The pixel checks pin what Skia does with them: tiling phase, letterbox
/// colour, a shrink that averages instead of aliasing, and the PNG round trip.
/// </summary>
public sealed class WallpaperRenderingTests
{
    static SKSizeI Size(int w, int h) => new(w, h);
    static SKRectI Rect(int x, int y, int w, int h) => SKRectI.Create(x, y, w, h);

    [Fact]
    public void FillCoversTheScreenAndCropsTheCentredOverflow()
    {
        var placement = WallpaperRendererHelper.Fill(Size(400, 100), Size(100, 100));

        Assert.Equal(Size(100, 100), placement.Canvas);
        Assert.Equal(SKRect.Create(150, 0, 100, 100), placement.Source);
        Assert.Equal(SKRect.Create(100, 100), placement.Dest);
    }

    [Fact]
    public void CenterKeepsTheImageUnscaledInTheMiddle()
    {
        var placement = WallpaperRendererHelper.Center(Size(50, 30), Size(100, 100));

        Assert.Equal(SKRect.Create(50, 30), placement.Source);
        Assert.Equal(SKRect.Create(25, 35, 50, 30), placement.Dest);
    }

    [Fact]
    public void CenterCropsALargerImageFromItsTopLeftCorner()
    {
        var placement = WallpaperRendererHelper.Center(Size(300, 300), Size(100, 50));

        Assert.Equal(SKRect.Create(100, 50), placement.Source);
        Assert.Equal(SKRect.Create(100, 50), placement.Dest);
    }

    [Fact]
    public void FitLetterboxesTheWholeImage()
    {
        var placement = WallpaperRendererHelper.Fit(Size(200, 100), Size(100, 100));

        Assert.Equal(SKRect.Create(200, 100), placement.Source);
        Assert.Equal(SKRect.Create(0, 25, 100, 50), placement.Dest);
    }

    [Fact]
    public void SpanCentresHorizontallyAndKeepsAThirdOfTheVerticalOverflowOnTop()
    {
        // 100×100 covering a 200×100 desktop is scaled to 200×200: 100 rows of overflow,
        // 33 of them above. The right-hand screen gets the right half of that band.
        var placement = WallpaperRendererHelper.Span(Size(100, 100), Rect(100, 0, 100, 100), Rect(0, 0, 200, 100));

        Assert.Equal(Size(100, 100), placement.Canvas);
        Assert.Equal(SKRect.Create(50, 16.5f, 50, 50), placement.Source);
        Assert.Equal(SKRect.Create(100, 100), placement.Dest);
    }

    [Fact]
    public void ASpanCropPastTheImageIsClampedInsteadOfThrowing()
    {
        // Issue #492: the crop could end up past the scaled image and kill the UI.
        var placement = WallpaperRendererHelper.Span(Size(10, 10), Rect(5, 5, 10, 10), Rect(0, 0, 10, 10));

        Assert.Equal(Size(5, 5), placement.Canvas);
        Assert.Equal(SKRect.Create(5, 5, 5, 5), placement.Source);
    }

    [Fact]
    public void AScreenOutsideTheDesktopIsAnErrorNotAnEmptyBitmap()
    {
        Assert.Throws<ArgumentException>(() =>
            WallpaperRendererHelper.Span(Size(10, 10), Rect(50, 50, 10, 10), Rect(0, 0, 10, 10)));
    }

    [Fact]
    public void TilesArePhasedOnTheDesktopOriginNotOnTheScreen()
    {
        var crop = WallpaperRendererHelper.Tile(Rect(30, 10, 20, 20), Rect(10, 0, 100, 50));
        Assert.Equal(Rect(20, 10, 20, 20), crop);

        // A red/blue 2×1 tile: desktop column 1 is blue, 2 red, 3 blue.
        using var tile = Image(2, 1, (x, _) => x == 0 ? SKColors.Red : SKColors.Blue);
        using var result = WallpaperRendererHelper.DrawTiles(tile, Rect(1, 0, 3, 1));

        Assert.Equal(new[] { SKColors.Blue, SKColors.Red, SKColors.Blue },
            new[] { result.GetPixel(0, 0), result.GetPixel(1, 0), result.GetPixel(2, 0) });
    }

    [Fact]
    public void FitPaintsTheLetterboxWithTheBackgroundColour()
    {
        using var image = Image(2, 1, (_, _) => SKColors.Red);
        var placement = WallpaperRendererHelper.Fit(Size(2, 1), Size(4, 4));
        using var result = WallpaperRendererHelper.Draw(image, placement, SKColors.Lime);

        Assert.Equal(SKColors.Lime, result.GetPixel(0, 0));
        Assert.Equal(SKColors.Red, result.GetPixel(2, 2));
        Assert.Equal(SKColors.Lime, result.GetPixel(3, 3));
    }

    [Fact]
    public void ShrinkingAveragesFineDetailInsteadOfAliasing()
    {
        // A one-pixel checkerboard shrunk 5× must come out grey. Shrunk 5×, each output
        // pixel centre lands exactly on one source texel, so plain bilinear without
        // mipmaps would return that texel and keep the black-and-white pattern.
        using var checker = Image(400, 400, (x, y) => ((x + y) & 1) == 0 ? SKColors.White : SKColors.Black);
        using var shrunk = WallpaperRendererHelper.Shrink(checker, 5);
        using var pixels = SKBitmap.FromImage(shrunk);

        Assert.Equal(80, pixels.Width);
        Assert.Equal(80, pixels.Height);
        foreach (var (x, y) in new[] { (0, 0), (37, 51), (79, 79) })
            Assert.InRange((int)pixels.GetPixel(x, y).Red, 100, 155);
    }

    [Fact]
    public void AnOpaqueSliceRoundTripsThroughPng()
    {
        using var dir = new ScratchDir();
        var path = Path.Combine(dir.Path, "slice.png");

        using (var slice = WallpaperImage.CreateCanvas(30, 20, SKColors.Teal, opaque: true))
            WallpaperImage.SavePng(slice, path);

        Assert.Equal(Size(30, 20), WallpaperImage.Identify(path));
        using var loaded = WallpaperImage.Load(path);
        Assert.Equal(SKAlphaType.Opaque, loaded.AlphaType);
        using var pixels = SKBitmap.FromImage(loaded);
        Assert.Equal(SKColors.Teal, pixels.GetPixel(15, 10));
    }

    [Fact]
    public void AFileSkiaCannotReadIsReportedAsInvalidData()
    {
        using var dir = new ScratchDir();
        var path = Path.Combine(dir.Path, "not-an-image.png");
        File.WriteAllText(path, "plain text");

        Assert.Throws<InvalidDataException>(() => WallpaperImage.Identify(path));
        Assert.Throws<InvalidDataException>(() => WallpaperImage.Load(path));
    }

    static SKImage Image(int width, int height, Func<int, int, SKColor> color)
    {
        using var bitmap = new SKBitmap(new SKImageInfo(width, height, SKColorType.Bgra8888, SKAlphaType.Premul));
        for (var y = 0; y < height; y++)
        for (var x = 0; x < width; x++)
            bitmap.SetPixel(x, y, color(x, y));
        bitmap.SetImmutable();
        return SKImage.FromBitmap(bitmap);
    }

    sealed class ScratchDir : IDisposable
    {
        public string Path { get; } = Directory.CreateTempSubdirectory("lbm-wallpaper-").FullName;
        public void Dispose() => Directory.Delete(Path, recursive: true);
    }
}
