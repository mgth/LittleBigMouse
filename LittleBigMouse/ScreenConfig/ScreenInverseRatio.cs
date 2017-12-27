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

using HLab.Notify;

namespace LittleBigMouse.ScreenConfigs
{
    public static class ScreenInverseRatioExt
    {
        public static ScreenRatio Inverse(this ScreenRatio source) => new ScreenInverseRatio(source);
    }
    public class ScreenInverseRatio : ScreenRatio
    {
        public ScreenRatio Source
        {
            get => this.Get<ScreenRatio>();
            private set => this.Set(value);
        }

        public ScreenInverseRatio(ScreenRatio ratio)
        {
            Source = ratio;
            this.SubscribeNotifier();
        }

        [TriggedOn(nameof(Source), "X")]
        public override double X
        {
            get => this.Get(()=> 1/Source.X);
            set => Source.X = 1/value;
        }

        [TriggedOn(nameof(Source), "Y")]
        public override double Y
        {
            get => this.Get(()=> 1/Source.Y);
            set => Source.Y = 1/value;
        }
    }
}