/*
  LittleBigMouse.DisplayLayout
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.DisplayLayout.

    LittleBigMouse.DisplayLayout is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.DisplayLayout is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Reactive.Linq;
using System.Runtime.CompilerServices;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayRatioRegistry : MutableDisplayRatio
{
    readonly string _prefix;
    public PhysicalMonitor Monitor { get; }

    public DisplayRatioRegistry(PhysicalMonitor screen, [CallerMemberName] string prefix = null)
    {
        Monitor = screen;
        _prefix = prefix;

        _x = LoadValue(nameof(X), () => 1.0);
        _y = LoadValue(nameof(Y), () => 1.0);

        this.WhenAnyValue(
                e => e.X,
                e  => e.Y)
            .Do((e) =>
            {
                // TODO
                //Monitor.Layout.Saved = false;
            });

    }

    public override double X
    {
        get => _x;
        set => this.RaiseAndSetIfChanged(ref _x, value);
    }
    double _x;

    public override double Y
    {
        get => _y;
        set => this.RaiseAndSetIfChanged(ref _y, value);
    }
    double _y;

    double LoadValue(string name, Func<double> defaultValue)
    {
        // TODO 
        //using var key = Monitor.OpenRegKey();
        //return key.GetKey(_prefix + "." + name, defaultValue);
        return 0.0;
    }
}
