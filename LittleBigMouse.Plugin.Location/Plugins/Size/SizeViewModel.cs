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

using System;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Effects;

using HLab.Mvvm;
using HLab.Notify.PropertyChanged;

using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Plugin.Location.Plugins.Size
{
    using H = H<ScreenSizeViewModel>;

    class ScreenSizeViewModel : ViewModel<Monitor>
    //ScreenControlViewModel
    {
        public ScreenSizeViewModel()
        {

            var canvas = new Canvas { Effect = _effect };

            _ousideHorizontalCotation.AddTo(canvas);
            _outsideVerticalCotation.AddTo(canvas);
            _insideHorizontalCotation.AddTo(canvas);
            _insideVerticalCotation.AddTo(canvas);

            InsideCoverControl.Children.Add(canvas);

            InsideCoverControl.LayoutUpdated += OnFrameSizeChanged;

            H.Initialize(this);
        }



        private readonly CotationMark _ousideHorizontalCotation = new() { Brush = new SolidColorBrush(Colors.CadetBlue)};
        private readonly CotationMark _outsideVerticalCotation = new() { Brush = new SolidColorBrush(Colors.CadetBlue) };
        private readonly CotationMark _insideHorizontalCotation = new() { Brush = new SolidColorBrush(Colors.Bisque) };
        private readonly CotationMark _insideVerticalCotation = new() { Brush = new SolidColorBrush(Colors.Bisque) };


        private void OnFrameSizeChanged(object sender, EventArgs eventArgs)
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
            var rx = InsideCoverControl.ActualWidth / Model.InMm.Width;//this.FindParent<>.Presenter.GetRatio();
            var ry = InsideCoverControl.ActualHeight / Model.InMm.Height;//this.FindParent<>.Presenter.GetRatio();

            var h = InsideCoverControl.ActualHeight;
            var w = InsideCoverControl.ActualWidth;
            var x = 5 * InsideCoverControl.ActualWidth / 8;// + ScreenGui.LeftBorder.Value;
            var y = 5 * InsideCoverControl.ActualHeight / 8;// + ScreenGui.TopBorder.Value;

            var x2 = - rx * Model.InMm.LeftBorder;
            var y2 = - ry * Model.InMm.TopBorder;

            var h2 = h - y2 + ry * Model.InMm.BottomBorder;
            var w2 = w - x2 + rx * Model.InMm.RightBorder;

            var arrow = rx*(Model.InMm.BottomBorder + Model.InMm.RightBorder + Model.InMm.LeftBorder + Model.InMm.TopBorder)/8;

            _insideVerticalCotation.SetPoints(arrow, x, 0,x,h);
            _insideHorizontalCotation.SetPoints (arrow, 0, y, w, y);

            _outsideVerticalCotation.SetPoints(arrow, x + (w/8)-(w/128),y2,x + (w/8)-(w/128),y2+h2);
            _ousideHorizontalCotation.SetPoints(arrow, x2, y + (h/8)-(h/128),x2+w2,y + (h/8)-(h/128));
        }

        public double Height
        {
            get => _height.Get();
            set
            {
                Model.PhysicalRotated.Height = value;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _height = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.Height)
            .On(e => e.Model.PhysicalRotated.Height)
            .Update());


        public double Width
        {
            get => _width.Get();
            set
            {
                Model.PhysicalRotated.Width = value;
                Model.Layout.Compact(); 
            }
        }
        private readonly IProperty<double> _width = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.Width)
            .On(e => e.Model.PhysicalRotated.Width)
            .Update());

        public double TopBorder
        {
            get => _topBorder.Get();
            set
            {
                Model.PhysicalRotated.TopBorder = value;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _topBorder = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.TopBorder)
            .On(e => e.Model.PhysicalRotated.TopBorder)
            .Update());

        public double RightBorder
        {
            get => _rightBorder.Get();
            set
            {
                Model.PhysicalRotated.RightBorder = value;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _rightBorder = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.RightBorder)
            .On(e => e.Model.PhysicalRotated.RightBorder)
            .Update());

        public double BottomBorder
        {
            get => _bottomBorder.Get();
            set
            {
                Model.PhysicalRotated.BottomBorder = value;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _bottomBorder = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.BottomBorder)
            .On(e => e.Model.PhysicalRotated.BottomBorder)
            .Update());

        public double LeftBorder
        {
            get => _leftBorder.Get();
            set
            {
                Model.PhysicalRotated.LeftBorder = value;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _leftBorder = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.LeftBorder)
            .On(e => e.Model.PhysicalRotated.LeftBorder)
            .Update());

        public double OutsideHeight
        {
            get => _outsideHeight.Get();
            set
            {
                var offset = value - OutsideHeight;
                Model.PhysicalRotated.BottomBorder += offset;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _outsideHeight = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.OutsideHeight)
            .On(e => e.Model.PhysicalRotated.OutsideHeight)
            .Update());

        public double OutsideWidth
        {
            get => _outsideWidth.Get();
            set
            {
                var offset = (value - OutsideWidth) / 2;
                Model.PhysicalRotated.LeftBorder += offset;
                Model.PhysicalRotated.RightBorder += offset;
                Model.Layout.Compact();
            }
        }
        private readonly IProperty<double> _outsideWidth = H.Property<double>(c => c
            .Set(e => e.Model.PhysicalRotated.OutsideWidth)
            .On(e => e.Model.PhysicalRotated.OutsideWidth)
            .Update());
    }
}
