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
using System.Windows;

namespace LittleBigMouse.ScreenConfigs
{
    public static class ScreenTranslateExt
    {
        public static IScreenSize Translate(this IScreenSize source, Vector translation) => new ScreenTranslate(source, translation);
    }

    public class ScreenTranslate : ScreenSize<ScreenTranslate>
    {
        public ScreenTranslate(IScreenSize source, Vector? translation = null):base(source)
        {
            Translation = translation ?? new Vector();
        }

        private readonly IProperty<Vector> _translation = H.Property<Vector>();
        public Vector Translation
        {
            get => _translation.Get();
            set => _translation.Set(value);
        }

        [TriggerOn(nameof(Source), "Width")]
        public override double Width
        {
            get => Source.Width;
            set => Source.Width = value;
        }

        [TriggerOn(nameof(Source), "Height")]
        public override double Height
        {
            get => Source.Height;
            set => Source.Height = value;
        }

        [TriggerOn(nameof(Source), "X")]
        [TriggerOn(nameof(Translation))]
        public override double X
        {
            get => Source.X + Translation.X;
            set => Source.X = value - Translation.X;
        }

        [TriggerOn(nameof(Source), "Y")]
        [TriggerOn(nameof(Translation))]
        public override double Y
        {
            get => Source.Y + Translation.Y;
            set => Source.Y = value - Translation.Y;
        }

        [TriggerOn(nameof(Source), "TopBorder")]
        public override double TopBorder
        {
            get => Source.TopBorder;
            set => Source.TopBorder = value;
        }

        [TriggerOn(nameof(Source), "RightBorder")]
        public override double RightBorder
        {
            get => Source.RightBorder;
            set => Source.RightBorder = value;
        }

        [TriggerOn(nameof(Source), "BottomBorder")]
        public override double BottomBorder
        {
            get => Source.BottomBorder;
            set => Source.BottomBorder = value;
        }

        [TriggerOn(nameof(Source), "LeftBorder")]
        public override double LeftBorder
        {
            get => Source.LeftBorder;
            set => Source.LeftBorder = value;
        }
    }
}
