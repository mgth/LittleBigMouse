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

namespace LittleBigMouse.DisplayLayout.Dimensions;

public static class ScreenSizeWpfExt
{
    public static IDisplaySize Wpf(this DisplaySizeInPixels source, IDisplayRatio ratio) => new DisplayScale(source, ratio);
}
public class DisplaySizeWpf : DisplaySize
{
    public DisplaySizeWpf(IDisplaySize source) : base(source)
    {
        Init();
    }

    public IDisplayRatio Ratio
    {
        get => _ratio;
        set => _ratio = value;
    }
    IDisplayRatio _ratio;

    public override double Width
    {
        get => Source.Width * Ratio.X;
        set => Source.Width = value / Ratio.X;
    }

    public override double Height
    {
        get => Source.Height * Ratio.Y;
        set => Source.Height = value / Ratio.Y;
    }

    public override double X
    {
        get => Source.X * Ratio.X;
        set => Source.X = value / Ratio.X;
    }

    public override double Y
    {
        get => Source.Y * Ratio.Y;
        set => Source.Y = value / Ratio.Y;
    }

    public override double TopBorder
    {
        get => Source.TopBorder * Ratio.Y;
        set => Source.TopBorder = value / Ratio.Y;
    }

    public override double BottomBorder
    {
        get => Source.BottomBorder * Ratio.Y;
        set => Source.BottomBorder = value / Ratio.Y;
    }

    public override double LeftBorder
    {
        get => Source.LeftBorder * Ratio.X;
        set => Source.LeftBorder = value / Ratio.X;
    }

    public override double RightBorder
    {
        get => Source.RightBorder * Ratio.X;
        set => Source.RightBorder = value / Ratio.X;
    }
}
