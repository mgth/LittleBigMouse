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

using ReactiveUI;
using System.Reactive.Concurrency;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class DisplayTranslateExt
{
    public static IDisplaySize Translate(this IDisplaySize source, Vector translation) => new DisplayTranslate(source, translation);
}

public class DisplayTranslate : DisplayMove
{
    public DisplayTranslate(IDisplaySize source, Vector? translation = null) : base(source)
    {
        Translation = translation ?? new Vector();
        
        _x = this.WhenAnyValue(e => e.Source.X,e =>e.Translation, (x,t)=>x + t.X)
            .ToProperty(this, e => e.X, scheduler: Scheduler.Immediate);

        _y = this.WhenAnyValue(e => e.Source.Y,e =>e.Translation, (y,t)=>y + t.Y)
            .ToProperty(this, e => e.Y, scheduler: Scheduler.Immediate);
    }

    public Vector Translation { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    // Base Init() pipelines read these virtual getters from a pool thread and can win
    // the race against this constructor: fall back to the pipeline's own formula.
    public override double X
    {
        get => _x?.Value ?? Source.X + Translation.X;
        set => Source.X = value - Translation.X;
    }
    readonly ObservableAsPropertyHelper<double> _x;

    public override double Y
    {
        get => _y?.Value ?? Source.Y + Translation.Y;
        set => Source.Y = value - Translation.Y;
    }
    readonly ObservableAsPropertyHelper<double> _y;

    public override string TransformToString => $"Translate:{Translation}";
}
