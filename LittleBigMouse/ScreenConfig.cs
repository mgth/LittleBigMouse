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

using LittleBigMouseGeo;
using Microsoft.Win32;
using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Windows;

namespace LittleBigMouse
{
    public class ScreenConfig : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }
        public ScreenConfig()
        {
            Load();
        }

        public event EventHandler RegistryChanged;

        private List<Screen> _allScreens = new List<Screen>();
        private readonly MouseHookListener _MouseHookManager = new MouseHookListener(new GlobalHooker());

        private Screen _currentScreen=null;
        private Point _oldPoint;
        private bool _enabled;
        private bool _loadAtStartup;
        private bool _adjustPointer;
        private bool _adjustSpeed;
        private Rect _configLocation;
        private bool _allowToJump;

        public void Start()
        {
            if (Enabled)
            {
                _MouseHookManager.MouseMoveExt += _MouseHookManager_MouseMoveExt;
                _MouseHookManager.Enabled = true;
            }
        }
        public void Stop()
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
                //_currentScreen.AlignScreens(pIn);
                _oldPoint = pIn;
                return;
            }

            Point pOutPhysical = _currentScreen.PixelToPhysical(pIn);

            Screen screenOut = FromPhysicalPoint(new Point(pOutPhysical.X+0.5,pOutPhysical.Y+0.5));

//
// Allow To Jump
//
            if (screenOut==null && AllowToJump)
            {
                double dist = 100.0; // double.PositiveInfinity;
                Segment seg = new Segment(_currentScreen.PixelToPhysical(_oldPoint), _currentScreen.PixelToPhysical(pIn));
                foreach (Screen s in AllScreens)
                {
                    if (s!=_currentScreen)
                    {
                        foreach (Point p in seg.Line.Intersect(s.PhysicalBounds))
                        {
                            Segment travel = new Segment(_currentScreen.PixelToPhysical(_oldPoint), p);
                            if (travel.Rect.Contains(_currentScreen.PixelToPhysical(pIn)))
                            {
                                if (travel.Size < dist)
                                {
                                    dist = travel.Size;
                                    pOutPhysical = p;
                                    screenOut = s;
                                }
                            }
                        }
                    }
                }
            }

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

        public RegistryKey OpenRegKey()
        {
                RegistryKey k = Registry.CurrentUser.CreateSubKey(RootKey);
                return k.CreateSubKey(_id);
        }

        private String _id = "";
        public void Load()
        {
            _id = "";

            foreach (System.Windows.Forms.Screen screen in System.Windows.Forms.Screen.AllScreens)
            {
                Screen s = getScreen(screen);
                _id += ((_id!="")?"." :"") + s.ID;
            }

            LoadAtStartup = App.Scheduled;

            using (RegistryKey k = OpenRegKey())
            {
                Enabled = k.GetValue("Enabled", 0).ToString() == "1";
                AdjustPointer = k.GetValue("AdjustPointer", 0).ToString() == "1";
                AdjustSpeed = k.GetValue("AdjustSpeed", 0).ToString() == "1";
                AllowToJump = k.GetValue("AllowToJump", 0).ToString() == "1";
                foreach(Screen s in AllScreens)
                {
                    s.Load(k);
                }

                k.Close();
            }


        }

        public void Save()
        {
            if (LoadAtStartup)
                App.Schedule();
            else
                App.Unschedule();

            using (RegistryKey k = OpenRegKey())
            {
                k.SetValue("Enabled", Enabled ? "1" : "0");
                k.SetValue("AdjustPointer", AdjustPointer ? "1" : "0");
                k.SetValue("AdjustSpeed", AdjustSpeed ? "1" : "0");
                k.SetValue("AllowToJump", AllowToJump ? "1" : "0");

                foreach (Screen s in AllScreens)
                    s.Save(k);

                k.Close();
            }

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
                AllScreens.Add(wpfScreen);
                changed("PhysicalBounds");
                changed("OverallPhysicalBounds");
                wpfScreen.PropertyChanged += Screen_PropertyChanged;
            }
            return wpfScreen;
        }

        private Rect _physicalOverallBounds = new Rect();
        private void Screen_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            switch(e.PropertyName)
            {
                case "PhysicalBounds":
                    Rect r = new Rect();
                    foreach (Screen s in AllScreens)
                    {
                        if (r.Width == 0)
                            r = s.PhysicalBounds;
                        else
                            r.Union(s.PhysicalBounds);
                    }
                    if (_physicalOverallBounds!=r)
                    {
                        _physicalOverallBounds = r;
                        changed("PhysicalOverallBounds");
                    }
                    break;
            }
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
                Rect r = new Rect();
                foreach (Screen s in AllScreens)
                {
                    if (r.Width == 0)
                        r = s.Bounds;
                    else
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
                return _physicalOverallBounds;
            }
        }

        public bool Enabled
        {
            get { return _enabled; }
            set {
                _enabled = value;
                changed("Enabled");
            }
        }

        public bool LoadAtStartup
        {
            get { return _loadAtStartup;  }
            set
            {
                _loadAtStartup = value;
                changed("LoadAtStartup");
            }
        }

        public bool AdjustPointer
        {
            get { return _adjustPointer; }
            set {
                _adjustPointer = value;
                changed("AdjustPointer");
            }
        }

        public bool AdjustSpeed
        {
            get { return _adjustSpeed; }
            set {
                _adjustSpeed = value;
                changed("AdjustSpeed");
            }
        }
        public bool AllowToJump
        {
            get { return _allowToJump; }
            set {
                _allowToJump = value;
                changed("AllowToJump");
            }
        }

        public Rect ConfigLocation
        {
            get { return _configLocation; }
            set {
                _configLocation = value;
                changed("ConfigLocation");
            }
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
