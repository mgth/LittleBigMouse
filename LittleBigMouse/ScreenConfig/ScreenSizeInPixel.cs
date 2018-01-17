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
using Microsoft.Win32;

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenSizeInPixels : ScreenSize
    {
        public Screen Screen { get; }

        public ScreenSizeInPixels(Screen screen)
        {
            Screen = screen;
            this.SubscribeNotifier();
        }



//        [TriggedOn("Screen.Monitor.DisplayOrientation")]
        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double Width
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.Width);
            //get => this.Get(() => Screen.Monitor.DisplayOrientation % 2 == 0 ? Screen.Monitor.MonitorArea.Width : Screen.Monitor.MonitorArea.Height);
            set => throw new NotImplementedException();
        }

//        [TriggedOn("Screen.Monitor.DisplayOrientation")]
        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double Height
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.Height);
            //get => this.Get(() => Screen.Monitor.DisplayOrientation % 2 == 0 ? Screen.Monitor.MonitorArea.Height : Screen.Monitor.MonitorArea.Width);
            set => throw new NotImplementedException();
        }

        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double X
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.X);
            set => throw new NotImplementedException();
        }

        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double Y
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.Y);
            set => throw new NotImplementedException();
        }
        public override double TopBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        public override double BottomBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        public override double LeftBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        public override double RightBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        private double LoadValueMonitor(Func<double> def, [CallerMemberName]string name = null)
        {
            using (RegistryKey key = Screen.OpenMonitorRegKey())
            {
                return key.GetKey(name, def);
            }
        }
        private double LoadValueConfig(Func<double> def, [CallerMemberName]string name = null)
        {
            using (RegistryKey key = Screen.OpenConfigRegKey())
            {
                return key.GetKey(name, def);
            }
        }
    }
}
