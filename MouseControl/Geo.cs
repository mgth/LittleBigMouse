using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace MouseControl
{
    public class Line
    {
        public static Line fromSegment(Segment s)
        {
            if (s.A.X == s.B.X)
            {
                return new Line(s.A.X);
            }

            double coef = (s.A.Y - s.B.Y) / (s.A.X - s.B.X);
            double origine = s.A.Y - coef * s.A.X;

            return new Line(coef, origine);
        }

        private double _coef;
        public double Coef { get { return _coef; } }

        private double _origine;
        public double OrigineY
        {
            get
            {
                if (double.IsNaN(_coef)) return double.NaN;
                return _origine;
            }
        }
        public double OrigineX
        {
            get
            {
                if (double.IsNaN(_coef)) return _origine;
                return (0 - _origine) / _coef;
            }
        }


        public Line(double coef, double origine)
        {
            _coef = coef;
            _origine = origine;
        }

        public Line(double X)
        {
            _coef = double.NaN;
            _origine = X;
        }

        public Point? Intersect(Line l)
        {
            double X;
            double Y;

            if (Coef == l.Coef) return null;

            if (double.IsNaN(Coef))
            {
                if (double.IsNaN(l.Coef))
                {
                    return null;
                }
                else
                {
                    X = OrigineX;
                    Y = l.Coef * X + l.OrigineY;
                }
            }
            else
            {
                if (double.IsNaN(l.Coef))
                {
                    X = l.OrigineX;
                    Y = Coef * X + OrigineY;
                }
                else
                {
                    X = (OrigineY - l.OrigineY) / (l.Coef - Coef);
                    Y = l.Coef * X + l.OrigineY;
                }
            }

            return new Point(X, Y);
        }

    }


    public class Segment
    {
        Point _a;
        Point _b;

        Line _line;
        //        Rect? _rect = null;
        public Segment(Point a, Point b)
        {
            _a = a; _b = b;
        }
        public Point A { get { return _a; } }
        public Point B { get { return _b; } }

        public Line Line
        {
            get
            {
                if (_line == null) _line = Line.fromSegment(this);
                return _line;
            }
        }
        public Rect Rect
        {
            get
            {
                return new Rect(A, B);
            }
        }

        public double Size
        {
            get
            {
                Rect r = Rect;
                return Math.Sqrt(r.Width * r.Width + r.Height * r.Height);
            }

        }

        public Point? Intersect(Segment s)
        {
            Point? p = Line.Intersect(s.Line);

            if (p!=null && Rect.Contains(p??new Point()) && s.Rect.Contains(p??new Point())) return p;

            return null;
        }

        public Point? Intersect(Rect r)
        {
            Point? p = null;
            
            p = Intersect(new Segment(r.TopLeft, r.BottomLeft));
            if (p==null)
                p = Intersect(new Segment(r.TopRight, r.BottomRight));

            if (p == null)
                p = Intersect(new Segment(r.TopLeft, r.TopRight));

            if (p == null)
                p = Intersect(new Segment(r.BottomLeft, r.BottomRight));

            return p;
            
        }


    }
}
