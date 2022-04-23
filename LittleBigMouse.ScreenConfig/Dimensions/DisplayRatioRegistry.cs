/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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

namespace LittleBigMouse.DisplayLayout.Dimensions;

using H = H<DisplayRatioRegistry>;

public class DisplayRatioRegistry : DisplayRatio
{
    private readonly string _prefix;
    public Monitor Monitor { get; }

    public DisplayRatioRegistry(Monitor screen, [CallerMemberName] string prefix = null)
    {
        Monitor = screen;
        _prefix = prefix;
        H.Initialize(this);
    }

    public override double X
    {
        get => _x.Get();
        set { if (_x.Set(value)) Monitor.Layout.Saved = false; }
    }
    private readonly IProperty<double> _x = H.Property<double>(c => c
        .Set(e => e.LoadValue(nameof(X), () => 1.0))
    );

    public override double Y
    {
        get => _y.Get();
        set { if (_y.Set(value)) Monitor.Layout.Saved = false; }
    }
    private readonly IProperty<double> _y = H.Property<double>(c => c
        .Set(e => e.LoadValue(nameof(Y), () => 1.0))
    );

    double LoadValue(string name, Func<double> defaultValue)
    {
        using (RegistryKey key = Monitor.OpenRegKey())
        {
            return key.GetKey(_prefix + "." + name, defaultValue);
        }
    }
}
