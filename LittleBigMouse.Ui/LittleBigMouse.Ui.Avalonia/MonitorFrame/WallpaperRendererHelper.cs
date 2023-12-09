using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using SixLabors.ImageSharp;
using SixLabors.ImageSharp.Diagnostics;
using SixLabors.ImageSharp.PixelFormats;
using SixLabors.ImageSharp.Processing;

using AvaloniaColor = Avalonia.Media.Color;
using AvaloniaSize = Avalonia.Size;
using AvaloniaPoint = Avalonia.Point;

using Point = SixLabors.ImageSharp.Point;
using Size = SixLabors.ImageSharp.Size;
using Color = SixLabors.ImageSharp.Color;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;

public static class WallpaperRendererHelper
{
    public static Task<Bitmap> GetWallpaperFillAsync(string path, AvaloniaSize size, int shrink) 
        => Image.LoadAsync(path)
            .MutateFluent(e => e.Shrink(shrink).Fill(size.ToImageSharp()))
            .ToBitmapAsync();

    public static Task<Bitmap> GetWallpaperCenterAsync(string path, AvaloniaSize size, AvaloniaColor color, int shrink) 
        => Image.LoadAsync(path)
            .MutateFluent(e => e.Shrink(shrink).Center(size.ToImageSharp(),color.ToImageSharp()))
            .ToBitmapAsync();

    public static Task<Bitmap> GetWallpaperFitAsync(string path, AvaloniaSize size, AvaloniaColor color, int shrink)
        => Image.LoadAsync(path)
            .MutateFluent(e => e.Shrink(shrink).Fit(size.ToImageSharp(), color.ToImageSharp()))
            .ToBitmapAsync();

    public static Task<Bitmap> GetWallpaperStretchAsync(string path, AvaloniaSize size, int shrink) 
        => Image.LoadAsync(path)
            .MutateFluent(e => e.Shrink(shrink).Stretch(size.ToImageSharp()))
            .ToBitmapAsync();

    public static Task<Bitmap> GetWallpaperTileAsync(string path, Rect target, Rect fullArea, int shrink) 
        => MakeTileWall(path, fullArea.Size.ToImageSharp(), shrink)
            .MutateFluent(e => e.SpanOrigin(target.ToImageSharp(),fullArea.ToImageSharp()))
            .ToBitmapAsync();

    public static Task<Bitmap> GetWallpaperSpanAsync(string path, Rect target, Rect fullArea, int shrink) 
        => Image.LoadAsync(path)
            .MutateFluent(e => e.Shrink(shrink).SpanCenter(target.ToImageSharp(),fullArea.ToImageSharp()))
            .ToBitmapAsync();



    static async Task<Image> MakeTileWall(string path, Size size, int shrink)
    {
        using var source =/*await (*/await Image.LoadAsync(path).MutateFluent(ctx => ctx.Shrink(shrink));//).ToRgba32Async();

        return await Task.Run(() =>
        {
            var img = new Image<Rgba32>(size.Width, size.Height);

            img.Mutate(e =>
            {
                for (var x = 0; x < size.Width; x += source.Width)
                for (var y = 0; y < size.Height; y += source.Height)
                {
                    var w = Math.Min(source.Width, size.Width - x);
                    var h = Math.Min(source.Height, size.Height - y);

                    e = e.DrawImage(source,new Point(x,y), new Rectangle(0, 0,w,h),1.0f);
                }
            });
            
            return img;
        });
    }

    static Color ToImageSharp(this AvaloniaColor color)
    {
        return Color.FromRgba(color.R, color.G, color.B, color.A);
    }

    static Size ToImageSharp(this AvaloniaSize size)
    {
        return new Size((int)size.Width,(int)size.Height);
    }
    static Point ToImageSharp(this AvaloniaPoint point)
    {
        return new Point((int)point.X,(int)point.Y);
    }
    static Rectangle ToImageSharp(this Rect rect)
    {
        return new Rectangle(rect.Position.ToImageSharp(),rect.Size.ToImageSharp());
    }

    public static async Task<Bitmap> ToBitmapAsync(this Task<Image> source)
    {
        return await (await source).ToBitmapAsync();
    }
    public static async Task<Image> ToRgba32Async(this Image source)
    {
        if (source is Image<Rgba32> img)
        {
            return img;
        }

        return await Task.Run(source.CloneAs<Rgba32>);
    }

