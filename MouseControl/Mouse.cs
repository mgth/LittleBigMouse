using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace MouseControl
{
    public class Mouse
    {
        const UInt32 SPI_SETCURSORS = 0x0057;
        const UInt32 SPIF_UPDATEINIFILE = 0x01;
        const UInt32 SPIF_SENDCHANGE = 0x02;
        const UInt32 SPI_SETMOUSESPEED = 0x0071;
        const UInt32 SPI_GETMOUSESPEED = 0x0070;

        [DllImport("user32.dll")]
        public static extern Boolean SetCursorPos(Int32 x, Int32 y);

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT
        {
            public int X;
            public int Y;

            public POINT(int x, int y)
            {
                this.X = x;
                this.Y = y;
            }

            public POINT(System.Drawing.Point pt) : this(pt.X, pt.Y) { }

            public static implicit operator Point(POINT p)
            {
                return new Point(p.X, p.Y);
            }

            public static implicit operator POINT(Point p)
            {
                return new POINT((int)p.X, (int)p.Y);
            }
        }
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        static extern bool GetCursorPos(out POINT lpPoint);

        public static Point CursorPos
        {
            get
            {
                POINT p=new POINT();
                GetCursorPos(out p);
                return p;
            }
        }

        [DllImport("User32.dll")]
        static extern Boolean SystemParametersInfo(
            UInt32 uiAction,
            UInt32 uiParam,
            UInt32 pvParam,
            UInt32 fWinIni);

        [DllImport("User32.dll")]
        static extern Boolean SystemParametersInfo(
            UInt32 uiAction,
            UInt32 uiParam,
            ref UInt32 pvParam,
            UInt32 fWinIni);
        static public double MouseSpeed
        {
            get
            {
                uint speed = 0;
                SystemParametersInfo(SPI_GETMOUSESPEED, 0, ref speed, 0);
                return speed;
            }

            set
            {
                SystemParametersInfo(SPI_SETMOUSESPEED, 0, (uint)value, 0);
            }
        }
        public static void setCursor(String name, String fileName)
        {
            Microsoft.Win32.Registry.SetValue(@"HKEY_CURRENT_USER\Control Panel\Cursors\", name, fileName);
        }

        public static void setCursorAero(String suffix)
        {
            setCursor("AppStarting", "%SystemRoot%\\cursors\\aero_working" + suffix + ".ani");
            setCursor("Arrow", "%SystemRoot%\\cursors\\aero_arrow" + suffix + ".cur");
            setCursor("Hand", "%SystemRoot%\\cursors\\aero_link" + suffix + ".cur");
            setCursor("Help", "%SystemRoot%\\cursors\\aero_helpsel" + suffix + ".cur");
            setCursor("No", "%SystemRoot%\\cursors\\aero_unavail" + suffix + ".cur");
            setCursor("NWPen", "%SystemRoot%\\cursors\\aero_pen" + suffix + ".cur");
            setCursor("SizeAll", "%SystemRoot%\\cursors\\aero_move" + suffix + ".cur");
            setCursor("SizeNESW", "%SystemRoot%\\cursors\\aero_nesw" + suffix + ".cur");
            setCursor("SizeNS", "%SystemRoot%\\cursors\\aero_ns" + suffix + ".cur");
            setCursor("SizeNWSE", "%SystemRoot%\\cursors\\aero_nwse" + suffix + ".cur");
            setCursor("SizeWE", "%SystemRoot%\\cursors\\aero_we" + suffix + ".cur");
            setCursor("UpArrow", "%SystemRoot%\\cursors\\aero_up" + suffix + ".cur");
            setCursor("Wait", "%SystemRoot%\\cursors\\aero_busy" + suffix + ".cur");

            SystemParametersInfo(SPI_SETCURSORS, 0, 0, SPIF_UPDATEINIFILE | SPIF_SENDCHANGE);
        }

        public static void setCursorAero(int size)
        {
            switch(size)
            {
                case 1:
                    setCursorAero("");
                    break;
                case 2:
                    setCursorAero("_l");
                    break;
                case 3:
                    setCursorAero("_xl");
                    break;
            }
        }
    }
}
