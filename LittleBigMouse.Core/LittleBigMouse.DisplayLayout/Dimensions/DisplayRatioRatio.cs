/*
  LittleBigMouse.DisplayLayout
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
using System.Reactive.Concurrency;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayRatioRatio : DisplayRatio
{
    public DisplayRatioRatio(IDisplayRatio ratioA, IDisplayRatio ratioB)
    {
        SourceA = ratioA;
        SourceB = ratioB;

        _x = this.WhenAnyValue(
                e => e.SourceA.X,
                e => e.SourceB.X,
                (a, b) => a * b)
            .ToProperty(this, e => e.X, scheduler: Scheduler.Immediate);

        _y = this.WhenAnyValue(
                e => e.SourceA.Y,
                e  => e.SourceB.Y, 
                (a,b) => a * b)
            .ToProperty(this, e => e.Y, scheduler: Scheduler.Immediate);


    }

    public IDisplayRatio SourceA { get; }
    public IDisplayRatio SourceB { get; }




    public override double X
    {
        get => _x.Value;
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y.Value;
        set => throw new NotImplementedException();
    }
    readonly ObservableAsPropertyHelper<double> _y;
}
