/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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

using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H = H<ScreenSizeWpf>;

    public static class ScreenSizeWpfExt
    {
        public static IScreenSize Wpf(this ScreenSizeInPixels source, IScreenRatio ratio) => new ScreenScale(source, ratio);
    }
    public class ScreenSizeWpf : ScreenSize
    {
        public ScreenSizeWpf(IScreenSize source):base(source)
        {
            H.Initialize(this);
        }

        private readonly IProperty<IScreenRatio> _ratio = H.Property<IScreenRatio>();
        public IScreenRatio Ratio
        {
            get => _ratio.Get();
            set => _ratio.Set(value);
        }

        [TriggerOn(nameof(Source), "Width")]
        [TriggerOn(nameof(Ratio), "X")]
        public override double Width
        {
            get => Source.Width * Ratio.X;
            set => Source.Width = value / Ratio.X;
        }

        [TriggerOn(nameof(Source), "Height")]
        [TriggerOn(nameof(Ratio), "Y")]
        public override double Height
        {
            get => Source.Height * Ratio.Y;
            set => Source.Height = value / Ratio.Y;
        }

        [TriggerOn(nameof(Source), "X")]
        [TriggerOn(nameof(Ratio), "X")]
        public override double X
        {
            get => Source.X * Ratio.X;
            set => Source.X = value / Ratio.X;
        }

        [TriggerOn(nameof(Source), "Y")]
        [TriggerOn(nameof(Ratio), "Y")]
        public override double Y
        {
            get => Source.Y * Ratio.Y;
            set => Source.Y = value / Ratio.Y;
        }

        [TriggerOn(nameof(Source), "TopBorder")]
        [TriggerOn(nameof(Ratio), "Y")]
        public override double TopBorder
        {
            get => Source.TopBorder * Ratio.Y;
            set => Source.TopBorder = value / Ratio.Y;
        }

        [TriggerOn(nameof(Source), "BottomBorder")]
        [TriggerOn(nameof(Ratio), "Y")]
        public override double BottomBorder
        {
            get => Source.BottomBorder * Ratio.Y;
            set => Source.BottomBorder = value / Ratio.Y;
        }

        [TriggerOn(nameof(Source), "LeftBorder")]
        [TriggerOn(nameof(Ratio), "X")]
        public override double LeftBorder
        {
            get => Source.LeftBorder * Ratio.X;
            set => Source.LeftBorder = value / Ratio.X;
        }

        [TriggerOn(nameof(Source), "RightBorder")]
        [TriggerOn(nameof(Ratio), "X")]
        public override double RightBorder
        {
            get => Source.RightBorder * Ratio.X;
            set => Source.RightBorder = value / Ratio.X;
        }
    }
}
