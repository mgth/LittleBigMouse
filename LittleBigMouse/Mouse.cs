/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Runtime.InteropServices;
using System.Windows;
using WinAPI_User32;

namespace LittleBigMouse
{
    public class Mouse
    {



        public static Point CursorPos
        {
            get
            {
                User32.POINT p=new User32.POINT();
                User32.GetCursorPos(out p);
                return p;
            }
            set
            {
                User32.SetCursorPos((int)value.X, (int)value.Y);
            }
/*            {
                InputUnion[] input = new InputUnion[1];
                input[0] = new InputUnion()
                {
                    mi = new MOUSEINPUT()
                    {
                        dwFlags =
            //                WinAPI_User32.MOUSEEVENTF.ABSOLUTE |
            //                WinAPI_User32.MOUSEEVENTF.VIRTUALDESK |
                            MOUSEEVENTF.MOVE |
                            MOUSEEVENTF.MOVE_NOCOALESCE
                            ,
                        dx = (int)value.X,
                        dy = (int)value.Y,
                    }
                };  


                uint res = User32.SendInput(1, input, Marshal.SizeOf(typeof(WinAPI_User32.MOUSEINPUT)));
                Console.WriteLine(res);

            }*/
        }

        static public double MouseSpeed
        {
            get
            {
                uint speed = 0;
                User32.SystemParametersInfo(User32.SPI_GETMOUSESPEED, 0, ref speed, 0);
                return speed;
            }

            set
            {
                User32.SystemParametersInfo(User32.SPI_SETMOUSESPEED, 0, (uint)Math.Round(value,0), 0);
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
            setCursor("Crosshair", "");//
            setCursor("Hand", "%SystemRoot%\\cursors\\aero_link" + suffix + ".cur");
            setCursor("Help", "%SystemRoot%\\cursors\\aero_helpsel" + suffix + ".cur");
            setCursor("IBeam", "");//
            setCursor("No", "%SystemRoot%\\cursors\\aero_unavail" + suffix + ".cur");
            setCursor("NWPen", "%SystemRoot%\\cursors\\aero_pen" + suffix + ".cur");
            setCursor("SizeAll", "%SystemRoot%\\cursors\\aero_move" + suffix + ".cur");
            setCursor("SizeNESW", "%SystemRoot%\\cursors\\aero_nesw" + suffix + ".cur");
            setCursor("SizeNS", "%SystemRoot%\\cursors\\aero_ns" + suffix + ".cur");
            setCursor("SizeNWSE", "%SystemRoot%\\cursors\\aero_nwse" + suffix + ".cur");
            setCursor("SizeWE", "%SystemRoot%\\cursors\\aero_ew" + suffix + ".cur");
            setCursor("UpArrow", "%SystemRoot%\\cursors\\aero_up" + suffix + ".cur");
            setCursor("Wait", "%SystemRoot%\\cursors\\aero_busy" + suffix + ".ani");

            User32.SystemParametersInfo(User32.SPI_SETCURSORS, 0, 0, User32.SPIF_UPDATEINIFILE | User32.SPIF_SENDCHANGE);
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
