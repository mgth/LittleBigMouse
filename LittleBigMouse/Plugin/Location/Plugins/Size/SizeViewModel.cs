/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using Hlab.Mvvm;
using Hlab.Notify;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.LocationPlugin.Plugins.Size
{
    class ScreenSizeViewModel : ViewModel, IViewModel<Screen>
    //ScreenControlViewModel
    {
        public Screen Model => this.GetModel();

        public ScreenSizeViewModel()
        {
            var canvas = new Canvas { Effect = _effect };

            _ousideHorizontalCotation.AddTo(canvas);
            _outsideVerticalCotation.AddTo(canvas);
            _insideHorizontalCotation.AddTo(canvas);
            _insideVerticalCotation.AddTo(canvas);

            InsideCoverControl.Children.Add(canvas);

            InsideCoverControl.LayoutUpdated += OnFrameSizeChanged;

            this.Subscribe();
        }



        private readonly CotationMark _ousideHorizontalCotation = new CotationMark {Brush = new SolidColorBrush(Colors.CadetBlue)};
        private readonly CotationMark _outsideVerticalCotation = new CotationMark { Brush = new SolidColorBrush(Colors.CadetBlue) };
        private readonly CotationMark _insideHorizontalCotation = new CotationMark { Brush = new SolidColorBrush(Colors.Bisque) };
        private readonly CotationMark _insideVerticalCotation = new CotationMark { Brush = new SolidColorBrush(Colors.Bisque) };


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

        [TriggedOn(nameof(Model), "PhysicalRotated", "Height")]
        public double Height
        {
            get => Model.PhysicalRotated.Height;
            set
            {
                Model.PhysicalRotated.Height = value;
                Model.Config.Compact();
            }
        }


        [TriggedOn(nameof(Model), "PhysicalRotated", "FinalWidth")]
        public double Width
        {
            get => Model.PhysicalRotated.Width;
            set
            {
                Model.PhysicalRotated.Width = value;
                Model.Config.Compact(); 
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRotated", "TopBorder")]
        public double TopBorder
        {
            get => Model.PhysicalRotated.TopBorder;
            set
            {
                Model.PhysicalRotated.TopBorder = value;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRotated","RightBorder")]
        public double RightBorder
        {
            get => Model.PhysicalRotated.RightBorder;
            set
            {
                Model.PhysicalRotated.RightBorder = value;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRotated","BottomBorder")]
        public double BottomBorder
        {
            get => Model.PhysicalRotated.BottomBorder;
            set
            {
                Model.PhysicalRotated.BottomBorder = value;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRotated","LeftBorder")]
        public double LeftBorder
        {
            get => Model.PhysicalRotated.LeftBorder;
            set
            {
                Model.PhysicalRotated.LeftBorder = value;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRotated", "OutsideHeight")]
        public double OutsideHeight
        {
            get => Model.PhysicalRotated.OutsideHeight;
            set
            {
                var offset = value - OutsideHeight;
                Model.PhysicalRotated.BottomBorder += offset;
                Model.Config.Compact();
            }
        }

        [TriggedOn(nameof(Model), "PhysicalRotated","OutsideWidth")]
        public double OutsideWidth
        {
            get => Model.PhysicalRotated.OutsideWidth;
            set
            {
                var offset = (value - OutsideWidth) / 2;
                Model.PhysicalRotated.LeftBorder += offset;
                Model.PhysicalRotated.RightBorder += offset;
                Model.Config.Compact();
            }
        }
    }
}
