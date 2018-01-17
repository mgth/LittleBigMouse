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

using System.Windows;
using HLab.Notify;

namespace LittleBigMouse.ScreenConfigs
{
    public static class ScreenLocateExt
    {
        public static ScreenSize Locate(this ScreenSize source, Point? point = null) => new ScreenLocate(source, point);
    }

    public class ScreenLocate : ScreenSize
    {
        public ScreenLocate(ScreenSize source, Point? point = null)
        {
            Source = source;
            Location = point??new Point();
            this.SubscribeNotifier();
        }


        [TriggedOn(nameof(Source), "Width")]
        public override double Width
        {
            get => this.Get(() => Source.Width);
            set => this.Set(Source.Width = value);
        }

        [TriggedOn(nameof(Source), "Height")]
        public override double Height
        {
            get => this.Get(() => Source.Height);
            set => this.Set(Source.Height = value);
        }

        [TriggedOn(nameof(Source), "X")]
        public override double X
        {
            get => this.Get<double>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(Source), "Y")]
        public override double Y
        {
            get => this.Get<double>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        public override double TopBorder
        {
            get => this.Get(() => Source.TopBorder);
            set => Source.TopBorder = value;
        }

        [TriggedOn(nameof(Source), "RightBorder")]
        public override double RightBorder
        {
            get => this.Get(() => Source.RightBorder);
            set => Source.RightBorder = value;
        }

        [TriggedOn(nameof(Source), "BottomBorder")]
        public override double BottomBorder
        {
            get => this.Get(() => Source.BottomBorder);
            set => Source.BottomBorder = value;
        }

        [TriggedOn(nameof(Source), "LeftBorder")]
        public override double LeftBorder
        {
            get => this.Get(() => Source.LeftBorder);
            set => Source.LeftBorder = value;
        }
    }
}
