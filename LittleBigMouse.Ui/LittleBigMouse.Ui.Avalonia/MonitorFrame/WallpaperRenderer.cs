using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using DynamicData.Kernel;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Advanced;
using SixLabors.ImageSharp.Formats.Jpeg;
using SixLabors.ImageSharp.Formats.Png;
using SixLabors.ImageSharp.Memory;
using SixLabors.ImageSharp.PixelFormats;
using SixLabors.ImageSharp.Processing;
using Point = SixLabors.ImageSharp.Point;
using Size = SixLabors.ImageSharp.Size;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

public class WallpaperRenderer
{
    Image<Rgba32>? _source;
    readonly Rectangle _bounds;

    public static WallpaperRenderer Load(string path)
    {
        using var img = Image.Load(path);
        return new WallpaperRenderer(img.CloneAs<Rgba32>(), new Rectangle());
    }

    WallpaperRenderer(Image<Rgba32> source, Rectangle bounds)
    {
        _source = source;
        _bounds = bounds;
    }

    public WallpaperRenderer Measure(IEnumerable<PhysicalSource> sources)
    {
        var xMin = 0;
        var yMin = 0;
        var xMax = 0;
        var yMax = 0;

        foreach (var source in sources)
        {
            var d = source.Source.InPixel;

            xMin = Math.Min(xMin, (int)d.X);
            yMin = Math.Min(yMin, (int)d.Y);
            xMax = Math.Max(xMax, (int)d.X + (int)d.Width);
            yMax = Math.Max(yMax, (int)d.Y + (int)d.Height);
        }
        var bounds = new Rectangle(xMin,yMin,xMax - xMin,yMax - yMin);

        return ReturnAndDispose(bounds);
    }

    public WallpaperRenderer MakeTileWall()
    {
        using var source = _source;
        _source = null;

        var img = new Image<Rgba32>(_bounds.Width, _bounds.Height);

        img.Mutate(e =>
        {
            for (var x = 0; x < _bounds.Width; x += source.Width)
            for (var y = 0; y < _bounds.Height; y += source.Height)
                e = e.DrawImage(source, new Point(x, y), 1.0f);
        });
        
        return Return(img);
    }

    public WallpaperRenderer MakeSpanWall()
    {
        using var source = _source;
        _source = null;

        var ratio = Math.Max((double)_bounds.Width / (double)source.Width, (double)_bounds.Height / (double)source.Height);

        return Return(source.Clone(e => e
        .Resize(
            (int)((double)source.Width * ratio), 
            (int)((double)source.Height * ratio))
        )).CropCenter(_bounds.Width, _bounds.Height);
    }

    public WallpaperRenderer Crop(IDisplaySize size)
    {
        return Crop(new Rectangle(
            (int)(size.X - _bounds.X),
            (int)(size.Y - _bounds.Y),
            (int)size.Width,
            (int)size.Height));
    }

    public WallpaperRenderer Crop(Rectangle r)
    {
        using var source = _source;
        _source = null;

        return Return(source.Clone(e => e.Crop(r)));
    }

    public WallpaperRenderer Fill(double width, double height)
    {
        using var source = _source;
        _source = null;


        var ratio = Math.Max( width / (double)source.Width, height / (double)source.Height);

        return Return(
             source.Clone
            (e => e
                .Resize(new Size((int)(ratio * source.Width), (int)(ratio * source.Height)))
            )
        );
    }

    public WallpaperRenderer Fit(int width, int height)
    {
        using var source = _source;
        _source = null;

        var ratio = Math.Min( width / (double)source.Width, height / (double)source.Height);

        return Return(
             source.Clone(e => e
            .Resize(new Size((int)(ratio * source.Width), (int)(ratio * source.Height)))
        )
        );
    }

    public WallpaperRenderer CropCenter(int width, int height)
    {
        using var source = _source;
        _source = null;

        height=Math.Min(height, source.Height);
        width=Math.Min(width, source.Width);

        return Return( 
            source.Clone(e => e
            .Crop(new Rectangle(
                (int)(source.Width - (double)width) / 2, 
                (int)(source.Height - (double)height) / 2, 
                (int)width, 
                (int)height)))
            );
    }

    public WallpaperRenderer Stretch(int width, int height)
    {   
        using var source = _source;
        _source = null;

        return Return(
            source.Clone(e => e.Resize(new Size(width, height)))
        );
    }

    static async Task Bench(string name, Func<Task> a)
    {
        var s = new Stopwatch();
        s.Start();
        await a();
        s.Stop();
        Debug.WriteLine($"SaveAs{name} {s.ElapsedMilliseconds}ms");
    }


    public static Bitmap GetBitmapFromImage(Image<Rgba32> image)
    {
        var size = new PixelSize(image.Width, image.Height);

        var wb = new WriteableBitmap(size, new Vector(96,96), PixelFormat.Bgra8888);

        var pixels = new byte[image.Width * image.Height * 4];
        image.CopyPixelDataTo(pixels);

        Marshal.Copy(pixels, 0, wb.Lock().Address, image.Width * image.Height * 4);

        return wb;
    }


    public async Task<Bitmap> ToBitmapAsync()
    {
        //return GetBitmapFromImage(_source);

        using var source = _source;
        _source = null;

        await using var ms = new MemoryStream(source.Height*source.Width*(source.PixelType.BitsPerPixel>>3));

        await source.SaveAsBmpAsync(ms);
        //await source.SaveAsJpegAsync(ms);
        //await source.SaveAsPngAsync(ms);

        //_source.SaveAsync(ms, PngFormat.Instance);
        //await Bench("BMP ", () => _source.SaveAsBmpAsync(ms));
        //ms.Position = 0;
        //await Bench("GIF ", () => _source.SaveAsGifAsync(ms));
        //ms.Position = 0;
        //await Bench("JPEG", () => _source.SaveAsJpegAsync(ms));
        //ms.Position = 0;
        //await Bench("PBM ", () => _source.SaveAsPbmAsync(ms));
        //ms.Position = 0;
        //await Bench("WEBP", () => _source.SaveAsWebpAsync(ms));
        //ms.Position = 0;
        //await Bench("TIFF", () => _source.SaveAsTiffAsync(ms));
        //ms.Position = 0;
        //await Bench("TGA ", () => _source.SaveAsTgaAsync(ms));
        //ms.Position = 0;
        //await Bench("PNG ", () => _source.SaveAsPngAsync(ms));

        ms.Position = 0;
        return new Bitmap(ms);
    }
    WallpaperRenderer ReturnAndDispose(Rectangle bounds)
    {
        return new WallpaperRenderer(_source, bounds);
    }

    WallpaperRenderer Return(Image<Rgba32> src) => new(src, _bounds);

    WallpaperRenderer Return(Image<Rgba32> src, Rectangle bounds) => new(src, bounds);
}