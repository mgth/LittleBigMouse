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

using System.Diagnostics;
using Avalonia;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayLocate : DisplayMove
{
    public DisplayLocate(IDisplaySize source, Point? point = null) : base(source)
    {
        Location = point ?? new Point();
    }

    public override double X
    {
        get => _x;
        set
        {
            Debug.Assert(!double.IsInfinity(value));
            this.SetUnsavedValue(ref _x, value);
        }
    }

    double _x;

    public override double Y
    {
        get => _y;
        set
        {
            Debug.Assert(!double.IsInfinity(value));
            this.SetUnsavedValue(ref _y, value);
        }
    }

    double _y;

    public override string TransformToString => $"To:{X},{Y}";

}