    public static Task<Bitmap> ToBitmapAsync(this Image source)
    {
        if (source is Image<Rgba32> img)
        {
            return img.ToBitmapAsync();
        }

        var result = source.CloneAs<Rgba32>();
        source.Dispose();
        return result.ToBitmapAsync();
    }

    public static async Task<Bitmap> ToBitmapAsync(this Image<Rgba32> source)
    {
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
        var result = new Bitmap(ms);
        source.Dispose();
        return result;
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
    static async Task<Image> MutateFluent(this Task<Image> image, Action<IImageProcessingContext> action)
    {
        return await (await image).MutateFluent(action);
    }

    static async Task<Image> MutateFluent(this Image image, Action<IImageProcessingContext> action)
    {
        await System.Threading.Tasks.Task.Run(() => image.Mutate(action));
        return image;
    }

    static IImageProcessingContext Shrink(this IImageProcessingContext ctx, int shrink)
    {
        if (shrink == 1) return ctx;

        var source = ctx.GetCurrentSize();

        var resize = new Size((int)(source.Width / shrink),(int)(source.Height / shrink));

        return ctx.Resize(resize);
    }

    static IImageProcessingContext Fill(this IImageProcessingContext ctx, Size target)
    {
        var source = ctx.GetCurrentSize();

        var ratio = Math.Max( target.Width / (double)source.Width, target.Height / (double)source.Height);

        var w = (int)(target.Width/ratio);
        var h = (int)(target.Height/ratio);

        var crop = new Rectangle(
            (source.Width - w) / 2,
            (source.Height - h) / 2,
            w, h);

        return ctx.Crop(crop).Resize(target);
    }

    static IImageProcessingContext Center(this IImageProcessingContext ctx, Size target, Color color)
    {
        var source = ctx.GetCurrentSize();
        var width = Math.Min(source.Width, target.Width);
        var height = Math.Min(source.Height, target.Height);

        var crop = new Rectangle(0,0, width, height);

        return ctx.Crop(crop).Pad(target.Width, target.Height, color);
     }

    static IImageProcessingContext Fit(this IImageProcessingContext ctx, Size target, Color color)
    {
        var source = ctx.GetCurrentSize();

        var ratio = Math.Min( target.Width / (double)source.Width, target.Height / (double)source.Height);

        var resize = new Size((int)(ratio * source.Width),(int)(ratio * source.Height));

        var x = (target.Width - resize.Width) / 2;
        var y = (target.Height - resize.Height) / 2;

        return ctx.Resize(resize).Pad(target.Width, target.Height, color);
    }

    static IImageProcessingContext Stretch(this IImageProcessingContext ctx, Size target)
    {
        return ctx.Resize(target);
    }

    static IImageProcessingContext SpanOrigin(this IImageProcessingContext ctx, Rectangle target, Rectangle fullArea)
    {
        var source = ctx.GetCurrentSize();

        var ratio = Math.Max((double)fullArea.Width / (double)source.Width, (double)fullArea.Height / (double)source.Height);
        var resize = new Size((int)(ratio * source.Width),(int)(ratio * source.Height));

        target.Offset(-fullArea.X,-fullArea.Y);

        return ctx
            .Resize(resize)
            .Crop(target);
    }

    static IImageProcessingContext SpanCenter(this IImageProcessingContext ctx, Rectangle target, Rectangle fullArea)
    {
        var source = ctx.GetCurrentSize();

        var ratio = Math.Max((double)fullArea.Width / (double)source.Width, (double)fullArea.Height / (double)source.Height);
        var resize = new Size((int)(ratio * source.Width),(int)(ratio * source.Height));

        var offsetX = (resize.Width - fullArea.Width) / 2;
        var offsetY = (resize.Height - fullArea.Height) / 2;

        target.Offset(-fullArea.X + offsetX,-fullArea.Y + offsetY);

        return ctx
            .Resize(resize)
            .Crop(target);
    }

    public static void ImageSharpDebugStats()
    {
        Debug.WriteLine(@$"Number of undisposed ImageSharp buffers: {MemoryDiagnostics.TotalUndisposedAllocationCount}");

    }
}

