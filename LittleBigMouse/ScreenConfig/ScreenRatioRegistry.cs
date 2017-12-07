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
using Hlab.Notify;
using Microsoft.Win32;

namespace LbmScreenConfig
{

    public class ScreenRatioRegistry : ScreenRatio
    {
        private readonly string _prefix;

        public ScreenRatioRegistry(Screen screen,[CallerMemberName] string prefix = null)
        {
            Screen = screen;
            _prefix = prefix;
            this.Subscribe();
        }

        public Screen Screen
        {
            get => this.Get<Screen>();
            private set => this.Set(value);
        }
        public override double X
        {
            get => this.Get(() => LoadValue(() => 1.0));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }
        public override double Y
        {
            get => this.Get(() => LoadValue(() => 1.0));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }

        double LoadValue(Func<double> def, [CallerMemberName]string name = null)
        {
            using (RegistryKey key = Screen.OpenConfigRegKey())
            {
                return key.GetKey(_prefix + "." + name, def);
            }
        }
    }
}