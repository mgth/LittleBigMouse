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
using System.Text.Json.Serialization;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// Actual real monitor size 
/// </summary>
public class DisplaySizeInMm : DisplaySize
{
    [JsonIgnore]
    public MonitorModel ScreenModel { get; }
    public DisplaySizeInMm(MonitorModel screenModel) : base(null)
    {
        ScreenModel = screenModel;
    }

    public bool Saved
    {
        get => _saved;
        set => this.RaiseAndSetIfChanged(ref _saved, value);
    }
    bool _saved;

    public bool FixedAspectRatio
    {
        get => _fixedAspectRatio;
        set => this.RaiseAndSetIfChanged(ref _fixedAspectRatio, value);
    }
    bool _fixedAspectRatio;//PropertyBuilder.DefaultValue(true);

    public override double Width
    {
        get => _width;

        set
        {
            using (DelayChangeNotifications())
            {
                var newValue = Math.Max(value, 0);
                var oldValue = _width;

                if (FixedAspectRatio)
                {
                    var ratio = newValue / oldValue;
                    FixedAspectRatio = false;
                    Height *= ratio;
                    FixedAspectRatio = true;
                }

                Saved = false;

                this.RaiseAndSetIfChanged(ref _width, value);
            }
        }
    }
    double _width;

    public override double Height
    {
        get => _height;
        set
        {
            using (DelayChangeNotifications())
            {
                var newValue = Math.Max(value, 0);
                var oldValue = _height;

                if (FixedAspectRatio)
                {
                    var ratio = newValue / oldValue;
                    FixedAspectRatio = false;
                    Width *= ratio;
                    FixedAspectRatio = true;
                }

                Saved = false;

                this.RaiseAndSetIfChanged(ref _height, value);
            }
        }
    }
    double _height;

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

    public override double TopBorder
    {
        get => _topBorder;
        set => this.RaiseAndSetIfChanged(ref _topBorder, Math.Max(value, 0.0));
    }
    double _topBorder = 20.0;

    public override double BottomBorder
    {
        get => _bottomBorder;
        set => this.RaiseAndSetIfChanged(ref _bottomBorder, Math.Max(value, 0.0));
    }
    double _bottomBorder = 20.0;

    public override double LeftBorder
    {
        get => _leftBorder;
        set => this.RaiseAndSetIfChanged(ref _leftBorder, Math.Max(value, 0.0));
    }
    double _leftBorder = 20.0;

    public override double RightBorder
    {
        get => _rightBorder;
        set => this.RaiseAndSetIfChanged(ref _rightBorder, Math.Max(value, 0.0));
    }
    double _rightBorder = 20.0;
}
