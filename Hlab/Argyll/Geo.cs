/*
  Hlab.Argyll
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of Hlab.Argyll.

    Hlab.Argyll is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    Hlab.Argyll is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections.Generic;
using System.Windows;

namespace Hlab.Argyll
{
    public enum Side
    {
        None,
        Top,
        Bottom,
        Left,
        Right,
    }

    public static class GeoExtentions
    {
        public static Segment Segment(this Rect rect, Side side)
        {
            switch (side)
            {
                case Side.Left:
                    return new Segment(rect.TopLeft, rect.BottomLeft);
                case Side.Right:
                    return new Segment(rect.TopRight, rect.BottomRight);
                case Side.Top:
                    return new Segment(rect.TopLeft, rect.TopRight);
                case Side.Bottom:
                    return new Segment(rect.BottomLeft, rect.BottomRight);
                case Side.None:
                default:
                    return null;
            }
        }
    }

    public class Line
    {
        public static Line fromSegment(Segment s)
        {
            // Line is vertical
            if (s.A.X == s.B.X)
            {
                return new Line(s.A.X);
            }

            double coef = (s.A.Y - s.B.Y) / (s.A.X - s.B.X);
            double origine = s.A.Y - coef * s.A.X;

            return new Line(coef, origine);
        }

        public double Coef { get; }

        private readonly double _origine;
        public double OrigineY => double.IsPositiveInfinity(Coef) ? 0 : _origine;

        public double OrigineX => double.IsPositiveInfinity(Coef) ? _origine : (0 - _origine)/Coef;

        public Point Origine => new Point(OrigineX, OrigineY);

        public Line(double coef, double origine)
        {
            Coef = coef;
            _origine = origine;
        }

        // Specific case of vertical line vhere coef is infinit
        public Line(double x)
        {
            Coef = double.PositiveInfinity;
            _origine = x;
        }

        public Point? Intersect(Line l)
        {
            double x;
            double y;

            if (Coef == l.Coef)
            {
                if (OrigineY == l.OrigineY) { return Origine; }
                return null;
            }
                

            if (double.IsPositiveInfinity(Coef))
            {
                if (double.IsPositiveInfinity(l.Coef)) { return null;}
                                
                x = OrigineX;
                y = l.Coef * x + l.OrigineY;
            }
            else
            {
                if (double.IsPositiveInfinity(l.Coef))
                {
                    x = l.OrigineX;
                    y = Coef * x + OrigineY;
                }
                else
                {
                    x = (OrigineY - l.OrigineY) / (l.Coef - Coef);
                    y = l.Coef * x + l.OrigineY;
                }
            }

            return new Point(x, y);
        }

        public IEnumerable<Point> Intersect(Segment s)
        {
            Point? p = Intersect(s.Line);
            if (p!=null && s.Rect.Contains(p.Value))
            {
                yield return p.Value;
            }
        }

        public IEnumerable<Point> Intersect(Rect rect)
        {
            foreach (Point p in Intersect(rect.Segment(Side.Left))) yield return p;
            foreach (Point p in Intersect(rect.Segment(Side.Right))) yield return p;
            foreach (Point p in Intersect(rect.Segment(Side.Top))) yield return p;
            foreach (Point p in Intersect(rect.Segment(Side.Bottom))) yield return p;
        }
    }

    public class Triangle
    {
        public Point[] Points { get; } = new Point[3];

        public Point A => Points[0];
        public Point B => Points[1];
        public Point C => Points[2];

        public Segment AB => new Segment(A,B);
        public Segment BC => new Segment(B,C);
        public Segment CA => new Segment(C,A);
  

        public Triangle(Point a, Point b, Point c)
        {
            Points[0] = a;
            Points[1] = b;
            Points[2] = c;
        }

        public Point Gravity => new Point((A.X + B.X + C.X)/3, (A.Y + B.Y + C.Y)/3);

        public Point Inside( Point pCenter, Point pOut)
        {
            Segment s = new Segment(pCenter,pOut);

            Point? pIn = s.Intersect(AB.Line);
            if (pIn != null)
            {
                pOut = pIn.Value;
                s = new Segment(pCenter, pOut);
            }

            pIn = s.Intersect(BC.Line);
            if (pIn != null)
            {
                pOut = pIn.Value;
                s = new Segment(pCenter, pOut);
            }

            pIn = s.Intersect(CA);
            if (pIn != null) 
            {
                pOut = pIn.Value;
                s = new Segment(pCenter, pOut);
            }

            return pOut;
        }
    }

    public class Segment
    {
        Line _line;

        public Segment(Point a, Point b)
        {
            A = a; B = b;
        }
        public Point A { get; }
        public Point B { get; }

        public Line Line => _line ?? (_line = Line.fromSegment(this));

        public Rect Rect => new Rect(A, B);

        public double Size
        {
            get
            {
                Rect r = Rect;
                return Math.Sqrt(r.Width * r.Width + r.Height * r.Height);
            }

        }

        public Point? Intersect(Line l)
        {
            if (l == null) throw new ArgumentNullException(nameof(l));
            Point? p = Line.Intersect(l);
            if (p==null) return null;

            Point p1 = p.Value;

            if (p1.X < Rect.X - Epsilon) return null;
            if (p1.Y < Rect.Y - Epsilon) return null;
            if (p1.X > Rect.Right + Epsilon) return null;
            if (p1.Y > Rect.Bottom + Epsilon) return null;
            //        && Rect.Contains(p.Value)) return p;
            return p1;
        }


        private const double Epsilon = 0.001; 
        public Point? Intersect(Segment s)
        {
            Point? p = Line.Intersect(s.Line);

            if (p == null) return null;

            if (p.Value.X < Rect.X - Epsilon) return null;
            if (p.Value.Y < Rect.Y - Epsilon) return null;
            if (p.Value.X > Rect.Right + Epsilon) return null;
            if (p.Value.Y > Rect.Bottom + Epsilon) return null;

            return p;
        }

        public IEnumerable<Point> Intersect(Rect r)
        {
            Point? p = null;
            
            p = Intersect(new Segment(r.TopLeft, r.BottomLeft));
            if (p != null) yield return p.Value;

            p = Intersect(new Segment(r.TopRight, r.BottomRight));
            if (p != null) yield return p.Value;

            p = Intersect(new Segment(r.TopLeft, r.TopRight));
            if (p != null) yield return p.Value;

            p = Intersect(new Segment(r.BottomLeft, r.BottomRight));
            if (p != null) yield return p.Value;
        }

        public Side IntersectSide(Rect rect)
        {
            Side[] sides = {Side.Left, Side.Right, Side.Top, Side.Bottom};

            //return sides.Where(s => Intersect(r.Segment(s)) != null).FirstOrDefault();
            foreach (var side in sides)
            {
                if (Intersect(rect.Segment(side)) != null) return side;
            }
            return Side.None;
        }

        public static Side OpositeSide(Side side)
        {
            switch (side)
            {
                case Side.Left:
                    return Side.Right;
                case Side.Right:
                    return Side.Left;
                case Side.Top:
                    return Side.Bottom;
                case Side.Bottom:
                    return Side.Top;
                default:
                    return Side.None;
            }
        }

    }
}
