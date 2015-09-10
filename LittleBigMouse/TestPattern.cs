using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace LittleBigMouse
{
    public enum TestPatternType
    {
        Solid,
        Gradient,
        RGBGradient,
        Circle,
        Grid
    }

    class TestPatternButton : Button
    {
        TestPattern _pattern = new TestPattern();
        public TestPatternType PatternType
        {
            get { return _pattern.PatternType; }
            set
            {
                _pattern.PatternType = value;
            }
        }
        public Color PatternColor
        {
            get { return _pattern.PatternColor; }
            set
            {
                _pattern.PatternColor = value;
            }
        }

        public TestPatternButton()
        {
            this.
            Content = _pattern;
            _pattern.InvalidateVisual();

        }

    }
    class TestPattern : Viewbox
    {
        public TestPattern()
        {
            Stretch = Stretch.Fill;
            MinWidth = 100;
            MinHeight = 25;
        }

        private TestPatternType _pattern = TestPatternType.Solid;
        public TestPatternType PatternType { get { return _pattern; }
            set {
                _pattern = value;
                InvalidateVisual();
            } }
        public Color _color = Colors.Black;
        public Color PatternColor { get { return _color; }
            set {
                _color = value;
                InvalidateVisual();
            }
        }

        private void DrawGradient(DrawingContext dc, Color color, int count=1, int hpos=0)
        {
            double r = (double)color.R / 255;
            double v = (double)color.G / 255;
            double b = (double)color.B / 255;

            double w = (ActualWidth / 256.0);
            double h = (ActualHeight / count);

            for (int i = 0; i < 256; i++)
            {
                double wpos = w * i;
                dc.DrawRectangle(
                    new SolidColorBrush(
                        Color.FromRgb(
                            (byte)(i * r),
                            (byte)(i * v),
                            (byte)(i * b)
                            )
                        ), null, new Rect(wpos, hpos*h, w + 1.0, h));

            }
        }

        protected override void OnRender(DrawingContext dc)
        {
            switch (_pattern)
            {
                // Solid colors
                case TestPatternType.Solid:
                    dc.DrawRectangle(new SolidColorBrush(_color), null, new Rect(0, 0, ActualWidth, ActualHeight));
                    break;
                // Linear Gradients
                case TestPatternType.Gradient:
                    {
                        DrawGradient(dc, _color);
                    }
                    break;
                case TestPatternType.RGBGradient:
                    DrawGradient(dc, Colors.White,5,0);
                    DrawGradient(dc, Colors.Red, 5, 1);
                    DrawGradient(dc, Colors.Lime, 5, 2);
                    DrawGradient(dc, Colors.Blue, 5, 3);
                    DrawGradient(dc, Colors.White, 5, 4);
                    break;

                case TestPatternType.Circle:
                    {
                        dc.DrawRectangle(Brushes.Black, null, new Rect(0, 0, ActualWidth, ActualHeight));
                        double rayon = Math.Min(ActualWidth, ActualHeight) * 0.45;
                        dc.DrawEllipse(new SolidColorBrush(_color), null, new Point(ActualWidth * 0.5, ActualHeight * 0.5), rayon, rayon);
                    }
                    break;
                // Grille
                case TestPatternType.Grid:
                    {
                        //this.UseLayoutRounding = false;
                        Pen pGray = new Pen(new SolidColorBrush(Color.FromRgb(50, 50, 50)), 1.0);
                        Pen pRed = new Pen(new SolidColorBrush(Color.FromRgb(100, 0, 0)), 1.0);
                        dc.DrawRectangle(Brushes.Black, null, new Rect(0, 0, ActualWidth, ActualHeight));

                        // Lignes verticales
                        dc.DrawLine(pGray, new Point(10.5, 0.0), new Point(10.5, ActualHeight));
                        dc.DrawLine(pGray, new Point(30.5, 0.0), new Point(30.5, ActualHeight));
                        dc.DrawLine(pGray, new Point(ActualWidth * 0.5, 0.0), new Point(ActualWidth * 0.5, ActualHeight));
                        dc.DrawLine(pGray, new Point(ActualWidth - 30.5, 0.0), new Point(ActualWidth - 30.5, ActualHeight));
                        dc.DrawLine(pGray, new Point(ActualWidth - 10.5, 0.0), new Point(ActualWidth - 10.5, ActualHeight));

                        // Lignes horizontales
                        dc.DrawLine(pGray, new Point(0.0, 10.5), new Point(ActualWidth, 10.5));
                        dc.DrawLine(pGray, new Point(0.0, 30.5), new Point(ActualWidth, 30.5));
                        dc.DrawLine(pGray, new Point(0.0, ActualHeight * 0.5), new Point(ActualWidth, ActualHeight * 0.5));
                        dc.DrawLine(pGray, new Point(0.0, ActualHeight - 30.5), new Point(ActualWidth, ActualHeight - 30.5));
                        dc.DrawLine(pGray, new Point(0.0, ActualHeight - 10.5), new Point(ActualWidth, ActualHeight - 10.5));

                        // Croix
                        dc.DrawLine(pGray, new Point(0.0, 0.0), new Point(ActualWidth, ActualHeight));
                        dc.DrawLine(pGray, new Point(ActualWidth, 0.0), new Point(0.0, ActualHeight));

                        // 2.35
                        double height = Math.Round(ActualWidth / 2.35);
                        double pos = ActualHeight - height + 0.5;// Math.Round((ActualHeight - hauteur)*0.5)+0.5;

                        // Lignes horizontales
                        dc.DrawLine(pRed, new Point(0.0, pos + 10.0), new Point(ActualWidth, pos + 10.0));
                        dc.DrawLine(pRed, new Point(0.0, pos + 30.0), new Point(ActualWidth, pos + 30.0));
                        dc.DrawLine(pRed, new Point(0.0, pos + height - 30.0), new Point(ActualWidth, pos + height - 30.0));
                        dc.DrawLine(pRed, new Point(0.0, pos + height - 10.0), new Point(ActualWidth, pos + height - 10.0));

                        // Croix
                        dc.DrawLine(pRed, new Point(0.0, pos), new Point(ActualWidth, pos + height));
                        dc.DrawLine(pRed, new Point(ActualWidth, pos), new Point(0.0, pos + height));

                        // Cadre
                        dc.DrawRectangle(null, new Pen(Brushes.White, 1.0), new Rect(0.5, 0.5, ActualWidth - 1.0, ActualHeight - 1.0));
                        dc.DrawRectangle(null, new Pen(Brushes.Red, 1.0), new Rect(0.5, pos, ActualWidth - 1.0, height - 1.0));
                    }
                    break;
            }
            base.OnRender(dc);
        }
    }
}
