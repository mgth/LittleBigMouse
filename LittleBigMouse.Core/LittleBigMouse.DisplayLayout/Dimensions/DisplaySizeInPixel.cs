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
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplaySizeInPixels : DisplaySize
{
    public DisplaySizeInPixels(Rect rect) : base(null)
    {
        Init();
        this.Set(rect);
    }

    public override double Width { get; set => SetUnsavedValue(ref field, value); }
    public override double Height { get; set => SetUnsavedValue(ref field, value); }
    public override double X { get; set => SetUnsavedValue(ref field, value); }
    public override double Y { get; set => SetUnsavedValue(ref field, value); }

    public override double TopBorder { get => 0; set => throw new NotImplementedException(); }
    public override double BottomBorder { get => 0; set => throw new NotImplementedException(); }
    public override double LeftBorder { get => 0; set => throw new NotImplementedException(); }
    public override double RightBorder { get => 0; set => throw new NotImplementedException(); }
}
