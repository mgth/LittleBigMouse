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
using LittleBigMouse.ScreenConfig.Dimensions;

namespace LittleBigMouse.ScreenConfig
{
    using H = H<ScreenInverseRatio>;

    public static class ScreenInverseRatioExt
    {
    }
    public class ScreenInverseRatio : ScreenRatio
    {
        public ScreenInverseRatio(IScreenRatio ratio)
        {
            Source = ratio;
            H.Initialize(this);
        }

        public IScreenRatio Source { get; }

        public override double X
        {
            get => _x.Get();
            set => Source.X = 1/value;
        }
        private readonly IProperty<double> _x = H.Property<double>(c => c
            .Set(e => 1 / e.Source.X)
            .On(e => e.Source.X)
            .Update()
        );

        public override double Y
        {
            get => _y.Get();
            set => Source.Y = 1/value;
        }
        private readonly IProperty<double> _y = H.Property<double>(c => c
            .Set(e => 1 / e.Source.Y)
            .On(e => e.Source.Y)
            .Update()
        );
    }
}