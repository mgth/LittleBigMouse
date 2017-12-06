using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using WinAPI;

namespace WindowsMonitors
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
