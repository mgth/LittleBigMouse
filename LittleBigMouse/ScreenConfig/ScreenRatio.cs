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

using System;
using HLab.Notify;
using Newtonsoft.Json;

namespace LittleBigMouse.ScreenConfigs
{
    public abstract class ScreenRatio : NotifierObject
    {
        [JsonProperty]
        public abstract double X { get; set; }
        [JsonProperty]
        public abstract double Y { get; set; }

    }

    public class ScreenRatioValue : ScreenRatio
    {
        public ScreenRatioValue(double x, double y)
        {
            X = x;
            Y = y;
            this.SubscribeNotifier();
        }
        public ScreenRatioValue(double r)
        {
            X = r;
            Y = r;
            this.SubscribeNotifier();
        }

        public override double X
        {
            get => this.Get<double>();
            set => this.Set(value);
        }

        public override double Y
        {
            get => this.Get<double>();
            set => this.Set(value);
        }
    }
    public static class ScreenRatioRatioExt
    {
        public static ScreenRatio Multiply(this ScreenRatio sourceA,ScreenRatio sourceB) => new ScreenRatioRatio(sourceA,sourceB);
    }
    public class ScreenRatioRatio : ScreenRatio
    {
        public ScreenRatio SourceA
        {
            get => this.Get<ScreenRatio>();
            private set => this.Set(value);
        }
        public ScreenRatio SourceB
        {
            get => this.Get<ScreenRatio>();
            private set => this.Set(value);
        }

        public ScreenRatioRatio(ScreenRatio ratioA, ScreenRatio ratioB)
        {
            using (this.Suspend())
            {
                SourceA = ratioA;
                SourceB = ratioB;
            }
        }

        [TriggedOn(nameof(SourceA), "X")]
        [TriggedOn(nameof(SourceB), "X")]
        public override double X
        {
            get => this.Get(() => SourceA.X * SourceB.X);
            set => throw new NotImplementedException();
        }

        [TriggedOn(nameof(SourceA), "Y")]
        [TriggedOn(nameof(SourceB), "Y")]
        public override double Y
        {
            get => this.Get(() => SourceA.Y * SourceB.Y);
            set => throw new NotImplementedException();
        }
    }
}