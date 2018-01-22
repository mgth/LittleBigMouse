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

using System;
using System.Runtime.CompilerServices;
using HLab.Notify;
using HLab.Windows.Monitors;
using Microsoft.Win32;

namespace LittleBigMouse.ScreenConfigs
{
    /// <summary>
    /// Actual real monitor size 
    /// </summary>
    public class ScreenSizeInMm : ScreenSize
    {
        public ScreenModel ScreenModel { get; }
        public ScreenSizeInMm(ScreenModel screen)
        {
            ScreenModel = screen;
            this.SubscribeNotifier();
        }

        public bool Saved
        {
            get => this.Get<bool>();
            set => this.Set(value);
        }

        public bool FixedAspectRatio
        {
            get => this.Get(() => true);
            set => this.Set(value);
        }

        public override double Width
        {
            get => this.Get(() => 0.0);

            set => this.Set(Math.Max(value, 0), (oldValue, newValue) =>
            {
                if (FixedAspectRatio)
                {
                    var ratio = newValue / oldValue;
                    FixedAspectRatio = false;
                    Height *= ratio;
                    FixedAspectRatio = true;
                }

                Saved = false;
            });
        }

        public override double Height
        {
            get => this.Get(() => 0.0);
            set
            {
                this.Set(Math.Max(value, 0), (oldValue, newValue) =>
                {
                    if (FixedAspectRatio)
                    {
                        var ratio = newValue / oldValue;
                        FixedAspectRatio = false;
                        Width *= ratio;
                        FixedAspectRatio = true;
                    }

                    Saved = false;
                } );
            }
        }

        public override double X
        {
            get => this.Get(()=>0.0);
            set => this.Set(value);
        }

        public override double Y
        {
            get => this.Get(() => 0.0);
            set => this.Set(value);
        }


        public override double TopBorder
        {
            get => this.Get(() => 20.0);
            set => this.Set(Math.Max(value,0.0));
        }
        public override double BottomBorder
        {
            get => this.Get(() => 20.0);
            set => this.Set(Math.Max(value, 0.0));
        }
        public override double LeftBorder
        {
            get => this.Get(() => 20.0);
            set => this.Set(Math.Max(value, 0.0));
        }
        public override double RightBorder
        {
            get => this.Get(() => 20.0);
            set => this.Set(Math.Max(value, 0.0));
        }

    }
}