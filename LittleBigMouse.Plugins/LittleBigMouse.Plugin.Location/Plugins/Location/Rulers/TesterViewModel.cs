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

using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers;

using H = H<TesterViewModel>;
public class TesterViewModel : NotifierBase
{
    public TesterViewModel() => H.Initialize(this);

    public double LeftInDip
    {
        get => _leftInDip.Get();
        set => _leftInDip.Set(value);
    }
    private readonly IProperty<double> _leftInDip = H.Property<double>();

    public double RightInDip
    {
        get => _rightInDip.Get();
        set => _rightInDip.Set(value);
    }
    private readonly IProperty<double> _rightInDip = H.Property<double>();

    public double TopInDip
    {
        get => _topInDip.Get();
        set => _topInDip.Set(value);
    }
    private readonly IProperty<double> _topInDip = H.Property<double>();

    public double BottomInDip
    {
        get => _bottomInDip.Get();
        set => _bottomInDip.Set(value);
    }
    private readonly IProperty<double> _bottomInDip = H.Property<double>();

    public double HeightInDip
    {
        get => _heightInDip.Get();
        set => _heightInDip.Set(value);
    }
    private readonly IProperty<double> _heightInDip = H.Property<double>();

    public double WidthInDip
    {
        get => _widthInDip.Get();
        set => _widthInDip.Set(value);
    }
    private readonly IProperty<double> _widthInDip = H.Property<double>();
}
