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
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using Microsoft.Win32;

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenSizeInPixels : ScreenSize<ScreenSizeInPixels>
    {
        public Screen Screen { get; }

        public ScreenSizeInPixels(Screen screen):base(null)
        {
            Screen = screen;
            Initialize();
        }

//        [TriggerOn("Screen.Monitor.DisplayOrientation")]
        [TriggerOn(nameof(Screen), "Monitor", "MonitorArea")]
        public override double Width
        {
            get => Screen.Monitor.MonitorArea.Width;
            //get => this.Get(() => Screen.Monitor.DisplayOrientation % 2 == 0 ? Screen.Monitor.MonitorArea.Width : Screen.Monitor.MonitorArea.Height);
            set => throw new NotImplementedException();
        }

//        [TriggerOn("Screen.Monitor.DisplayOrientation")]
        [TriggerOn(nameof(Screen), "Monitor", "MonitorArea")]
        public override double Height
        {
            get => Screen.Monitor.MonitorArea.Height;
            //get => this.Get(() => Screen.Monitor.DisplayOrientation % 2 == 0 ? Screen.Monitor.MonitorArea.Height : Screen.Monitor.MonitorArea.Width);
            set => throw new NotImplementedException();
        }

        private readonly IProperty<double> _x = H.Property<double>(c => c
             .On(nameof(Screen), "Monitor", "MonitorArea")
            .Set(s => s.Screen.Monitor.MonitorArea.X));
        public override double X
        {
            get => _x.Get();
            set => throw new NotImplementedException();
        }

        private readonly IProperty<double> _y = H.Property<double>(nameof(Y), c => c
             .On(nameof(Screen), "Monitor", "MonitorArea")
            .Set(s => s.Screen.Monitor.MonitorArea.Y));
        public override double Y
        {
            get => _y.Get();
            set => throw new NotImplementedException();
        }
        public override double TopBorder
        {
            get => 0;
            set => throw new NotImplementedException();
        }
        public override double BottomBorder
        {
            get => 0;
            set => throw new NotImplementedException();
        }
        public override double LeftBorder
        {
            get => 0;
            set => throw new NotImplementedException();
        }
        public override double RightBorder
        {
            get => 0;
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
