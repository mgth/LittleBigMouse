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

using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H = H<ScreenScaleDip>;

    public class ScreenScaleDip : ScreenSize
    {
        public IScreenRatio EffectiveDpi { get; }
        public ScreenConfig Config { get; }

        public ScreenScaleDip(IScreenSize source, IScreenRatio effectiveDpi, ScreenConfig config):base(source)
        {
            EffectiveDpi = effectiveDpi;
            Config = config;
            H.Initialize(this);
        }

        public IScreenRatio Ratio => _ratio.Get();

        private readonly IProperty<IScreenRatio> _ratio 
            = H.Property<IScreenRatio>( c => c
            .Set(e => (IScreenRatio)new ScreenRatioValue(
                 96 / e.EffectiveDpi.X,
                96 / e.EffectiveDpi.Y) )
            .On(e => e.EffectiveDpi.X)
            .On(e => e.EffectiveDpi.Y)
            .Update()
           );

        public IScreenRatio MainRatio => _mainRatio.Get();
        private readonly IProperty<IScreenRatio> _mainRatio 
            = H.Property<IScreenRatio>(c => c
            .Set(e => (IScreenRatio)new ScreenRatioValue(
                96 / (e.Config.PrimaryScreen?.EffectiveDpi.X??96),
                96 / (e.Config.PrimaryScreen?.EffectiveDpi.Y??96)))
            .On( e => e.Config.PrimaryScreen.EffectiveDpi.X)
            .On( e => e.Config.PrimaryScreen.EffectiveDpi.Y)
            .Update()
            );

        public override double Width
        {
            get => _width.Get();
            set => Source.Width = value / Ratio.X;
        }
        private readonly IProperty<double> _width = H.Property<double>(c => c
                .Set(e => e.Source.Width * e.Ratio.X)
                .On(e => e.Source.Width)
                .On(e => e.Ratio.X)
                .Update()
            );

        public override double Height
        {
            get => _height.Get();
            set => Source.Height = value / Ratio.Y;
        }
        private readonly IProperty<double> _height
            = H.Property<double>(c => c
                .Set(e => e.Source.Height * e.Ratio.Y)
                .On(e => e.Source.Height)
                .On(e => e.Ratio.Y)
                .Update()
            );

        public override double X
        {
            get => _x.Get();
            set => Source.X = value / MainRatio.X;
        }
        private readonly IProperty<double> _x = H.Property<double>(c => c
                .Set(e => e.Source.X * e.MainRatio.X)
                .On(e => e.Source.X)
                .On(e => e.MainRatio.X)
                .Update()
            );

        public override double Y
        {
            get => _y.Get();
            set => Source.Y = value / MainRatio.Y;
        }
        private readonly IProperty<double> _y = H.Property<double>(c => c
                .Set(e => e.Source.Y * e.MainRatio.Y)
                .On(e => e.Source.Y)
                .On(e => e.MainRatio.Y)
                .Update()
            );

        public override double TopBorder
        {
            get => _topBorder.Get();
            set => Source.TopBorder = value / Ratio.Y;
        }
        private readonly IProperty<double> _topBorder = H.Property<double>(c => c
                .Set(e => e.Source.TopBorder * e.Ratio.Y)
                .On(e => e.Source.TopBorder)
                .On(e => e.Ratio.Y)
                .Update()
            );

        public override double BottomBorder
        {
            get => _bottomBorder.Get();
            set => Source.BottomBorder = value / Ratio.Y;
        }
        private readonly IProperty<double> _bottomBorder = H.Property<double>(c => c
                .Set(e => e.Source.BottomBorder * e.Ratio.Y)
                .On(e => e.Source.BottomBorder)
                .On(e => e.Ratio.Y)
                .Update()
            );

        public override double LeftBorder
        {
            get => _leftBorder.Get();
            set => Source.LeftBorder = value / Ratio.X;
        }
        private readonly IProperty<double> _leftBorder = H.Property<double>(c => c
                .Set(e => e.Source.LeftBorder * e.Ratio.X)
                .On(e => e.Source.LeftBorder)
                .On(e => e.Ratio.X)
                .Update()
            );

        public override double RightBorder
        {
            get => _rightBorder.Get();
            set => Source.RightBorder = value / Ratio.X;
        }
        private readonly IProperty<double> _rightBorder = H.Property<double>(c => c
                .Set(e => e.Source.RightBorder * e.Ratio.X)
                .On(e => e.Source.RightBorder)
                .On(e => e.Ratio.X)
                .Update()
            );
    }
}