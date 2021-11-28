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
    using H = H<ScreenRotate>;

    public class ScreenRotate : ScreenSize
    {
        public int Rotation { get; }
        public ScreenRotate(IScreenSize source, int rotation = 0) : base(source)
        {
            Rotation = rotation;
            H.Initialize(this);
        }

        public Vector Translation
        {
            get => _translation.Get();
            set => _translation.Set(value);
        }
        private readonly IProperty<Vector> _translation = H.Property<Vector>();

        public override double Width
        {
            get => _width.Get();
            set
            {
                switch (Rotation % 2)
                {
                    case 0:
                        Source.Width = value;
                        break;
                    case 1:
                        Source.Height = value;
                        break;
                }
            }
        }
        private readonly IProperty<double> _width = H.Property<double>(c => c
            .Set(e => e.Rotation % 2 == 0 ? e.Source.Width : e.Source.Height)
            .On(e => e.Source.Width)
            .On(e => e.Source.Height)
            .On(e => e.Rotation)
            .Update()
        );

        public override double Height
        {
            get => _height.Get();
            set
            {
                switch (Rotation % 2)
                {
                    case 0: Source.Height = value;
                        break;
                    case 1: Source.Width = value;
                        break;
                }
            }
        }
        private readonly IProperty<double> _height = H.Property<double>(c => c
            .Set(e => e.Rotation % 2 == 1 ? e.Source.Width : e.Source.Height)
            .On(e => e.Source.Width)
            .On(e => e.Source.Height)
            .On(e => e.Rotation)
            .Update()
        );

        public override double X
        {
            get => _x.Get();
            set => Source.X = value;
        }
        private readonly IProperty<double> _x = H.Property<double>(c => c
            .Set(e => e.Source.X)
            .On(e => e.Source.X)
            .Update()
        );

        public override double Y
        {
            get => _y.Get();
            set => Source.Y = value;
        }
        private readonly IProperty<double> _y = H.Property<double>(c => c
            .Set(e => e.Source.X)
            .On(e => e.Source.X)
            .Update()
        );

        private double GetBorder(int border)
        {
            return ((border + Rotation) % 4) switch
            {
                0 => Source.TopBorder,
                1 => Source.RightBorder,
                2 => Source.BottomBorder,
                3 => Source.LeftBorder,
                _ => -1,
            };
        }
        private void SetBorder(int border, double value)
        {
            switch ((border + Rotation) % 4)
            {
                case 0:
                    Source.TopBorder = value;
                    break;
                case 1:
                    Source.RightBorder = value;
                    break;
                case 2:
                    Source.BottomBorder = value;
                    break;
                case 3:
                    Source.LeftBorder = value;
                    break;
            }
        }


        public override double TopBorder
        {
            get => _topBorder.Get();
            set => SetBorder(0,value);
        }
        private readonly IProperty<double> _topBorder = H.Property<double>(c => c
            .Set(e => e.GetBorder(0))
            .On(e => e.Rotation)
            .On(e => e.Source.TopBorder)
            .On(e => e.Source.RightBorder)
            .On(e => e.Source.BottomBorder)
            .On(e => e.Source.LeftBorder)
            .Update()
        );

        public override double RightBorder
        {
            get => _rightBorder.Get();
            set => SetBorder(1,value);
        }
        private readonly IProperty<double> _rightBorder = H.Property<double>(c => c
            .Set(e => e.GetBorder(1))
            .On(e => e.Rotation)
            .On(e => e.Source.TopBorder)
            .On(e => e.Source.RightBorder)
            .On(e => e.Source.BottomBorder)
            .On(e => e.Source.LeftBorder)
            .Update()
        );

        public override double BottomBorder
        {
            get => _bottomBorder.Get();
            set => SetBorder(2,value);
        }
        private readonly IProperty<double> _bottomBorder = H.Property<double>(c => c
            .Set(e => e.GetBorder(2))
            .On(e => e.Rotation)
            .On(e => e.Source.TopBorder)
            .On(e => e.Source.RightBorder)
            .On(e => e.Source.BottomBorder)
            .On(e => e.Source.LeftBorder)
            .Update()
        );

        public override double LeftBorder
        {
            get => _leftBorder.Get();
            set => SetBorder(3,value);
        }
        private readonly IProperty<double> _leftBorder = H.Property<double>(c => c
            .Set(e => e.GetBorder(3))
            .On(e => e.Rotation)
            .On(e => e.Source.TopBorder)
            .On(e => e.Source.RightBorder)
            .On(e => e.Source.BottomBorder)
            .On(e => e.Source.LeftBorder)
            .Update()
        );
    }
}