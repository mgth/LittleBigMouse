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
using System.Runtime.CompilerServices;
using HLab.Notify.PropertyChanged;
using Microsoft.Win32;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H = H<ScreenRatioRegistry>;

    public class ScreenRatioRegistry : ScreenRatio
    {
        private readonly string _prefix;
        public Screen Screen { get; }

        public ScreenRatioRegistry(Screen screen,[CallerMemberName] string prefix = null)
        {
            Screen = screen;
            _prefix = prefix;
            H.Initialize(this);
        }

        public override double X
        {
            get => _x.Get();
            set { if (_x.Set(value)) Screen.Config.Saved = false; }
        }
        private readonly IProperty<double> _x = H.Property<double>(c => c
            .Set(e => e.LoadValue(()=>1.0,nameof(X)))
        );

        public override double Y
        {
            get => _y.Get();
            set { if (_y.Set(value)) Screen.Config.Saved = false; }
        }
        private readonly IProperty<double> _y = H.Property<double>(c => c
            .Set(e => e.LoadValue(() => 1.0,nameof(Y)))
        );

        double LoadValue(Func<double> def, string name)
        {
            using (RegistryKey key = Screen.OpenConfigRegKey())
            {
                return key.GetKey(_prefix + "." + name, def);
            }
        }
    }
}