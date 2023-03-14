/*
  HLab.Windows.Monitors
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.Monitors.

    HLab.Windows.Monitors is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.Monitors is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using Avalonia;

using static HLab.Sys.Windows.API.WinGdi;

namespace HLab.Sys.Windows.Monitors
{
    public class DeviceCaps
    {
        public Size Size { get; }
        public Size Resolution { get; }
        public Size LogPixels { get; }
        public Size Aspect { get; }
        public int BitsPixel { get; }

        public DeviceCaps(string deviceName)
        {
            var hdc = CreateDC("DISPLAY", deviceName, null, 0);

            Size = new Size(
                GetDeviceCaps(hdc, DeviceCap.HorzSize),
                GetDeviceCaps(hdc, DeviceCap.VertSize)
            );

            Resolution = new Size(
                GetDeviceCaps(hdc, DeviceCap.HorzRes),
                GetDeviceCaps(hdc, DeviceCap.VertRes)
            );

            LogPixels = new Size(
                GetDeviceCaps(hdc, DeviceCap.LogPixelsX),
                GetDeviceCaps(hdc, DeviceCap.LogPixelsY)
            );

            BitsPixel = GetDeviceCaps(hdc, DeviceCap.BitsPixel);

            Aspect = new Size(
                GetDeviceCaps(hdc, DeviceCap.AspectX),
                GetDeviceCaps(hdc, DeviceCap.AspectY)
            );

            // TODO : https://msdn.microsoft.com/en-us/library/windows/desktop/dd144877(v=vs.85).aspx

            DeleteDC(hdc);
        }
    }
}
