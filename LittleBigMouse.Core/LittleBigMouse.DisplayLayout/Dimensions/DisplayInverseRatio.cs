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


using System.Reactive.Concurrency;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class ScreenInverseRatioExt
{
}

public class DisplayInverseRatio : DisplayRatio
{
    public DisplayInverseRatio(IDisplayRatio ratio)
    {
        Source = ratio;

        _x = this.WhenAnyValue(e => e.Source.X, x => 1/x)
            .ToProperty(this, e => e.X, scheduler: Scheduler.Immediate);
        
        _y = this.WhenAnyValue(e => e.Source.Y, y => 1/y)
            .ToProperty(this, e => e.Y, scheduler: Scheduler.Immediate);
    }

    public IDisplayRatio Source { get; }

    protected override double XValue => _x.Value;
    readonly ObservableAsPropertyHelper<double> _x;

    protected override double YValue => _y.Value;
    readonly ObservableAsPropertyHelper<double> _y;
}
