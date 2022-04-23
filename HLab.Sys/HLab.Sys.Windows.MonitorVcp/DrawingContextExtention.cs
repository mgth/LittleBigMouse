using System;
using System.Diagnostics;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace HLab.Sys.Windows.MonitorVcp
{
    public static class DrawingContextExtension
    {
        public static void DrawContrast(this DrawingContext dc, Color colorA,  Color colorB, Rect area, int count, Orientation orientation)
        {
            var hz = orientation == Orientation.Horizontal;
            var length = (hz?area.Width:area.Height)/count;
            var w = (hz?area.Height:area.Width)/2;

            var location = area.Location;
            var size = hz?new Size(length,w):new Size(w,length);
            var move = hz ? new Vector(length, 0) : new Vector(0, length);

            var dR = colorA.R < colorB.R ? +1 : colorA.R > colorB.R? -1 : 0;
            var dG = colorA.G < colorB.G ? +1 : colorA.G > colorB.G? -1 : 0;
            var dB = colorA.B < colorB.B ? +1 : colorA.B > colorB.B? -1 : 0;


            var color = colorA;
            for (int i = 0; i < count; i++)
            {
                color = new Color{A=255,R= (byte)((int)color.R+dR), G = (byte)((int)color.G + dG) , B = (byte)((int)color.B + dB) };
                dc.RenderCircle(color,colorA,new Rect(location,size));
                location += move;
            }

            location = area.Location + (hz ? new Vector(0,w) : new Vector(w,0));

            dR = -dR;
            dG = -dG;
            dB = -dB;

            color = colorB;
            for (int i = 0; i < count; i++)
            {
                color = new Color{A=255,R= (byte)((int)color.R+dR), G = (byte)((int)color.G + dG) , B = (byte)((int)color.B + dB) };
                dc.RenderCircle(color,colorB,new Rect(location,size));
                location += move;
            }


        }

        public static void DrawChessboard(this DrawingContext dc, Color colorA, Color colorB, Rect area, Size size)
        {
            var brushA = new SolidColorBrush(colorA);
            var brushB = new SolidColorBrush(colorB);

            var brushC =
                new DrawingBrush(
                    new GeometryDrawing
                    {
                        Brush = brushB,
                        Geometry = new GeometryGroup
                        {
                            Children =
                            {
                                new RectangleGeometry(new Rect(0, 0, 1, 1)),
                                new RectangleGeometry(new Rect(
                                    1, 1,
                                    1, 1))
                            }
                        }
                    }
                )
                {
                    TileMode = TileMode.Tile,
                    Viewport = new Rect(0, 0, 2 * size.Width / area.Width, 2 * size.Height / area.Height),
                    AlignmentX = AlignmentX.Left,
                    AlignmentY = AlignmentY.Top

                };

            dc.DrawRectangle(brushA, null, area);

            dc.DrawRectangle(brushC, null, area);
        }
        public static void DrawGamma(this DrawingContext dc, Color colorA, Color colorB, Rect area, Orientation orientation)
        {
            const double startGamma = 1.15;
            const double endGamma = 3.05;

            var hz = orientation == Orientation.Horizontal;

            float _g(float value, double gg)
            {
                return (float)Math.Pow(value, /*1.0f / */2.2 / gg);
            }
            var startColor = Color.FromScRgb(1.0f, _g(colorA.ScR/2.0f, startGamma), _g(colorA.ScG/2.0f, startGamma), _g(colorA.ScB/2.0f, startGamma));
            var endColor = Color.FromScRgb(1.0f, _g(colorA.ScR/2.0f, endGamma), _g(colorA.ScG/2.0f, endGamma), _g(colorA.ScB/2.0f, endGamma));

            dc.DrawGradient(startColor, endColor, area, orientation, 1.0);

            double d1 = hz?(area.Y + area.Height / 3):(area.X + area.Width / 3);
            double d2 = d1 + (hz?area.Height:area.Width) / 3;
            double middle = (hz?area.Top + area.Bottom:area.Left + area.Right) / 2;

            if (hz)
            {
                dc.DrawChessboard(colorA, colorB, new Rect(area.X, d1, area.Width, Math.Abs(d2 - d1)),new Size(1,1));
                dc.DrawLine(
                    new Pen(new SolidColorBrush(colorB), area.Height / 9.0), 
                    new Point(0, middle), 
                    new Point(area.Right, middle));
            }
            else
            {
                dc.DrawChessboard(colorA, colorB, new Rect(d1, area.Y, Math.Abs(d2 - d1), area.Height),new Size(1,1));
                dc.DrawLine(
                    new Pen(new SolidColorBrush(colorB), area.Width / 9.0), 
                    new Point(middle, 0), 
                    new Point(middle, area.Bottom));
            }


            var gc = new GradientCalculator(startColor, endColor, 1.0);

            var length = hz?area.Width:area.Height;
            var d = length;


            var start = hz ? area.Left : area.Top;
            var end = hz ? area.Right : area.Bottom;

            var ratio = (end - start) / (endGamma - startGamma);

            for (var g = startGamma; g<endGamma; g +=0.1)
            {
                d = start + (g - startGamma)*ratio;

                double thickness = 1.0;
                if (Math.Abs(g - 1.0) < 0.1) thickness = 3.0;
                if (Math.Abs(g - 1.8) < 0.1) thickness = 3.0;
                if (Math.Abs(g - 2.2) < 0.1) thickness = 3.0;

                var c = gc.Get(d/length);

                if(hz)
                    dc.DrawLine(new Pen(new SolidColorBrush(c), thickness), new Point(d, d1 ), new Point(d, d2 ));
                else
                    dc.DrawLine(new Pen(new SolidColorBrush(c), thickness), new Point(d1, d ), new Point( d2, d ));
            }

            //if (hz)
            //    dc.DrawRectangle(new SolidColorBrush(colorB), null, new Rect(area.X,d1,d-area.X,Math.Abs(d2-d1)));
            //else
            //    dc.DrawRectangle(new SolidColorBrush(colorB), null, new Rect(d1,area.Y,Math.Abs(d2-d1),d-area.Y));

            var old = (hz ? area.Width:area.Height) * 2;
            d = hz?area.Right:area.Bottom;

            double size = ratio * 0.05;


            //Writing decimal gamma values
            for (var g = startGamma + 0.05; g < endGamma ; g += 0.1)
            {
                d = start + (g - startGamma) * ratio;

                var t = new FormattedText(g.ToString("N1"), CultureInfo.CurrentCulture, FlowDirection.LeftToRight, new Typeface("Segoe UI"), size, new SolidColorBrush(colorA), 1);

                if(hz)
                    dc.DrawText(t,new Point(d - (t.Width/2),middle - (t.Height/2)));
                else
                    dc.DrawText(t,new Point(middle - (t.Width/2), d- (t.Height/2)));
            }

            //var nx = area.Width / block.Width;
            //var ny = area.Height / block.Height;

            //for (int y = 0; y < ny; y++)
            //for (int x = 0; x < nx; x++)
            //{
            //    if (x % 2 == y % 2)
            //        dc.DrawRectangle(brushB, null,
            //            new Rect(x * block.Width, y * block.Height, block.Width, block.Height));
            //}
        }

        public static void DrawGradient(this DrawingContext dc, Color colorA, Color colorB, Rect area, Orientation orientation, double gamma=2.2)
        {
            var gc = new GradientCalculator(colorA,colorB,gamma);

            var brush = new SolidColorBrush();

            var length = orientation == Orientation.Horizontal ? area.Width : area.Height;

            if (length>500) { }

            var bench = new Stopwatch();
            bench.Start();

            if (orientation == Orientation.Horizontal)
            {
                for (var p = 0.0; p < length; p++)
                {
                    var color = gc.Get(p / length);
                    dc.DrawRectangle(new SolidColorBrush(color), null, new Rect(p - 0.5, area.Y, 2.0, area.Height));
                }
            }
            else
            {
                for (var p = 0.0; p < length; p++)
                {
                    var color = gc.Get(p / length);
                    dc.DrawRectangle(new SolidColorBrush(color), null, new Rect( area.X,p - 0.5, area.Width, 2.0));
                }
                
            }

            bench.Stop();
            Debug.WriteLine("Gradient : " + area + " : " + bench.ElapsedTicks);

        }

        private class GradientCalculator
        {
            private readonly double _gamma; 
            private readonly double _invgamma; 

            private readonly float _aa;
            private readonly float _ra;
            private readonly float _ga;
            private readonly float _ba;

            private readonly double _da;
            private readonly double _dr;
            private readonly double _dg;
            private readonly double _db;
            public GradientCalculator(Color colorA, Color colorB, double gamma)
            {
                _gamma = gamma;
                _invgamma = 1.0f / gamma;

                _aa = colorA.ScA;
                _ra = colorA.ScR;
                _ga = colorA.ScG;
                _ba = colorA.ScB;

                _da = ((double)colorB.ScA - _aa);
                _dr = ((double)colorB.ScR - _ra);
                _dg = ((double)colorB.ScG - _ga);
                _db = ((double)colorB.ScB - _ba);
            }

            float g(double value) => (float)Math.Pow(value, _invgamma);

            public Color Get(double p)
            {

                return Color.FromScRgb(
                    g(_aa + _da * p),
                    g(_ra + _dr * p),
                    g(_ga + _dg * p),
                    g(_ba + _db * p));

            }
        }


        public static void RenderCircle(this DrawingContext dc, Color colorA, Color colorB, Rect area)
        {
            dc.DrawRectangle(new SolidColorBrush(colorB), null, area);
            double rayon = Math.Min(area.Width, area.Height) * 0.45;
            dc.DrawEllipse(new SolidColorBrush(colorA), null, new Point(area.X + area.Width * 0.5,  area.Y + area.Height * 0.5),
                rayon, rayon);
        }
        public static void DrawHomeCinemaPattern(this DrawingContext dc, Rect area)
        {
            //this.UseLayoutRounding = false;
            Pen pGray = new Pen(new SolidColorBrush(Color.FromRgb(50, 50, 50)), 1.0);
            Pen pRed = new Pen(new SolidColorBrush(Color.FromRgb(100, 0, 0)), 1.0);
            dc.DrawRectangle(Brushes.Black, null, area);

            // Lignes verticales
            //dc.DrawLine(pGray, new Point(10.5, 0.0), new Point(10.5, area.Height));
            //dc.DrawLine(pGray, new Point(30.5, 0.0), new Point(30.5, area.Height));
            dc.DrawLine(pGray, new Point(area.Width * 0.5, 0.0), new Point(area.Width * 0.5, area.Height));
            //dc.DrawLine(pGray, new Point(area.Width - 30.5, 0.0), new Point(area.Width - 30.5, area.Height));
            //dc.DrawLine(pGray, new Point(area.Width - 10.5, 0.0), new Point(area.Width - 10.5, area.Height));

            // Lignes horizontales
            //dc.DrawLine(pGray, new Point(0.0, 10.5), new Point(area.Width, 10.5));
            //dc.DrawLine(pGray, new Point(0.0, 30.5), new Point(area.Width, 30.5));
            dc.DrawLine(pGray, new Point(0.0, area.Height * 0.5), new Point(area.Width, area.Height * 0.5));
            //dc.DrawLine(pGray, new Point(0.0, area.Height - 30.5), new Point(area.Width, area.Height - 30.5));
            //dc.DrawLine(pGray, new Point(0.0, area.Height - 10.5), new Point(area.Width, area.Height - 10.5));

            // Croix
            dc.DrawLine(pGray, new Point(0.0, 0.0), new Point(area.Width, area.Height));
            dc.DrawLine(pGray, new Point(area.Width, 0.0), new Point(0.0, area.Height));

            // 2.35
            double h = Math.Round(area.Width / 2.35);
            double pos = area.Height - h + 0.5;// Math.Round((ActualHeight - hauteur)*0.5)+0.5;

            // Lignes horizontales
            //dc.DrawLine(pRed, new Point(0.0, pos + 10.0), new Point(area.Width, pos + 10.0));
            //dc.DrawLine(pRed, new Point(0.0, pos + 30.0), new Point(area.Width, pos + 30.0));
            //dc.DrawLine(pRed, new Point(0.0, pos + h - 30.0), new Point(area.Width, pos + h - 30.0));
            //dc.DrawLine(pRed, new Point(0.0, pos + h - 10.0), new Point(area.Width, pos + h - 10.0));

            // Croix
            dc.DrawLine(pRed, new Point(0.0, pos), new Point(area.Width, pos + h));
            dc.DrawLine(pRed, new Point(area.Width, pos), new Point(0.0, pos + h));

            // Cadre
            dc.DrawRectangle(null, new Pen(Brushes.White, 1.0), new Rect(0.5, 0.5, Math.Max(0,area.Width - 1.0), Math.Max(0,area.Height - 1.0)));
            dc.DrawRectangle(null, new Pen(Brushes.Red, 1.0), new Rect(0.5, pos, Math.Max(0,area.Width - 1.0), Math.Max(0,h - 1.0)));
        }
    }
}
