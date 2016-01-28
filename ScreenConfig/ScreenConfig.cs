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
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using NotifyChange;
using WinAPI_User32;
// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LbmScreenConfig
{
    public class ScreenConfig : INotifyPropertyChanged
    {
        public static RegistryKey OpenRootRegKey(bool create = false)
        {
            using (RegistryKey key = Registry.CurrentUser)
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(RootKey) : key.OpenSubKey(RootKey);
            }
        }

        public static IEnumerable<string> ConfigsList
        {
            get
            {
                using (RegistryKey rootkey = OpenRootRegKey())
                {
                    using (RegistryKey key = rootkey.OpenSubKey("configs"))
                    {
                        return key?.GetSubKeyNames();
                    }
                }
            }
        }
        // PropertyChanged Handling
        private readonly PropertyChangedHelper _change;
        public event PropertyChangedEventHandler PropertyChanged { add { _change.Add(this, value); } remove { _change.Remove(value); } }

        public ScreenConfig()
        {
            _change = new PropertyChangedHelper(this);
            AllScreens = new List<Screen>();
            Load();
        }

        public List<Screen> AllScreens { get; }

        public IEnumerable<Screen> AllBut(Screen screen) => AllScreens.Where(s => s != screen);

        public Screen Selected => AllScreens.FirstOrDefault(screen => screen.Selected);

        private static readonly string RootKey = @"SOFTWARE\Mgth\LittleBigMouse";

        internal static RegistryKey OpenConfigRegKey(string configId, bool create)
        {
            using (RegistryKey key = OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(@"configs\" + configId) : key.OpenSubKey(@"configs\" + configId);
            }
        }

        internal static string ConfigPath(string configId, bool create)
        {
            string path = Path.Combine(Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse", configId);

            if (create) System.IO.Directory.CreateDirectory(path);

            return path;
        }

        public string ConfigPath(bool create) => ConfigPath(Id, create);

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Id,create);

        public string Id => AllScreens.OrderBy(s => s.Id).Aggregate("", (current, screen) => current + (((current != "") ? "." : "") + screen.Id));

        public static void EnumDisplays()
        {
            DISPLAY_DEVICE ddDev = new DISPLAY_DEVICE(true);
            uint devIdx = 0;

            while (User32.EnumDisplayDevices(null, devIdx, ref ddDev, 0))
            {
                    DISPLAY_DEVICE ddMon = new DISPLAY_DEVICE();
                    ddMon.cb = Marshal.SizeOf(ddMon);
                    uint monIdx = 0;
                    while (User32.EnumDisplayDevices(ddDev.DeviceName, monIdx,ref ddMon, 0))
                    {
                        DEVMODE devmode = new DEVMODE();
                        devmode.Size = (short)Marshal.SizeOf(devmode);

                        bool result = User32.EnumDisplaySettings(ddDev.DeviceName, 0, ref devmode) ;
                        monIdx++;
                    }
                    devIdx++;
            }
        }

        public void MatchConfig(string id)
        {
            using (RegistryKey rootkey = OpenRootRegKey())
            {
                using (RegistryKey key = rootkey.OpenSubKey(@"configs\" + id))
                {
                    List<string> todo = key.GetSubKeyNames().ToList();

                    foreach (Screen screen in AllScreens)
                    {
                        if (todo.Contains(screen.IdMonitor))
                        {
                            todo.Remove(screen.IdMonitor);
                        }
                        else
                        {
                            screen.DetachFromDesktop();
                        }
                    }

                    foreach (string s in todo)
                    {
                        Screen.AttachToDesktop(id, s);
                    }
                }
            }
        }

        public void Load()
        {
            EnumDisplays();

            User32.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData)
                {
                    GetScreen(hMonitor);
                    return true;
                }, IntPtr.Zero);

            SetPhysicalAuto();

            using (RegistryKey k = OpenConfigRegKey())
            {
                if (k != null)
                {
                    Enabled = k.GetValue("Enabled", 0).ToString() == "1";
                    AdjustPointer = k.GetValue("AdjustPointer", 0).ToString() == "1";
                    AdjustSpeed = k.GetValue("AdjustSpeed", 0).ToString() == "1";
                    AllowCornerCrossing = k.GetValue("AllowCornerCrossing", 0).ToString() == "1";
                    AllowOverlaps = k.GetValue("AllowOverlaps", 0).ToString() == "1";
                    AllowDiscontinuity = k.GetValue("AllowDiscontinuity", 0).ToString() == "1";
                    LoadAtStartup = k.GetValue("LoadAtStartup", 0).ToString() == "1";
                    HomeCinema = k.GetValue("HomeCinema", 0).ToString() == "1";
                }
            }

            foreach (Screen s in AllScreens)
            {
                s.Load();
            }
        }

        public bool Save()
        {
            using (RegistryKey k = OpenConfigRegKey(true))
            {
                if (k != null)
                {
                    k.SetValue("Enabled", Enabled ? "1" : "0");
                    k.SetValue("AdjustPointer", AdjustPointer ? "1" : "0");
                    k.SetValue("AdjustSpeed", AdjustSpeed ? "1" : "0");
                    k.SetValue("AllowCornerCrossing", AllowCornerCrossing ? "1" : "0");
                    k.SetValue("AllowOverlaps", AllowOverlaps ? "1" : "0");
                    k.SetValue("AllowDiscontinuity", AllowDiscontinuity ? "1" : "0");
                    k.SetValue("LoadAtStartup", LoadAtStartup ? "1" : "0");
                    k.SetValue("HomeCinema", HomeCinema ? "1" : "0");

                    foreach (Screen s in AllScreens)
                        s.Save(k);
                    return true;
                }
                return false;
            }
        }

        private Screen GetScreen(IntPtr hMonitor)
        {
            foreach (Screen s in AllScreens.Where(s => s.HMonitor == hMonitor))
            {
                return s;
            }

            {
                Screen s = new Screen(this, hMonitor);
                AllScreens.Add(s);
                s.PropertyChanged += Screen_PropertyChanged;
                UpdatePhysicalOutsideBounds();
                return s;
            }
        }


        private void Screen_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            switch (e.PropertyName)
            {
                case "PhysicalBounds":
                    UpdatePhysicalBounds();
                    break;
                case "PhysicalOutsideBounds":
                    UpdatePhysicalOutsideBounds();
                    break;
            }
        }


        public Screen PrimaryScreen => AllScreens.FirstOrDefault(s => s.Primary);

        private bool _moving = false;
        public bool Moving
        {
            get { return _moving; }
            set { _change.SetProperty(ref _moving, value); }
        }


        // Physical Locations
        private Rect _physicalBounds = new Rect();
        public Rect PhysicalBounds => _physicalBounds;

        Rect _physicalOutsideBounds = new Rect();
        public Rect PhysicalOutsideBounds => _physicalOutsideBounds;

        public void UpdatePhysicalOutsideBounds()
        {
            Rect outside = new Rect();

            bool first = true;
            foreach (Screen s in AllScreens)
            {
                if (first)
                {
                    outside = s.PhysicalOutsideBounds;
                    first = false;
                    continue;
                }

                outside.Union(s.PhysicalOutsideBounds);
            }

            _change.SetProperty(ref _physicalOutsideBounds, outside, "PhysicalOutsideBounds");
        }

        private Rect _movingPhysicalOutsideBounds;
        public Rect MovingPhysicalOutsideBounds => _movingPhysicalOutsideBounds;
        public void ShiftMovingPhysicalBounds(Vector shift)
        {
            Rect r = new Rect(
                    _movingPhysicalOutsideBounds.TopLeft + shift
                    , _movingPhysicalOutsideBounds.Size
                    );
            _change.SetProperty(ref _movingPhysicalOutsideBounds, r, "MovingPhysicalOutsideBounds");
        }

        [DependsOn("Moving", "PhysicalOutsideBounds")]
        public void UpdateMovingPhysicalOutsideBounds()
        {
            if (Moving) return;
            _change.SetProperty(ref _movingPhysicalOutsideBounds, PhysicalOutsideBounds, "MovingPhysicalOutsideBounds");
        }



        public void UpdatePhysicalBounds()
        {
            Rect inside = new Rect();

            bool first = true;
            foreach (Screen s in AllScreens)
            {
                if (first)
                {
                    inside = s.PhysicalBounds;
                    first = false;
                    continue;
                }

                inside.Union(s.PhysicalBounds);
            }

            _change.SetProperty(ref _physicalBounds, inside, "PhysicalBounds");
        }

        private bool _enabled;
        public bool Enabled {
            get { return _enabled; }
            set { _change.SetProperty(ref _enabled, value); }
        }

        private bool _loadAtStartup;
        public bool LoadAtStartup
        {
            get { return _loadAtStartup;  }
            set { _change.SetProperty(ref _loadAtStartup, value); }
        }

        public bool IsRatio100
        {
            get
            {
                foreach (Screen screen in AllScreens)
                {
                    if (screen.PixelToWpfRatioX != 1) return false;
                    if (screen.PixelToWpfRatioY != 1) return false;
                }
                return true;
            }
        }

        public bool AdjustPointerAllowed => IsRatio100; 
        private bool _adjustPointer;
        public bool AdjustPointer
        {
            get { return AdjustPointerAllowed && _adjustPointer; }
            set { _change.SetProperty(ref _adjustPointer, value); }
        }

        public bool AdjustSpeedAllowed => IsRatio100;
        private bool _adjustSpeed;
        public bool AdjustSpeed
        {
            get { return AdjustSpeedAllowed && _adjustSpeed; }
            set {
                _change.SetProperty(ref _adjustSpeed, value);
            }
        }

        private bool _allowCornerCrossing;
        public bool AllowCornerCrossing
        {
            get { return _allowCornerCrossing; }
            set { _change.SetProperty(ref _allowCornerCrossing, value); }
        }

        private bool _homeCinema;
        public bool HomeCinema
        {
            get { return _homeCinema; }
            set { _change.SetProperty(ref _homeCinema, value); }
        }

        private Rect _configLocation;
        public Rect ConfigLocation
        {
            get { return _configLocation; }
            set { _change.SetProperty(ref _configLocation, value); }
        }


        public void SetPhysicalAuto()
        {

            List<Screen> unatachedScreens = AllScreens.ToList();

            List<Screen> todo = new List<Screen> {PrimaryScreen};

            while (todo.Count > 0)
            {
                foreach (Screen s2 in todo)
                {
                    unatachedScreens.Remove(s2);
                }

                Screen s = todo[0];
                todo.Remove(s);
                foreach (Screen s1 in unatachedScreens)
                {
                    if (s1 == s) continue;

                    bool done = false;
                    if (s1.PixelBounds.X == s.PixelBounds.Right)
                    {
                        s1.PhysicalX = s.PhysicalOutsideBounds.Right + s1.LeftBorder;
                        done = true;
                    }
                    if (s1.PixelBounds.Y == s.PixelBounds.Bottom)
                    {
                        s1.PhysicalY = s.PhysicalOutsideBounds.Bottom + s1.TopBorder;
                        done = true;
                    }

                    if (s1.PixelBounds.Right == s.PixelBounds.X)
                    {
                        s1.PhysicalX = s.PhysicalOutsideBounds.Left - s1.PhysicalOutsideBounds.Width + s1.LeftBorder;
                        done = true;
                    }
                    if (s1.PixelBounds.Bottom == s.PixelLocation.Y)
                    {
                        s1.PhysicalY = s.PhysicalOutsideBounds.Top - s1.PhysicalOutsideBounds.Height + s1.TopBorder;
                        done = true;
                    }

                    if (s1.PixelBounds.X == s.PixelBounds.X)
                    {
                        s1.PhysicalX = s.PhysicalX;
                        done = true;
                    }
                    if (s1.PixelBounds.Y == s.PixelBounds.Y)
                    {
                        s1.PhysicalY = s.PhysicalY;
                        done = true;
                    }

                    if (s1.PixelBounds.Right == s.PixelBounds.Right)
                    {
                        s1.PhysicalX = s.PhysicalBounds.Right - s1.PhysicalBounds.Width;
                        done = true;
                    }
                    if (s1.PixelBounds.Bottom == s.PixelBounds.Bottom)
                    {
                        s1.PhysicalY = s.PhysicalBounds.Bottom - s1.PhysicalBounds.Height;
                        done = true;
                    }
                    if (done)
                    {
                        todo.Add(s1);
                    }
                }

            }

        }


        public void Compact()
        {
            CompactX();
            CompactY();
        }

        public void CompactX()
        {
            List<Screen> todo = AllScreens.ToList();

            double right = todo.Select(s => s.PhysicalOutsideBounds.X).Min();


            while (todo.Count > 0)
            {
                Screen leftScreen = null;

                foreach (Screen s in todo.Where(s => leftScreen==null || s.PhysicalOutsideBounds.X < leftScreen.PhysicalOutsideBounds.X))
                {
                    leftScreen = s;
                }
                leftScreen.PhysicalX = right + leftScreen.LeftBorder;


                right = leftScreen.PhysicalOutsideBounds.Right;
                List<Screen> doneList = new List<Screen>() {leftScreen};

                while (doneList.Count > 0)
                {
                    todo = todo.Except(doneList).ToList(); doneList.Clear();

                    foreach (Screen s in todo.Where(s => s.PhysicalOutsideBounds.X <= right))
                    {
                        if (right<s.PhysicalOutsideBounds.Right) right = s.PhysicalOutsideBounds.Right;
                        doneList.Add(s);
                    }               
                }               
            }

        }
        public void CompactY()
        {
            List<Screen> todo = AllScreens.ToList();

            double bottom = todo.Select(s => s.PhysicalOutsideBounds.Y).Min();


            while (todo.Count > 0)
            {
                Screen topScreen = null;

                foreach (Screen s in todo.Where(s => topScreen == null || s.PhysicalOutsideBounds.Y < topScreen.PhysicalOutsideBounds.Y))
                {
                    topScreen = s;
                }
                topScreen.PhysicalY = bottom + topScreen.TopBorder;


                bottom = topScreen.PhysicalOutsideBounds.Bottom;
                List<Screen> doneList = new List<Screen>() { topScreen };

                while (doneList.Count > 0)
                {
                    todo = todo.Except(doneList).ToList(); doneList.Clear();

                    foreach (Screen s in todo.Where(s => s.PhysicalOutsideBounds.Y <= bottom))
                    {
                        if (bottom < s.PhysicalOutsideBounds.Bottom) bottom = s.PhysicalOutsideBounds.Bottom;
                        doneList.Add(s);
                    }
                }
            }

        }

        public void Expand()
        {
            bool done = false;
            int i = 100; // hack to avoid infinit loop
            while (!done)
            {
                done = true;
                foreach (Screen screen in AllScreens.Where(screen => screen.Expand()))
                {                 
                    i--;
                    if (i>0) done = false;
                }
            }
        }

        private bool _allowOverlaps = false;
        public bool AllowOverlaps
        {
            get { return _allowOverlaps; }
            set { _change.SetProperty(ref _allowOverlaps, value); }
        }

        private bool _allowDiscontinuity = false;
        public bool AllowDiscontinuity
        {
            get { return _allowDiscontinuity; }
            set { _change.SetProperty(ref _allowDiscontinuity, value); }
        }
    }

    public class ScreenList : List<Screen>
    {
        public List<ScreenList> SplitBlocs()
        {
            List<ScreenList> result = new List<ScreenList>();
            List<Screen> leftScreens = this.ToList();

            while (leftScreens.Count > 0)
            {
                Screen s = leftScreens[0];
                ScreenList list = new ScreenList {s};

                bool done = true;

                while (done)
                {
                    leftScreens = leftScreens.Except(list).ToList();

                    done = false;
                    foreach (Screen screen in leftScreens.Where(screen => !list.Overlap(s) && list.Touch(s)))
                    {
                        list.Add(screen);
                        done = true;
                    }             
                }

                result.Add(list);             
            }

            return result;
        }

        public bool Overlap(Screen screen)
        {
            return this.Any(screen.PhysicalOverlapWith);
        }

        public bool Touch(Screen screen)
        {
            return this.Any(screen.PhysicalTouch);
        }

        void Compact()
        {
            List<ScreenList> lists = SplitBlocs();

            while (lists.Count > 1)
            {
                
            }
        }

    }
}
