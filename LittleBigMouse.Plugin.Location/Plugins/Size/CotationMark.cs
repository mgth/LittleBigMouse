/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shapes;

namespace LittleBigMouse.Plugin.Location.Plugins.Size;

class CotationMark
{
    public void SetPoints(double length, double x1, double y1, double x2, double y2)
    {
        bool result = SetArrowLength(length);
        result |= SetStartPoint(new Point(x1, y1));
        result |= SetEndPoint(new Point(x2, y2));

        if (result) SetArrows();
    }
    public double ArrowLength
    {
        get => _arrowLength; set
        {
            if (SetArrowLength(value))
                SetArrows();
        }
    }

    private bool SetArrowLength(double length)
    {
        if (_arrowLength == length) return false;
        _arrowLength = length;
        return true;
    }

    public Point StartPoint
    {
        get => _startPoint; set
        {
            if (SetStartPoint(value))
                SetArrows();
        }
    }

    private bool SetStartPoint(Point p)
    {
        if (_startPoint == p) return false;
        _startPoint = p;
        _line.X1 = _startPoint.X;
        _line.Y1 = _startPoint.Y;
        return true;
    }

    private void SetArrows()
    {
        SetArrow(_startArrow, StartPoint, EndPoint, ArrowLength);
        SetArrow(_endArrow, EndPoint, StartPoint, ArrowLength);
    }

    private void SetArrow(Polygon p, Point start, Point end, double length = 1)
    {
        p.Points[0] = start;

        Vector v = end - start;

        v.Normalize();
        v *= length;

        Vector v1 = new Vector(v.Y / 2, -v.X / 2);
        Vector v2 = new Vector(-v.Y / 2, v.X / 2);

        p.Points[1] = start + v + v1;
        p.Points[2] = start + v + v2;
    }

    public Point EndPoint
    {
        get => _endPoint; set
        {
            SetStartPoint(value);
            SetArrows();
        }
    }

    private bool SetEndPoint(Point p)
    {
        if (_endPoint == p) return false;
        _endPoint = p;
        _line.X2 = _endPoint.X;
        _line.Y2 = _endPoint.Y;
        return true;
    }

    private Brush _brush;

    public Brush Brush
    {
        get => _brush;
        set
        {
            _brush = value;
            _line.Stroke = _brush;
            _startArrow.Fill = _brush;
            _endArrow.Fill = _brush;
        }
    }

    public void AddTo(Canvas c)
    {
        c.Children.Add(_line);
        c.Children.Add(_startArrow);
        c.Children.Add(_endArrow);
    }

    private readonly Line _line = new Line { StrokeThickness = 2 };
    private readonly Polygon _startArrow = new Polygon { Points = { new Point(), new Point(), new Point(), } };
    private readonly Polygon _endArrow = new Polygon { Points = { new Point(), new Point(), new Point(), } };

    private Point _startPoint;
    private Point _endPoint;
    private double _arrowLength;
}
