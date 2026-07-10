using HLab.Geo;
using LittleBigMouse.DisplayLayout.Wallpaper;

namespace LittleBigMouse.DisplayLayout.Tests;

public class WallpaperSpanSlicerTests
{
    static WallpaperSpanSlicer.ScreenInput Screen(string id, double x, double y, double wMm, double hMm, double wPx, double hPx)
        => new(id, new Rect(x, y, wMm, hMm), new Size(wPx, hPx));

    static Rect CropOf(IReadOnlyList<WallpaperSpanSlicer.Slice> slices, string id)
        => slices.Single(s => s.Id == id).SourcePx;

    [Fact]
    public void BoundsMmIsUnionOfOutsideBounds()
    {
        var bounds = WallpaperSpanSlicer.ComputeBoundsMm([
            new Rect(-20, -20, 520, 340),
            new Rect(500, 10, 520, 340),
        ]);

        Assert.Equal(new Rect(-20, -20, 1040, 370), bounds);
    }

    [Fact]
    public void BoundsMmOfNothingIsEmpty()
    {
        Assert.True(WallpaperSpanSlicer.ComputeBoundsMm([]).IsEmpty);
    }

    [Fact]
    public void TwoEqualScreensSplitTheImageInHalves()
    {
        // Two 500x300mm panels side by side, no bezels; image matches the box aspect exactly.
        var bounds = new Rect(0, 0, 1000, 300);
        var slices = WallpaperSpanSlicer.ComputeSlices(new Size(2000, 600), bounds, [
            Screen("A", 0, 0, 500, 300, 1920, 1080),
            Screen("B", 500, 0, 500, 300, 1920, 1080),
        ]);

        Assert.Equal(new Rect(0, 0, 1000, 600), CropOf(slices, "A"));
        Assert.Equal(new Rect(1000, 0, 1000, 600), CropOf(slices, "B"));
    }

    [Fact]
    public void MixedDpiScreensGetSameCropDifferentOutput()
    {
        // Same physical panels, one FHD one 4K: identical crops, different output sizes.
        var bounds = new Rect(0, 0, 1000, 300);
        var slices = WallpaperSpanSlicer.ComputeSlices(new Size(2000, 600), bounds, [
            Screen("FHD", 0, 0, 500, 300, 1920, 1080),
            Screen("4K", 500, 0, 500, 300, 3840, 2160),
        ]);

        Assert.Equal(CropOf(slices, "FHD").Size, CropOf(slices, "4K").Size);
        Assert.Equal(new Size(1920, 1080), slices.Single(s => s.Id == "FHD").OutputPx);
        Assert.Equal(new Size(3840, 2160), slices.Single(s => s.Id == "4K").OutputPx);
    }

    [Fact]
    public void BezelGapSeparatesCropsByScaledGap()
    {
        // 40mm of bezels between the panels: the image must skip gap*s pixels between crops.
        var bounds = new Rect(0, 0, 1040, 300);
        var slices = WallpaperSpanSlicer.ComputeSlices(new Size(2080, 600), bounds, [
            Screen("A", 0, 0, 500, 300, 1920, 1080),
            Screen("B", 540, 0, 500, 300, 1920, 1080),
        ]);

        var s = 2080.0 / 1040; // 2 px/mm
        Assert.Equal(40 * s, CropOf(slices, "B").Left - CropOf(slices, "A").Right, 6);
    }

    [Fact]
    public void AspectMismatchIsCenterCropped()
    {
        // Image twice as tall as needed: cover scale is driven by width, excess height
        // is split evenly above and below.
        var bounds = new Rect(0, 0, 1000, 300);
        var slices = WallpaperSpanSlicer.ComputeSlices(new Size(2000, 1200), bounds, [
            Screen("A", 0, 0, 1000, 300, 1920, 1080),
        ]);

        // s = 2 px/mm, box = 2000x600 px, offsetY = (1200-600)/2 = 300.
        Assert.Equal(new Rect(0, 300, 2000, 600), CropOf(slices, "A"));
    }

    [Fact]
    public void PortraitScreenGetsPortraitCrop()
    {
        var bounds = new Rect(0, 0, 800, 500);
        var slices = WallpaperSpanSlicer.ComputeSlices(new Size(1600, 1000), bounds, [
            Screen("L", 0, 0, 500, 300, 1920, 1080),
            Screen("P", 500, 0, 300, 500, 1080, 1920),
        ]);

        var crop = CropOf(slices, "P");
        Assert.True(crop.Height > crop.Width);
        Assert.Equal(new Rect(1000, 0, 600, 1000), crop);
    }

    [Fact]
    public void CropIsClampedToImageEdges()
    {
        // Perfect aspect but double-checking rounding hazards: a screen flush with the
        // box edge must never produce a crop outside the image.
        var bounds = new Rect(-500, -300, 1000, 600);
        var slices = WallpaperSpanSlicer.ComputeSlices(new Size(1000, 600), bounds, [
            Screen("A", -500, -300, 500, 600, 1920, 1080),
            Screen("B", 500 - 500, -300 + 300, 500, 300, 1920, 1080),
        ]);

        var image = new Rect(0, 0, 1000, 600);
        foreach (var slice in slices)
            Assert.True(image.Contains(slice.SourcePx), $"{slice.Id}: {slice.SourcePx} outside image");
    }

    [Fact]
    public void EmptyOrDegenerateInputsYieldNoSlices()
    {
        Assert.Empty(WallpaperSpanSlicer.ComputeSlices(new Size(0, 0), new Rect(0, 0, 100, 100), [Screen("A", 0, 0, 100, 100, 100, 100)]));
        Assert.Empty(WallpaperSpanSlicer.ComputeSlices(new Size(100, 100), Rect.Empty, [Screen("A", 0, 0, 100, 100, 100, 100)]));
    }
}
