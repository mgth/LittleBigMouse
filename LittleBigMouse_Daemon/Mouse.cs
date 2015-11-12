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

namespace LittleBigMouse_Daemon
{
    public class Mouse
    {
        private static void MouseEvent(WinAPI_User32.MOUSEEVENTF evt, double x, double y)
        {
            WinAPI_User32.InputUnion[] input = new WinAPI_User32.InputUnion[1];
            input[0] = new WinAPI_User32.InputUnion()
            {
                mi = new WinAPI_User32.MOUSEINPUT()
                {
                    dwFlags = evt
                        ,
                    dx = (int)x,
                    dy = (int)y,
                }
            };


            uint res = WinAPI_User32.User32.SendInput(1, input, Marshal.SizeOf(typeof(WinAPI_User32.MOUSEINPUT)));

        }

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

                User32.POINT newLocation;
                User32.GetCursorPos(out newLocation);

                MouseEvent(MOUSEEVENTF.MOVE | MOUSEEVENTF.ABSOLUTE, value.X, value.Y);
                                        //(DWORD)((65535.0f * x) / (w - 1) + 0.5f),
                                        //(DWORD)((65535.0f * y) / (h - 1) + 0.5f),
                                        //0, 0);

            }
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
        public static void SetCursor(string name, string fileName)
        {
            Microsoft.Win32.Registry.SetValue(@"HKEY_CURRENT_USER\Control Panel\Cursors\", name, fileName);
        }

        public static void SetCursorAero(string suffix)
        {
            SetCursor("AppStarting", @"%SystemRoot%\cursors\aero_working" + suffix + ".ani");
            SetCursor("Arrow", @"%SystemRoot%\cursors\aero_arrow" + suffix + ".cur");
            SetCursor("Crosshair", "");//
            SetCursor("Hand", @"%SystemRoot%\cursors\aero_link" + suffix + ".cur");
            SetCursor("Help", @"%SystemRoot%\cursors\aero_helpsel" + suffix + ".cur");
            SetCursor("IBeam", "");//
            SetCursor("No", @"%SystemRoot%\cursors\aero_unavail" + suffix + ".cur");
            SetCursor("NWPen", @"%SystemRoot%\cursors\aero_pen" + suffix + ".cur");
            SetCursor("SizeAll", @"%SystemRoot%\cursors\aero_move" + suffix + ".cur");
            SetCursor("SizeNESW", @"%SystemRoot%\cursors\aero_nesw" + suffix + ".cur");
            SetCursor("SizeNS", @"%SystemRoot%\cursors\aero_ns" + suffix + ".cur");
            SetCursor("SizeNWSE", @"%SystemRoot%\cursors\aero_nwse" + suffix + ".cur");
            SetCursor("SizeWE", @"%SystemRoot%\cursors\aero_ew" + suffix + ".cur");
            SetCursor("UpArrow", @"%SystemRoot%\cursors\aero_up" + suffix + ".cur");
            SetCursor("Wait", @"%SystemRoot%\cursors\aero_busy" + suffix + ".ani");

            User32.SystemParametersInfo(User32.SPI_SETCURSORS, 0, 0, User32.SPIF_UPDATEINIFILE | User32.SPIF_SENDCHANGE);
        }

        public static void SetCursorAero(int size)
        {
            switch (size)
            {
                case 1:
                    SetCursorAero("");
                    break;
                case 2:
                    SetCursorAero("_l");
                    break;
                case 3:
                    SetCursorAero("_xl");
                    break;
            }
        }
    }
}
