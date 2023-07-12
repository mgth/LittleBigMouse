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

using Avalonia;
using Avalonia.Controls.Shapes;
using Avalonia.Media;

namespace LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin;

internal class MeasureArrow : Line
{
    public static readonly StyledProperty<double> ArrowLengthProperty =
        AvaloniaProperty.Register<MeasureArrow, double>("ArrowLength");

    static MeasureArrow()
    {
        AffectsGeometry<MeasureArrow>(ArrowLengthProperty);
    }

    public double ArrowLength
    {
        get => GetValue(ArrowLengthProperty);
        set => SetValue(ArrowLengthProperty, value);
    }

    static Vector Multiply(Vector v,Matrix m)
    {
        var p = new Point(v.X, v.Y);
        return p * m;
    }

    static (Point,PolylineGeometry) GetArrow(Point start, Point end, double length = 1)
    {
        Vector v = end - start;

        v = v.Normalize();
        v *= length;

        var v1 = Multiply(v,Matrix.CreateRotation(0.35));
        var v2 = Multiply(v,Matrix.CreateRotation(-0.35));

        var p = start + (v1 + v2)/2;

        return (p, new PolylineGeometry(new[] { start, start + v1, start + v2 }, true));
    }

    protected override Geometry CreateDefiningGeometry()
    {
        var (start,startArrow) = GetArrow(StartPoint, EndPoint, ArrowLength);
        var (end,endArrow) = GetArrow(EndPoint, StartPoint, ArrowLength);

        var line = new LineGeometry(start, end);

        return new GeometryGroup{Children = new GeometryCollection(new Geometry[]{startArrow,endArrow,line})};
    }
}
