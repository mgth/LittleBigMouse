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
using System.Threading;
using System.Windows;
using HLab.Base;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using Newtonsoft.Json;

namespace LittleBigMouse.ScreenConfigs
{
    public interface IScreenRatio
    {
        double X { get; set; }
        double Y { get; set; }
    }

    public abstract class ScreenRatio<TClass> : N<TClass>, IScreenRatio, IEquatable<IScreenRatio>
    where TClass : ScreenRatio<TClass>
    {
        [JsonProperty]
        public abstract double X { get; set; }
        [JsonProperty]
        public abstract double Y { get; set; }

        public bool Equals(IScreenRatio other)
        {
            if (other == null) return false;
            return !(Math.Abs(X - other.X) > double.Epsilon || Math.Abs(Y - other.Y) > double.Epsilon);
        }
    }

    public class ScreenRatioValue : ScreenRatio<ScreenRatioValue>
    {
        public ScreenRatioValue(double x, double y)
        {
            Initialize();
            X = x;
            Y = y;
        }
        public ScreenRatioValue(double r)
        {
            Initialize();
            X = r;
            Y = r;
        }
        public ScreenRatioValue(Vector v)
        {
            Initialize();
            X = v.X;
            Y = v.Y;
        }

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
    }
}