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

using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using System.Windows;

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenRotate : ScreenSize<ScreenRotate>
    {
        public int Rotation { get; }
        public ScreenRotate(IScreenSize source, int rotation = 0) : base(source)
        {
            Rotation = rotation;
            Initialize();
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
            .On(e => e.Source.Width)
            .On(e => e.Source.Height)
            .On(e => e.Rotation)
            .NotNull(e => e.Source)
            .Set(e => e.Rotation % 2 == 0 ? e.Source.Width : e.Source.Height)
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
            .On(e => e.Source.Width)
            .On(e => e.Source.Height)
            .On(e => e.Rotation)
            .NotNull(e => e.Source)
            .Set(e => e.Rotation % 2 == 1 ? e.Source.Width : e.Source.Height)
        );

        [TriggerOn(nameof(Source),"X")]
        public override double X
        {
            get => Source.X;
            set => Source.X = value;
        }

        [TriggerOn(nameof(Source),"Y")]
        public override double Y
        {
            get => Source.Y;
            set => Source.Y = value;
        }

        private double GetBorder(int border)
        {
            switch ((border+Rotation)%4)
            {
                case 0: return Source.TopBorder;
                case 1: return Source.RightBorder;
                case 2: return Source.BottomBorder;
                case 3: return Source.LeftBorder;
            }
            return -1;
            
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


        [TriggerOn(nameof(Source), "TopBorder")]
        [TriggerOn(nameof(Source), "RightBorder")]
        [TriggerOn(nameof(Source), "BottomBorder")]
        [TriggerOn(nameof(Source), "LeftBorder")]
        [TriggerOn(nameof(Rotation))]
        public override double TopBorder
        {
            get => GetBorder(0);
            set => SetBorder(0,value);
        }

        [TriggerOn(nameof(Source), "TopBorder")]
        [TriggerOn(nameof(Source), "RightBorder")]
        [TriggerOn(nameof(Source), "BottomBorder")]
        [TriggerOn(nameof(Source), "LeftBorder")]
        [TriggerOn(nameof(Rotation))]
        public override double RightBorder
        {
            get => GetBorder(1);
            set => SetBorder(1,value);
        }

        [TriggerOn(nameof(Source), "TopBorder")]
        [TriggerOn(nameof(Source), "RightBorder")]
        [TriggerOn(nameof(Source), "BottomBorder")]
        [TriggerOn(nameof(Source), "LeftBorder")]
        [TriggerOn(nameof(Rotation))]
        public override double BottomBorder
        {
            get => GetBorder(2);
            set => SetBorder(2,value);
        }

        [TriggerOn(nameof(Source), "TopBorder")]
        [TriggerOn(nameof(Source), "RightBorder")]
        [TriggerOn(nameof(Source), "BottomBorder")]
        [TriggerOn(nameof(Source), "LeftBorder")]
        [TriggerOn(nameof(Rotation))]
        public override double LeftBorder
        {
            get => GetBorder(3);
            set => SetBorder(3,value);
        }
    }
}