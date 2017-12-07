/*
  HLab.Windows.Monitors
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Windows;
using WinAPI;

namespace HLab.Windows.Monitors
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
            IntPtr hdc = NativeMethods.CreateDC("DISPLAY", deviceName, null, IntPtr.Zero);

            Size = new Size(
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.HORZSIZE),
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.VERTSIZE)
            );

            Resolution = new Size(
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.HORZRES),
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.VERTRES)
            );

            LogPixels = new Size(
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.LOGPIXELSX),
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.LOGPIXELSY)
            );

            BitsPixel = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.BITSPIXEL);

            Aspect = new Size(
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.ASPECTX),
                NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.ASPECTY)
            );

            // TODO : https://msdn.microsoft.com/en-us/library/windows/desktop/dd144877(v=vs.85).aspx

            NativeMethods.DeleteDC(hdc);
        }
    }
}
