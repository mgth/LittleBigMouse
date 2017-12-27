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

using HLab.Notify;

namespace LittleBigMouse.ScreenConfigs
{
    public static class ScreenScaleDipExt
    {
        public static ScreenSize ScaleDip(this ScreenSize source) => new ScreenScaleDip(source);
    }
    public class ScreenScaleDip : ScreenSize
    {
        public ScreenScaleDip(ScreenSize source)
        {
            using (this.Suspend())
            {
                Source = source;
            }
            this.SubscribeNotifier();
        }

        [TriggedOn("Screen.PixelToDipRatio")]
        public ScreenRatio Ratio
        {
            get => this.Get<ScreenRatio>(()=>new ScreenRatioValue(
                96 / Screen.EffectiveDpi.X,
                96 / Screen.EffectiveDpi.Y
            ));
            set => this.Set(value);
        }

        [TriggedOn("Screen.Config.PrimaryScreen.EffectiveDpi.X")]
        [TriggedOn("Screen.Config.PrimaryScreen.EffectiveDpi.Y")]
        public ScreenRatio MainRatio
        {
            get => this.Get(() => new ScreenRatioValue(
                96 / Screen.Config.PrimaryScreen.EffectiveDpi.X,
                96 / Screen.Config.PrimaryScreen.EffectiveDpi.Y
                ));

            set => this.Set(value);
        }

        [TriggedOn(nameof(Source), "Width")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double Width
        {
            get => this.Get(() => Source.Width * Ratio.X);
            set => Source.Width = value / Ratio.X;
        }

        [TriggedOn(nameof(Source), "Height")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double Height
        {
            get => this.Get(() => Source.Height * Ratio.Y);
            set => Source.Height = value / Ratio.Y;
        }


        //return new ScreenRatioValue(
        //    //EffectiveDpi.X / 96,
        //    //EffectiveDpi.Y / 96
        //    DpiAwareAngularDpi.X / 96,
        //    DpiAwareAngularDpi.Y / 96
        //); ;




        [TriggedOn(nameof(Source), "X")]
        [TriggedOn(nameof(MainRatio), "X")]
        public override double X
        {
            get => this.Get(() => Source.X * MainRatio.X);
            set => Source.X = value / MainRatio.X;
        }

        [TriggedOn(nameof(Source), "Y")]
        [TriggedOn(nameof(MainRatio), "Y")]
        public override double Y
        {
            get => this.Get(() => Source.Y * Ratio.Y);
            set => Source.Y = value / MainRatio.Y;
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double TopBorder
        {
            get => this.Get(() => Source.TopBorder * Ratio.Y);
            set => Source.TopBorder = value / Ratio.Y;
        }

        [TriggedOn(nameof(Source), "BottomBorder")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double BottomBorder
        {
            get => this.Get(() => Source.BottomBorder * Ratio.Y);
            set => Source.BottomBorder = value / Ratio.Y;
        }

        [TriggedOn(nameof(Source), "LeftBorder")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double LeftBorder
        {
            get => this.Get(() => Source.LeftBorder * Ratio.X);
            set => Source.LeftBorder = value / Ratio.X;
        }

        [TriggedOn(nameof(Source), "RightBorder")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double RightBorder
        {
            get => this.Get(() => Source.RightBorder * Ratio.X);
            set => Source.RightBorder = value / Ratio.X;
        }
    }
}