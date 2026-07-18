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
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// Actual real monitor size 
/// </summary>
public class DisplaySizeInMm : DisplaySize
{
    public DisplaySizeInMm() : base(null) => Init();

    public bool FixedAspectRatio { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    public override double Width { get; set {
          using (DelayChangeNotifications())
          {
             var newValue = Math.Max(value, 0);
             var oldValue = field;

             if (FixedAspectRatio)
             {
                var ratio = newValue / oldValue;
                FixedAspectRatio = false;
                Height *= ratio;
                FixedAspectRatio = true;
             }

             SetUnsavedValue(ref field, value);
          }
       }
    }

    public override double Height { get; set {
          using (DelayChangeNotifications())
          {
             var newValue = Math.Max(value, 0);
             var oldValue = field;

             if (FixedAspectRatio)
             {
                var ratio = newValue / oldValue;
                FixedAspectRatio = false;
                Width *= ratio;
                FixedAspectRatio = true;
             }

             SetUnsavedValue(ref field, value);
          }
       }
    }

    public override double X { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    public override double Y { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    public override double TopBorder { get; set => SetUnsavedValue(ref field, Math.Max(value, 0.0)); } = 20.0;

    public override double RightBorder { get; set => SetUnsavedValue(ref field, Math.Max(value, 0.0)); } = 20.0;

    public override double BottomBorder { get; set => this.SetUnsavedValue(ref field, Math.Max(value, 0.0)); } = 20.0;

    public override double LeftBorder { get; set => this.SetUnsavedValue(ref field, Math.Max(value, 0.0)); } = 20.0;

    public override string TransformToString => $"InMm";

}
