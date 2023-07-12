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
using Avalonia.Controls;
using Avalonia.Media;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin
{
    public struct Arrow
    {
        public Arrow(Point start, Point end)
        {
            StartPoint = start;
            EndPoint = end;
        }
        public Point StartPoint { get; }
        public Point EndPoint { get; }
    }

    internal class ScreenSizeViewModel : ViewModel<PhysicalMonitor>
    {
        public ScreenSizeViewModel()
        {


            _height = this.WhenAnyValue(e => e.Model.PhysicalRotated.Height)
                .ToProperty(this, e => e.Height);

            _width = this.WhenAnyValue(e => e.Model.PhysicalRotated.Width)
                .ToProperty(this, e => e.Width);

            _topBorder = this.WhenAnyValue(e => e.Model.PhysicalRotated.TopBorder)
                .ToProperty(this, e => e.TopBorder);

            _rightBorder = this.WhenAnyValue(e => e.Model.PhysicalRotated.RightBorder)
                .ToProperty(this, e => e.RightBorder);

            _bottomBorder = this.WhenAnyValue(e => e.Model.PhysicalRotated.BottomBorder)
                .ToProperty(this, e => e.BottomBorder);

            _leftBorder = this.WhenAnyValue(e => e.Model.PhysicalRotated.LeftBorder)
                .ToProperty(this, e => e.LeftBorder);

            _outsideHeight = this.WhenAnyValue(e => e.Model.PhysicalRotated.OutsideHeight)
                .ToProperty(this, e => e.OutsideHeight);

            _outsideWidth = this.WhenAnyValue(e => e.Model.PhysicalRotated.OutsideWidth)
                .ToProperty(this, e => e.OutsideWidth);
        }

        public double ArrowLength
        {
            get => _arrowLength;
            set => this.RaiseAndSetIfChanged(ref _arrowLength, value);
        }
        double _arrowLength;

        //Inside Vertical
        public Arrow InsideVerticalArrow
        {
            get => _insideVerticalArrow;
            set => this.RaiseAndSetIfChanged(ref _insideVerticalArrow, value);
        }
        Arrow _insideVerticalArrow;

        //Inside Horizontal
        public Arrow InsideHorizontalArrow
        {
            get => _insideHorizontalArrow;
            set => this.RaiseAndSetIfChanged(ref _insideHorizontalArrow, value);
        }
        Arrow _insideHorizontalArrow;

        //Outside Vertical
        public Arrow OutsideVerticalArrow
        {
            get => _outsideVerticalArrow;
            set => this.RaiseAndSetIfChanged(ref _outsideVerticalArrow, value);
        }
        Arrow _outsideVerticalArrow;

        //Outside Horizontal
        public Arrow OutsideHorizontalArrow
        {
            get => _outsideHorizontalArrow;
            set => this.RaiseAndSetIfChanged(ref _outsideHorizontalArrow, value);
        }
        Arrow _outsideHorizontalArrow;


        public void UpdateArrows(Rect bounds)
        {
            var rx = bounds.Width / Model.DepthProjection.Width;
            var ry = bounds.Height / Model.DepthProjection.Height;

            var h = bounds.Height;
            var w = bounds.Width;
            var x = 5 * bounds.Width / 8;
            var y = 5 * bounds.Height / 8;

            var x2 = -rx * Model.DepthProjection.LeftBorder;
            var y2 = -ry * Model.DepthProjection.TopBorder;

            var h2 = h - y2 + ry * Model.DepthProjection.BottomBorder;
            var w2 = w - x2 + rx * Model.DepthProjection.RightBorder;

            ArrowLength = rx * 
                Math.Min(Model.DepthProjection.Width, Model.DepthProjection.Height) / 32;

            InsideVerticalArrow = new Arrow(new Point(x, 0), new Point(x,h));
            InsideHorizontalArrow = new Arrow(new Point(0, y), new Point(w, y));
            OutsideVerticalArrow = new Arrow(new Point(x + w / 8 - w / 128, y2), new Point(x + w / 8 - w / 128, y2 + h2));
            OutsideHorizontalArrow = new Arrow(new Point(x2, y + h / 8 - h / 128), new Point(x2 + w2, y + h / 8 - h / 128));
        }

        public double Height
        {
            get => _height.Value;
            set
            {
                Model.PhysicalRotated.Height = value;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _height;


        public double Width
        {
            get => _width.Value;
            set
            {
                Model.PhysicalRotated.Width = value;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _width;


        public double TopBorder
        {
            get => _topBorder.Value;
            set
            {
                Model.PhysicalRotated.TopBorder = value;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _topBorder;


        public double RightBorder
        {
            get => _rightBorder.Value;
            set
            {
                Model.PhysicalRotated.RightBorder = value;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _rightBorder;


        public double BottomBorder
        {
            get => _bottomBorder.Value;
            set
            {
                Model.PhysicalRotated.BottomBorder = value;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _bottomBorder;


        public double LeftBorder
        {
            get => _leftBorder.Value;
            set
            {
                Model.PhysicalRotated.LeftBorder = value;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _leftBorder;


        public double OutsideHeight
        {
            get => _outsideHeight.Value;
            set
            {
                var offset = value - OutsideHeight;
                Model.PhysicalRotated.BottomBorder += offset;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _outsideHeight;


        public double OutsideWidth
        {
            get => _outsideWidth.Value;
            set
            {
                var offset = (value - OutsideWidth) / 2;
                Model.PhysicalRotated.LeftBorder += offset;
                Model.PhysicalRotated.RightBorder += offset;
                Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _outsideWidth;

    }
}
