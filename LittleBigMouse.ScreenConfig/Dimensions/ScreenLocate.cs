/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Windows;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H = H<ScreenLocate>;

    public class ScreenLocate : ScreenSize
    {
        public ScreenLocate(IScreenSize source, Point? point = null):base(source)
        {
            H.Initialize(this);
            Location = point??new Point();
        }

        public override double Width
        {
            get => _width.Get();
            set => Source.Width = value;
        }
        private readonly IProperty<double> _width = H.Property<double>(c => c
            .Set(e => e.Source.Width)
            .On(e => e.Source.Width)
            .Update()
        );

        public override double Height
        {
            get => _height.Get();
            set => Source.Height = value;
        }
        private readonly IProperty<double> _height = H.Property<double>(c => c
            .Set(e => e.Source.Height)
            .On(e => e.Source.Height)
            .Update()
        );

        public override double X
        {
            get => _x.Get();
            set => _x.Set(value);
        }
        private readonly IProperty<double> _x = H.Property<double>();

        public override double Y
        {
            get => _y.Get();
            set => _y.Set(value);
        }
        private readonly IProperty<double> _y = H.Property<double>();

        public override double TopBorder
        {
            get => _topBorder.Get();
            set => Source.TopBorder = value;
        }
        private readonly IProperty<double> _topBorder = H.Property<double>(c => c
            .Set(e => e.Source.TopBorder)
            .On(e => e.Source.TopBorder)
            .Update()
        );

        public override double RightBorder
        {
            get => _rightBorder.Get();
            set => Source.RightBorder = value;
        }
        private readonly IProperty<double> _rightBorder = H.Property<double>(c => c
            .Set(e => e.Source.RightBorder)
            .On(e => e.Source.RightBorder)
            .Update()
        );

        public override double BottomBorder
        {
            get => _bottomBorder.Get();
            set => Source.BottomBorder = value;
        }
        private readonly IProperty<double> _bottomBorder = H.Property<double>(c => c
            .Set(e => e.Source.BottomBorder)
            .On(e => e.Source.BottomBorder)
            .Update()
        );

        public override double LeftBorder
        {
            get => _leftBorder.Get();
            set => Source.LeftBorder = value;
        }
        private readonly IProperty<double> _leftBorder = H.Property<double>(c => c
            .Set(e => e.Source.LeftBorder)
            .On(e => e.Source.LeftBorder)
            .Update()
        );
    }
}
