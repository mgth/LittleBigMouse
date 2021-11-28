/*
  LittleBigMouse.Daemon
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Daemon.

    LittleBigMouse.Daemon is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Daemon is distributed in the hope that it will be useful,
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
using System.Linq;
using System.Windows;

namespace LittleBigMouse.Daemon
{
    public enum Side
    {
        None,
        Top,
        Bottom,
        Left,
        Right,
    }

    public static class GeoExtensions
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
        public static Line FromSegment(Segment s)
        {
            // Line is vertical
            if (Math.Abs(s.A.X - s.B.X) < double.Epsilon)
            {
                return new Line(s.A.X);
            }

            double coef = (s.A.Y - s.B.Y) / (s.A.X - s.B.X);
            double origine = s.A.Y - coef * s.A.X;

            return new Line(coef, origine);
        }

        public double Coefficient { get; }

        private readonly double _origin;
        public double OriginY => double.IsPositiveInfinity(Coefficient) ? 0 : _origin;

        public double OriginX => (double.IsPositiveInfinity(Coefficient) || Math.Abs(Coefficient) < double.Epsilon) ? _origin : (0 - _origin)/Coefficient;

        public Point Origin => new Point(OriginX, OriginY);

        public Line(double coef, double origine)
        {
            Coefficient = coef;
            _origin = origine;
        }

        // Specific case of vertical line vhere coef is infinit
        public Line(double x)
        {
            Coefficient = double.PositiveInfinity;
            _origin = x;
        }

        public Point? Intersect(Line l)
        {
            double x;
            double y;

            if (Math.Abs(Coefficient - l.Coefficient) < double.Epsilon)
            {
                if (Math.Abs(OriginY - l.OriginY) < double.Epsilon) { return Origin; }
                return null;
            }
                

            if (double.IsPositiveInfinity(Coefficient))
            {
                if (double.IsPositiveInfinity(l.Coefficient)) { return null;}
                                
                x = OriginX;
                y = l.Coefficient * x + l.OriginY;
            }
            else
            {
                if (double.IsPositiveInfinity(l.Coefficient))
                {
                    x = l.OriginX;
                    y = Coefficient * x + OriginY;
                }
                else
                {
                    x = (OriginY - l.OriginY) / (l.Coefficient - Coefficient);
                    y = l.Coefficient * x + l.OriginY;
                }
            }

            return new Point(x, y);
        }

        public IEnumerable<Point> Intersect(Segment s)
        {
            var p = Intersect(s.Line);
            if (p!=null && s.Rect.Contains(p.Value))
            {
                yield return p.Value;
            }
        }

        public IEnumerable<Point> Intersect(Rect rect)
        {
            foreach (var p in Intersect(rect.Segment(Side.Left))) yield return p;
            foreach (var p in Intersect(rect.Segment(Side.Right))) yield return p;
            foreach (var p in Intersect(rect.Segment(Side.Top))) yield return p;
            foreach (var p in Intersect(rect.Segment(Side.Bottom))) yield return p;
        }
    }


    public class Segment
    {
        private Line _line;

        public Segment(Point a, Point b)
        {
            A = a; B = b;
        }
        public Point A { get; }
        public Point B { get; }

        public Line Line => _line ??= Line.FromSegment(this);

        public Rect Rect => new Rect(A, B);
        public double SizeSquared
        {
            get
            {
                var r = Rect;
                return r.Width * r.Width + r.Height * r.Height;
            }

        }

        public double Size => Math.Sqrt(SizeSquared);

        public Point? Intersect(Line l)
        {
            if (l == null) throw new ArgumentNullException(nameof(l));
            var p = Line.Intersect(l);
            if (p==null) return null;

            var p1 = p.Value;

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
            var p = Line.Intersect(s.Line);

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
            return sides.FirstOrDefault(side => Intersect(rect.Segment(side)) != null);
        }

        public static Side OppositeSide(Side side)
        {
            return side switch
            {
                Side.Left => Side.Right,
                Side.Right => Side.Left,
                Side.Top => Side.Bottom,
                Side.Bottom => Side.Top,
                _ => Side.None,
            };
        }

    }
}
