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
    internal class ScreenSizeViewModel : ViewModel<PhysicalMonitor>
    //ScreenControlViewModel
    {
        public ScreenSizeViewModel()
        {
            var canvas = new Canvas
            {
                Children =
                {
                    _outsideHorizontalMeasureArrow,
                    _outsideVerticalMeasureArrow,
                    _insideHorizontalMeasureArrow,
                    _insideVerticalMeasureArrow
                }
            };

            InsideCoverControl.Children.Add(canvas);

            InsideCoverControl.LayoutUpdated += OnFrameSizeChanged;

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

        readonly MeasureArrow _outsideHorizontalMeasureArrow = new()
        {
            StrokeThickness = 2, 
            Fill = new SolidColorBrush(Colors.CadetBlue)
        };

        readonly MeasureArrow _outsideVerticalMeasureArrow = new()
        {
            StrokeThickness = 2, 
            Fill = new SolidColorBrush(Colors.CadetBlue)
        };

        readonly MeasureArrow _insideHorizontalMeasureArrow = new()
        {
            StrokeThickness = 2, 
            Fill = new SolidColorBrush(Colors.Bisque)
        };

        readonly MeasureArrow _insideVerticalMeasureArrow = new()
        {
            StrokeThickness = 2, 
            Fill = new SolidColorBrush(Colors.Bisque)
        };

        void OnFrameSizeChanged(object sender, EventArgs eventArgs)
        {
            DrawLines();
        }

        public Grid InsideCoverControl { get; } = new Grid();

        readonly Effect _effect = new DropShadowEffect
        {
            Color = Colors.DarkBlue,
        };


        public void DrawLines()
        {
            var rx = InsideCoverControl.Bounds.Width / Model.DepthProjection.Width;//this.FindParent<>.Presenter.GetRatio();
            var ry = InsideCoverControl.Bounds.Height / Model.DepthProjection.Height;//this.FindParent<>.Presenter.GetRatio();

            var h = InsideCoverControl.Bounds.Height;
            var w = InsideCoverControl.Bounds.Width;
            var x = 5 * InsideCoverControl.Bounds.Width / 8;// + ScreenGui.LeftBorder.Value;
            var y = 5 * InsideCoverControl.Bounds.Height / 8;// + ScreenGui.TopBorder.Value;

            var x2 = -rx * Model.DepthProjection.LeftBorder;
            var y2 = -ry * Model.DepthProjection.TopBorder;

            var h2 = h - y2 + ry * Model.DepthProjection.BottomBorder;
            var w2 = w - x2 + rx * Model.DepthProjection.RightBorder;

            var length = rx * (Model.DepthProjection.BottomBorder + Model.DepthProjection.RightBorder + Model.DepthProjection.LeftBorder + Model.DepthProjection.TopBorder) / 8;


            static void SetArrow(MeasureArrow arrow, double l, Point s, Point e)
            {
                arrow.ArrowLength = l;
                arrow.StartPoint = s;
                arrow.EndPoint = e;
            }

            SetArrow(
                _insideVerticalMeasureArrow,
                length, 
                new Point(x,0),
                new Point(x,h)
                );

            SetArrow(
                _insideHorizontalMeasureArrow,
                length, 
                new Point(0, y),
                new Point(w, y)
                );

            SetArrow(
                _outsideVerticalMeasureArrow,
                length, 
                new Point(x + w / 8 - w / 128, y2), 
                new Point(x + w / 8 - w / 128, y2 + h2)
                );

            SetArrow(
                _outsideHorizontalMeasureArrow,
                length, 
                new Point(x2, y + h / 8 - h / 128), 
                new Point(x2 + w2, y + h / 8 - h / 128)
                );
        }

        public double Height
        {
            get => _height.Value;
            set
            {
                Model.PhysicalRotated.Height = value;
                // TODO : layout not reachable :
                // Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _height;


        public double Width
        {
            get => _width.Value;
            set
            {
                Model.PhysicalRotated.Width = value;
                // TODO : layout not reachable :
                // Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _width;


        public double TopBorder
        {
            get => _topBorder.Value;
            set
            {
                Model.PhysicalRotated.TopBorder = value;
                // TODO : layout not reachable :
                // Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _topBorder;


        public double RightBorder
        {
            get => _rightBorder.Value;
            set
            {
                Model.PhysicalRotated.RightBorder = value;
                // TODO : layout not reachable :
                // Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _rightBorder;


        public double BottomBorder
        {
            get => _bottomBorder.Value;
            set
            {
                Model.PhysicalRotated.BottomBorder = value;
                // TODO : layout not reachable :
                // Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _bottomBorder;


        public double LeftBorder
        {
            get => _leftBorder.Value;
            set
            {
                Model.PhysicalRotated.LeftBorder = value;
                // TODO : layout not reachable :
                // Model.Layout.Compact();
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
                // TODO : layout not reachable :
                // Model.Layout.Compact();
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
                // TODO : layout not reachable :
                // Model.Layout.Compact();
            }
        }
        readonly ObservableAsPropertyHelper<double> _outsideWidth;

    }
}
