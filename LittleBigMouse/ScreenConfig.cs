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

using Microsoft.Win32;
using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;
using System;
using System.Collections.Generic;
using System.Windows;

namespace LittleBigMouse
{
    public class ScreenConfig
    {
        RegistryKey _key;
        public ScreenConfig(RegistryKey key)
        {
            _key = key;
        }

        public event EventHandler RegistryChanged;

        private List<Screen> _allScreens = new List<Screen>();
        private readonly MouseHookListener _MouseHookManager = new MouseHookListener(new GlobalHooker());

        private Screen _currentScreen=null;
        private Point _oldPoint;
        private bool _enabled;
        private bool _adjustPointer;
        private bool _adjustSpeed;

        public void Enable()
        {
            if (Enabled)
            {
                _MouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
                _MouseHookManager.Enabled = true;
            }
        }
        public void Disable()
        {
            _MouseHookManager.MouseMoveExt -= _MouseHookManager_MouseMoveExt;
            _MouseHookManager.Enabled = false;
        }

        private void _MouseHookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            Point pIn = new Point(e.X, e.Y);
            Point pOut;

            if (_currentScreen == null) _currentScreen = FromPoint(pIn);

            if (_currentScreen.InsideBounds.Contains(pIn))
            {
                _oldPoint = pIn;
                return;
            }

            Point pOutPhysical = _currentScreen.PixelToPhysical(pIn);

            Screen screenOut = FromPhysicalPoint(pOutPhysical);

            // if new position is within another screen
            if (screenOut != null)
            {
                pOut = screenOut.PhysicalToPixel(pOutPhysical);

                Mouse.SetCursorPos((int)pOut.X, (int)pOut.Y);

                _currentScreen = screenOut;

                if (AdjustPointer)
                {
                    if (_currentScreen.DpiAvg > 110)
                    {
                        if (_currentScreen.DpiAvg > 138)
                            Mouse.setCursorAero(3);
                        else Mouse.setCursorAero(2);
                    }
                    else Mouse.setCursorAero(1);
                }

                if (AdjustSpeed)
                {
                    Mouse.MouseSpeed = Math.Round((5.0 / 96.0) * _currentScreen.DpiAvg, 0);
                }

                _oldPoint = pIn;
            }
            else
            {
                double x = pIn.X;
                double y = pIn.Y;

                x = Math.Max(x, _currentScreen.InsideBounds.Left);
                x = Math.Min(x, _currentScreen.InsideBounds.Right);
                y = Math.Max(y, _currentScreen.InsideBounds.Top);
                y = Math.Min(y, _currentScreen.InsideBounds.Bottom);

                Mouse.SetCursorPos((int)x,(int)y);

                _oldPoint = new Point(x, y);
            }

            e.Handled = true;
        }


        public List<Screen> AllScreens { get { return _allScreens; } }

        private static String RootKey = "SOFTWARE\\" + System.Windows.Forms.Application.CompanyName + "\\" + Application.ResourceAssembly.GetName().Name;

        public static ScreenConfig Load(RegistryKey baseKey)
        {
            RegistryKey key = baseKey.OpenSubKey(RootKey);
            if (key==null)
            {
                key = baseKey.CreateSubKey(RootKey);
            }

            ScreenConfig config = new ScreenConfig(key)
            {
                Enabled = (key.GetValue("Enabled","0").ToString() == "1"),
                AdjustPointer = (key.GetValue("AdjustPointer", "0").ToString() == "1"),
                AdjustSpeed = (key.GetValue("AdjustSpeed", "0").ToString() == "1"),
            };

            foreach (System.Windows.Forms.Screen screen in System.Windows.Forms.Screen.AllScreens)
            {
                config.getScreen(screen);
            }
            return config;
        }

        public void Save(RegistryKey baseKey)
        {
            RegistryKey key = baseKey.CreateSubKey(RootKey);
            key.SetValue("Enabled", Enabled ? "1" : "0");
            key.SetValue("AdjustPointer", AdjustPointer ? "1" : "0");
            key.SetValue("AdjustSpeed", AdjustSpeed ? "1" : "0");

            foreach (Screen s in AllScreens)
                s.Save(key);

            if (RegistryChanged != null) RegistryChanged(this, new EventArgs());
        }

        private Screen getScreen(System.Windows.Forms.Screen screen)
        {
            Screen wpfScreen = null;
            foreach (Screen s in AllScreens)
            {
                if (s._screen.DeviceName == screen.DeviceName) { wpfScreen = s; break; }
            }
            if (wpfScreen == null)
            {
                wpfScreen = new Screen(this,screen);
                wpfScreen.Load(_key);
                AllScreens.Add(wpfScreen);
            }
            return wpfScreen;
        }

        public Screen getScreen(int nb)
        {
            foreach (Screen s in AllScreens)
            {
                if (s.DeviceName.EndsWith(nb.ToString())) return s;
            }
            return null;
        }
        public Screen FromPoint(Point point)
        {
            int x = (int)Math.Round(point.X);
            int y = (int)Math.Round(point.Y);

            // are x,y device-independent-pixels ??
            System.Drawing.Point drawingPoint = new System.Drawing.Point(x, y);
            System.Windows.Forms.Screen screen = System.Windows.Forms.Screen.FromPoint(drawingPoint);
            Screen wpfScreen = getScreen(screen);

            return wpfScreen;
        }
        public Screen PrimaryScreen
        {
            get { return getScreen(System.Windows.Forms.Screen.PrimaryScreen); }
        }

// Original windows locations
        public Rect OverallBounds
        {
            get
            {
                Rect r = PrimaryScreen.Bounds;
                foreach (Screen s in AllScreens)
                {
                    r.Union(s.Bounds);
                }
                return r;
            }
        }

        // Physical Locations
        public Rect PhysicalOverallBounds
        {
            get
            {
                Rect r = new Rect();
                foreach (Screen s in AllScreens)
                {
                    if (r.Width == 0)
                        r = s.PhysicalBounds;
                    else
                        r.Union(s.PhysicalBounds);
                }
                return r;
            }
        }

        public bool Enabled
        {
            get { return _enabled; }
            set { _enabled = value; }
        }

        public bool AdjustPointer
        {
            get { return _adjustPointer; }
            set { _adjustPointer = value; }
        }

        public bool AdjustSpeed
        {
            get { return _adjustSpeed; }
            set { _adjustSpeed = value; }
        }

        public Point PhysicalToUI(Size s, Point p)
        {
            Rect all = PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Point(
                (p.X - all.Left) * ratio,
                (p.Y - all.Top) * ratio
                );
        }

        public Point FromUI(Size s, Point p)
        {
            Rect all = PhysicalOverallBounds;

            double ratio = Math.Min(
                s.Width / all.Width,
                s.Height / all.Height
                );

            return new Point(
                (p.X / ratio) + all.Left,
                (p.Y / ratio) + all.Top
                );
        }
        public Screen FromPhysicalPoint(Point p)
        {
            foreach(Screen s in AllScreens)
            {
                if (s.PhysicalBounds.Contains(p))
                    return s;
            }
            return null;
        }

    }
}
