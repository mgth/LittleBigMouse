/*
  LittleBigMouse.Daemon
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Daemon.

    LittleBigMouse.Daemon is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Daemon is distributed in the hope that it will be useful,
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
using System.Threading;
using System.Windows;
using HLab.Sys.Windows.API;
using Microsoft.Win32;

namespace LittleBigMouse.Daemon
{
    class LbmMouse
    {
        public static uint MouseEvent(NativeMethods.MOUSEEVENTF evt, double x, double y)
        {
            NativeMethods.InputUnion[] input = {
            new NativeMethods.InputUnion
            {
                type = 0,
                mi = new NativeMethods.MOUSEINPUT
                {
                    dwFlags = evt ,
                    dx = (int)x,
                    dy = (int)y,
                    time = 0,
                    mouseData = 0,
                    dwExtraInfo = UIntPtr.Zero
                }
            }};

            return NativeMethods.SendInput((uint)input.Length, input, Marshal.SizeOf<NativeMethods.InputUnion>());
        }

        public static Point CursorPos
        {
            get
            {
                NativeMethods.GetCursorPos(out var p);
                return p;
            }
            set
            {
                NativeMethods.SetCursorPos((int) value.X, (int) value.Y);
                //new Thread(() => 
                //{
                //    /* run your code here */ 
                //    NativeMethods.SetCursorPos((int) value.X, (int) value.Y);
                //}).Start();
            }
        }

        public static double MouseSpeed
        {
            get {
                uint speed = 0;
                NativeMethods.SystemParametersInfo(NativeMethods.SPI_GETMOUSESPEED, 0, ref speed, 0);
                return speed;
            }

            set => NativeMethods.SystemParametersInfo(NativeMethods.SPI_SETMOUSESPEED, 0, (uint)Math.Round(value,0), 0);
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

            NativeMethods.SystemParametersInfo(NativeMethods.SPI_SETCURSORS, 0, 0, NativeMethods.SPIF_UPDATEINIFILE | NativeMethods.SPIF_SENDCHANGE);
        }

        public static void SaveCursor(RegistryKey savekey)
        {
            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Cursors\"))
            {
                if (key == null) return;
                foreach (string name in key.GetValueNames())
                {
                    savekey.SetValue(name, key.GetValue(name));
                }
            }
        }

        public static void RestoreCursor(RegistryKey savekey)
        {
            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"\Control Panel\Cursors\"))
            {
                if (key == null) return;

                foreach (string name in savekey.GetValueNames())
                {
                    key.SetValue(name, savekey.GetValue(name));
                }
            }
            NativeMethods.SystemParametersInfo(NativeMethods.SPI_SETCURSORS, 0, 0, NativeMethods.SPIF_UPDATEINIFILE | NativeMethods.SPIF_SENDCHANGE);
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
