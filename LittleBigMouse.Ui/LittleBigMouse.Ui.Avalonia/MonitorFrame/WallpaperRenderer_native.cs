using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using Avalonia;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using Bitmap = Avalonia.Media.Imaging.Bitmap;
using Size = System.Drawing.Size;

namespace LittleBigMouse.Ui.Avalonia.MonitorFrame;
/*
public class WallpaperRenderer
{
    Bitmap _source;
    Rectangle _bounds;

    public WallpaperRenderer(string path)
    {
        _source = new Bitmap(path);
    }

    public WallpaperRenderer(Bitmap source, Rectangle bounds)
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
        _bounds = new Rectangle(xMin,yMin,xMax - xMin,yMax - yMin);

        return this;
    }

    public WallpaperRenderer MakeTileWall()
    {
        // TODO : Avalonia
        //var img = new Image<Rgba32>(_bounds.Width, _bounds.Height);

        //_source.

        //img.Mutate(e =>
        //{
        //    for (var x = 0; x < _bounds.Width; x += _source.Width)
        //    for (var y = 0; y < _bounds.Height; y += _source.Height)
        //        e = e.DrawImage(_source, new Point(x, y), 1.0f);
        //});
        
        //_source = img;
        return new WallpaperRenderer(_source,_bounds);
    }

    public WallpaperRenderer MakeSpanWall()
    {
        var ratio = Math.Max((double)_bounds.Width / (double)_source.PixelSize.Width, (double)_bounds.Height / (double)_source.PixelSize.Height);

        var img = _source.CreateScaledBitmap(new PixelSize((int)(_source.PixelSize.Width * ratio),
            (int)(_source.PixelSize.Height * ratio)));


        return new WallpaperRenderer(_source.Clone(e => e
        .Resize(
            (int)((double)_source.Width * ratio), 
            (int)((double)_source.Height * ratio))
        ),_bounds).CropCenter(_bounds.Width, _bounds.Height);
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
        => new(_source.Clone(e => e.Crop(r)), _bounds);

    public WallpaperRenderer Fill(double width, double height)
    {
        var ratio = Math.Max( width / (double)_source.Width, height / (double)_source.Height);

        var img = _source.Clone(e => e
            .Resize(new Size((int)(ratio * _source.Width), (int)(ratio * _source.Height)))
        );

        return new WallpaperRenderer(img,_bounds);
    }

    public WallpaperRenderer Fit(int width, int height)
    {
        var ratio = Math.Min( width / (double)_source.Width, height / (double)_source.Height);

        var img = _source.Clone(e => e
            .Resize(new Size((int)(ratio * _source.Width), (int)(ratio * _source.Height)))
        );
        return new WallpaperRenderer(img, _bounds);
    }

    public WallpaperRenderer CropCenter(int width, int height)
    {
        height=Math.Min(height, _source.Height);
        width=Math.Min(width, _source.Width);

        var img = _source.Clone(e => e
            .Crop(new Rectangle(
                (int)(_source.Width - (double)width) / 2, 
                (int)(_source.Height - (double)height) / 2, 
                (int)width, 
                (int)height))
        );
        return new WallpaperRenderer(img, _bounds);
    }

    public WallpaperRenderer Stretch(int width, int height) 
        => new WallpaperRenderer(_source
            .Clone(e => e.Resize(new Size(width, height))),_bounds  );

    public Bitmap ToBitmap()
    {
        using var ms = new MemoryStream();
        _source.Save(ms, PngFormat.Instance);
        ms.Seek(0, SeekOrigin.Begin);
        return new Bitmap(ms);
    }

}*/