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

using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.DisplayLayout.Dimensions;

using H = H<DisplayRatioRatio>;

public class DisplayRatioRatio : DisplayRatio
{
    public DisplayRatioRatio(IDisplayRatio ratioA, IDisplayRatio ratioB)
    {
        SourceA = ratioA;
        SourceB = ratioB;
        H.Initialize(this);
    }

    public IDisplayRatio SourceA { get; }
    public IDisplayRatio SourceB { get; }

    public override double X
    {
        get => _x.Get();
        set => throw new NotImplementedException();
    }
    private readonly IProperty<double> _x = H.Property<double>(c => c
        .Set(s => s.SourceA.X * s.SourceB.X)
        .On(e => e.SourceA.X)
        .On(e => e.SourceB.X)
        .Update()
    );

    public override double Y
    {
        get => _y.Get();
        set => throw new NotImplementedException();
    }
    private readonly IProperty<double> _y = H.Property<double>(c => c
        .Set(s => s.SourceA.Y * s.SourceB.Y)
        .On(e => e.SourceA.Y)
        .On(e => e.SourceB.Y)
        .Update()
    );
}
