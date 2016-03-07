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
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Management;
using System.Net.WebSockets;
using System.Runtime.InteropServices;
using System.Runtime.Remoting.Messaging;
using System.Windows;
using System.Windows.Documents;
using WindowsMonitors;
using NotifyChange;
// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LbmScreenConfig
{
    public class ScreenConfig : Notifier
    {
        public static RegistryKey OpenRootRegKey(bool create = false)
        {
            using (RegistryKey key = Registry.CurrentUser)
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(RootKey) : key.OpenSubKey(RootKey);
            }
        }

        /// <returns>a list of string representing each known config in registry</returns>
        /// <summary>
        /// 
        /// </summary>
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

        private static ObservableCollection<DisplayMonitor> Monitors => DisplayDevice.AttachedMonitors;
        public ScreenConfig()
        {
            MonitorsOnCollectionChanged(Monitors,new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add,Monitors));
            Monitors.CollectionChanged += MonitorsOnCollectionChanged;

            Watch(AllScreens, "Screen");
        }

        private void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
        {
            if (args.NewItems != null)
                foreach (DisplayMonitor monitor in args.NewItems)
                {
                    Screen screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));
                    if (screen == null)
                    {
                        screen = new Screen(this, monitor);
                        AllScreens.Add(screen);
                    }
                }
            if (args.OldItems != null)
                foreach (DisplayMonitor monitor in args.OldItems)
                {
                    Screen screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));

                    if (screen != null) AllScreens.Remove(screen);
                }

            Load();
        }

        public ObservableCollection<Screen> AllScreens { get; } = new ObservableCollection<Screen>();

        public IEnumerable<Screen> AllBut(Screen screen) => AllScreens.Where(s => s != screen);

        public Screen Selected
        {
            get { return _selected; }
            private set { SetProperty(ref _selected, value); }
        }

        [DependsOn("Screen.Selected")]
        public void UpdateSelected()
        {
            Selected  = AllScreens.FirstOrDefault ( screen => screen.Selected );
        }

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

            if (create) Directory.CreateDirectory(path);

            return path;
        }

        public string ConfigPath(bool create) => ConfigPath(Id, create);

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Id, create);

        public string Id
            =>
                AllScreens.OrderBy(s => s.Id)
                    .Aggregate("", (current, screen) => current + (((current != "") ? "." : "") + screen.Id));


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
                            AttachToDesktop(id, screen.IdMonitor, false);
                            todo.Remove(screen.IdMonitor);
                        }
                        else
                        {
                            screen.Monitor.DetachFromDesktop(false);
                        }
                    }

                    foreach (string s in todo)
                    {
                        AttachToDesktop(id, s, false);
                    }

                    DisplayMonitor.ApplyDesktop();
                }
            }
        }

        public static bool IsDoableConfig(String id)
        {
            using (RegistryKey rootkey = OpenRootRegKey())
            {
                using (RegistryKey key = rootkey.OpenSubKey(@"configs\" + id))
                {
                    List<string> todo = key.GetSubKeyNames().ToList();

                    // ReSharper disable once LoopCanBeConvertedToQuery
                    foreach (string s in todo)
                    {
                        //string s2 = s.Substring(0, s.Length - 2);
                        DisplayMonitor m = DisplayDevice.AllMonitors.FirstOrDefault(
                            d => s == d.ManufacturerCode + d.ProductCode + "_" + d.Serial);

                        if (m == null) return false;
                    }
                }
            }
            return true;
        }
        public static void AttachToDesktop(string configId, string monitorId, bool apply=true)
        {
            //using (RegistryKey monkey = Screen.OpenMonitorRegKey(monitorId))
            //{
            //    id = monkey?.GetValue("DeviceId").ToString();
            //    if (id == null) return;
            //}
            Rect area = new Rect();
            bool primary = false;
            int orientation = 0;

            using (RegistryKey monkey = Screen.OpenConfigRegKey(configId, monitorId))
            {
                area.X = double.Parse(monkey.GetValue("PixelX").ToString());
                area.Y = double.Parse(monkey.GetValue("PixelY").ToString());
                area.Width = double.Parse(monkey.GetValue("PixelWidth").ToString());
                area.Height = double.Parse(monkey.GetValue("PixelHeight").ToString());

                primary = double.Parse(monkey.GetValue("Primary").ToString()) == 1;
                orientation = (int)double.Parse(monkey.GetValue("Orientation").ToString());
            }

            DisplayMonitor monitor = DisplayDevice.AllMonitors.FirstOrDefault(
                                        d => monitorId == d.ManufacturerCode + d.ProductCode + "_" + d.Serial);

            monitor?.AttachToDesktop(primary, area, orientation, apply);
        }

        public void EnumWMI()
        {
            string NamespacePath = "\\\\.\\ROOT\\WMI\\ms_409";
            string ClassName = "WmiMonitorID";

            //Create ManagementClass
            ManagementClass oClass = new ManagementClass(NamespacePath + ":" + ClassName);

            //Get all instances of the class and enumerate them
            foreach (ManagementObject oObject in oClass.GetInstances())
            {
                //access a property of the Management object
                Console.WriteLine("ManufacturerName : {0}", oObject["ManufacturerName"]);
            }
        }



        public void Load()
        {
            Moving = true; //TODO : Hugly hack
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
            Saved = true;
            Moving = false;
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

                    Saved = true;
                    return true;
                }
                return false;
            }
        }

        public Screen PrimaryScreen => AllScreens.FirstOrDefault(s => s.Primary);

        /// <summary>
        /// Moving is true when screen is dragged on gui
        /// </summary>
        private bool _moving = false;

        public bool Moving
        {
            get { return _moving; }
            set { SetProperty(ref _moving, value); }
        }

        /// <summary>
        /// Physical Outside Bounds updated while moving (screen dragged on gui)
        /// </summary>
        Rect _physicalOutsideBounds = new Rect();

        public Rect PhysicalOutsideBounds
        {
            get { return _physicalOutsideBounds; }
            private set { if (SetProperty(ref _physicalOutsideBounds, value)) Saved = false; }
        }

        [DependsOn(nameof(Moving),"Screen.PhysicalOutsideBounds")]
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

            PhysicalOutsideBounds = outside;
        }

        private Rect _movingPhysicalOutsideBounds;


        /// <summary>
        /// Physical Outside Bounds NOT updated while moving (screen dragged on gui)
        /// </summary>
        public Rect MovingPhysicalOutsideBounds
        {
            get { return _movingPhysicalOutsideBounds; }
            private set { SetProperty(ref _movingPhysicalOutsideBounds, value); }
        }

        public void ShiftMovingPhysicalBounds(Vector shift)
        {
            Rect r = new Rect(
                _movingPhysicalOutsideBounds.TopLeft + shift
                , _movingPhysicalOutsideBounds.Size
                );
            MovingPhysicalOutsideBounds = r;
        }

        [DependsOn(nameof(Moving), nameof(PhysicalOutsideBounds))]
        private void UpdateMovingPhysicalOutsideBounds()
        {
            if (Moving) return;
            MovingPhysicalOutsideBounds = PhysicalOutsideBounds;
        }


        /// <summary>
        /// Physical Bounds of overall screens without borders
        /// </summary>
        private Rect _physicalBounds = new Rect();

        public Rect PhysicalBounds
        {
            get { return _physicalBounds; }
            private set { if (SetProperty(ref _physicalBounds, value)) Saved = false; }
        }

        [DependsOn("Screen.PhysicalBounds")]
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

            PhysicalBounds = inside;
        }

        /// <summary>
        /// 
        /// </summary>
        private bool _enabled;

        public bool Enabled
        {
            get { return _enabled; }
            set { if (SetProperty(ref _enabled, value)) Saved = false; }
        }

        private bool _loadAtStartup;

        public bool LoadAtStartup
        {
            get { return _loadAtStartup; }
            set { if (SetProperty(ref _loadAtStartup, value)) Saved = false; }
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
            set { if (SetProperty(ref _adjustPointer, value)) Saved = false; }
        }

        public bool AdjustSpeedAllowed => IsRatio100;
        private bool _adjustSpeed;

        public bool AdjustSpeed
        {
            get { return AdjustSpeedAllowed && _adjustSpeed; }
            set { if (SetProperty(ref _adjustSpeed, value)) Saved = false; }
        }

        private bool _allowCornerCrossing;

        public bool AllowCornerCrossing
        {
            get { return _allowCornerCrossing; }
            set { if (SetProperty(ref _allowCornerCrossing, value)) Saved = false; }
        }


        private bool _homeCinema;

        public bool HomeCinema
        {
            get { return _homeCinema; }
            set { if (SetProperty(ref _homeCinema, value)) Saved = false; }
        }

        private Rect _configLocation;

        public Rect ConfigLocation
        {
            get { return _configLocation; }
            set { SetProperty(ref _configLocation, value); }
        }


        public void SetPhysicalAuto()
        {
            if (PrimaryScreen == null) return;

            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }
            // List all screens not positioned
            List<Screen> unatachedScreens = AllScreens.ToList();

            // start with primary screen
            Queue<Screen> todo = new Queue<Screen>();
            todo.Enqueue(PrimaryScreen);

            while (todo.Count > 0)
            {
                foreach (Screen s2 in todo)
                {
                    unatachedScreens.Remove(s2);
                }

                Screen placedScreen = todo.Dequeue();

                foreach (Screen screenToPlace in unatachedScreens)
                {
                    if (screenToPlace == placedScreen) continue;

                    bool done = false;

                    //     __
                    //  __| A
                    // B  |__
                    //  __|
                    if (screenToPlace.PixelBounds.X == placedScreen.PixelBounds.Right)
                    {
                        screenToPlace.PhysicalX = placedScreen.PhysicalOutsideBounds.Right + screenToPlace.LeftBorder;
                        done = true;
                    }
                    //B |___|_
                    //A  |    |
                    if (screenToPlace.PixelBounds.Y == placedScreen.PixelBounds.Bottom)
                    {
                        screenToPlace.PhysicalY = placedScreen.PhysicalOutsideBounds.Bottom + screenToPlace.TopBorder;
                        done = true;
                    }

                    //     __
                    //  __| B
                    // A  |__
                    //  __|
                    if (screenToPlace.PixelBounds.Right == placedScreen.PixelBounds.X)
                    {
                        screenToPlace.PhysicalX = placedScreen.PhysicalOutsideBounds.Left -
                                                  screenToPlace.PhysicalOutsideBounds.Width + screenToPlace.LeftBorder;
                        done = true;
                    }

                    //A |___|_
                    //B  |    |

                    if (screenToPlace.PixelBounds.Bottom == placedScreen.PixelLocation.Y)
                    {
                        screenToPlace.PhysicalY = placedScreen.PhysicalOutsideBounds.Top -
                                                  screenToPlace.PhysicalOutsideBounds.Height + screenToPlace.TopBorder;
                        done = true;
                    }


                    //  __
                    // |
                    // |__
                    //  __
                    // |
                    // |__
                    if (screenToPlace.PixelBounds.X == placedScreen.PixelBounds.X)
                    {
                        screenToPlace.PhysicalX = placedScreen.PhysicalX;
                        done = true;
                    }

                    //  ___   ___
                    // |   | |   |
                    if (screenToPlace.PixelBounds.Y == placedScreen.PixelBounds.Y)
                    {
                        screenToPlace.PhysicalY = placedScreen.PhysicalY;
                        done = true;
                    }

                    // __
                    //   |
                    // __|
                    // __
                    //   |
                    // __|
                    if (screenToPlace.PixelBounds.Right == placedScreen.PixelBounds.Right)
                    {
                        screenToPlace.PhysicalX = placedScreen.PhysicalBounds.Right - screenToPlace.PhysicalBounds.Width;
                        done = true;
                    }

                    //|___||___|
                    if (screenToPlace.PixelBounds.Bottom == placedScreen.PixelBounds.Bottom)
                    {
                        screenToPlace.PhysicalY = placedScreen.PhysicalBounds.Bottom -
                                                  screenToPlace.PhysicalBounds.Height;
                        done = true;
                    }
                    if (done)
                    {
                        todo.Enqueue(screenToPlace);
                    }
                }

                lock (_compactLock)
                {
                    _compacting = false;
                }
            }
        }

        private readonly object _compactLock = new object();
        private bool _compacting = false;

        public void Compact()
        {
            if (PrimaryScreen == null) return;


            if (Moving) return;
            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }

            List<Screen> done = new List<Screen> { PrimaryScreen };

            List<Screen> todo = AllBut(PrimaryScreen).OrderBy(s => s.Distance(PrimaryScreen)).ToList();

            while (todo.Count > 0)
            {
                Screen screen = todo[0];
                todo.Remove(screen);

                screen.PlaceAuto(done);
                done.Add(screen);

                todo = todo.OrderBy(s => s.Distance(done)).ToList();
            }

            lock (_compactLock)
            {
                _compacting = false;
            }
        }


        private bool _allowOverlaps = false;

        public bool AllowOverlaps
        {
            get { return _allowOverlaps; }
            set
            {
                if (SetProperty(ref _allowOverlaps, value))
                {
                    Saved = false;
                }
            }
        }

        private bool _allowDiscontinuity = false;
        private bool _saved = false;
        private Screen _selected;

        public bool AllowDiscontinuity
        {
            get { return _allowDiscontinuity; }
            set { if (SetProperty(ref _allowDiscontinuity, value)) Saved = false; }
        }

        public bool Saved
        {
            get { return _saved; }
            set { SetProperty(ref _saved, value); }
        }
    }
}
