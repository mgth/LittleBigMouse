using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Media;
using LittleBigMouse.ScreenConfig;

namespace LittleBigMouse.Daemon
{
    public class Zones
    {
        public void Add(Zone zone)
        {
            All.Add(zone);
            if(ReferenceEquals(zone.Main,zone)) Main.Add(zone);
        }

        public Zone FromPx(Point px) => Main.FirstOrDefault(zone => zone.ContainsPx(px));
        public Zone FromMm(Point mm) => All.FirstOrDefault(zone => zone.ContainsMm(mm));

        public List<Zone> All { get; } = new List<Zone>();
        public List<Zone> Main { get; } = new List<Zone>();

    }

    public class Zone
    {
        public readonly Rect Px;
        public readonly Rect Mm;
        public readonly Zone Main;
        public  Rect Capture;

        public readonly double Dpi;

        private readonly Matrix _px2Mm;
        private readonly Matrix _mm2Px;

        //public Screen Screen { get; }

        public Zone(Screen screen,Zone main = null,double translateX=0, double translateY = 0)
        {
            //Screen = screen;

            main ??= this;

            Main = main;

            Px = screen.InPixel.Bounds;
            //Px.Width -= 1;
            //Px.Height -= 1;

            var mm = screen.InMm.Bounds;
            var matrix = new Matrix();
            matrix.Translate(translateX,translateY);
            mm.Transform(matrix);
            Mm = mm;


            var px2Mm = new Matrix();
            px2Mm.Translate(-Px.X,-Px.Y);
            px2Mm.Scale(1/ Px.Width, 1/ Px.Height);
            px2Mm.Scale(Mm.Width, Mm.Height);
            px2Mm.Translate(Mm.X, Mm.Y);
            _px2Mm = px2Mm;

            var mm2Px = new Matrix();
            mm2Px.Translate(-Mm.X, -Mm.Y);
            mm2Px.Scale(1 / Mm.Width, 1 / Mm.Height);
            mm2Px.Scale(Px.Width, Px.Height);
            mm2Px.Translate(Px.X, Px.Y);
            _mm2Px = mm2Px;

            var dpiX = Px.Width / (Mm.Width / 25.4);
            var dpiY = Px.Height / (Mm.Height / 25.4);

            Dpi = Math.Sqrt(dpiX * dpiX + dpiY * dpiY) / Math.Sqrt(2);
        }

        public Point Px2Mm(Point px) => px * _px2Mm;

        public Point Mm2Px(Point mm) => mm * _mm2Px;

        public bool ContainsPx(Point px)
        {
            if (px.X < Px.X) return false;
            if (px.Y < Px.Y) return false;
            if (px.X >= Px.Right) return false;
            if (px.Y >= Px.Bottom) return false;
            return true;
        }

        public bool ContainsMm(Point mm) => Mm.Contains(mm);

        public Point InsidePx(Point px)
        {
            if (px.X < Px.X) px.X = Px.X;
            else if (px.X > Px.Right - 1.0) px.X = Px.Right - 1.0;

            if (px.Y < Px.Y) px.Y = Px.Y;
            else if (px.Y > Px.Bottom - 1.0) px.Y = Px.Bottom - 1.0;

            return px;
        }
        public Point InsideMm(Point mm)
        {
            if (mm.X < Mm.X) mm.X = Mm.X;
            else if (mm.X > Mm.Right) mm.X = Mm.Right;

            if (mm.Y < Mm.Y) mm.Y = Mm.Y;
            else if (mm.Y > Mm.Bottom) mm.Y = Mm.Bottom;

            return mm;
        }
    }
}
