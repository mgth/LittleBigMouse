/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using HLab.Mvvm.ReactiveUI;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

public class TesterViewModel : ViewModel
{
    public double LeftInDip
    {
        get => _leftInDip;
        set => this.RaiseAndSetIfChanged(ref _leftInDip, value);
    }
    double _leftInDip;

    public double RightInDip
    {
        get => _rightInDip;
        set => this.RaiseAndSetIfChanged(ref _rightInDip, value);
    }
    double _rightInDip;

    public double TopInDip
    {
        get => _topInDip;
        set => this.RaiseAndSetIfChanged(ref _topInDip, value);
    }
    double _topInDip;

    public double BottomInDip
    {
        get => _bottomInDip;
        set => this.RaiseAndSetIfChanged(ref _bottomInDip, value);
    }
    double _bottomInDip;

    public double HeightInDip
    {
        get => _heightInDip;
        set => this.RaiseAndSetIfChanged(ref _heightInDip, value);
    }
    double _heightInDip;

    public double WidthInDip
    {
        get => _widthInDip;
        set => this.RaiseAndSetIfChanged(ref _widthInDip, value);
    }
    double _widthInDip;
}
