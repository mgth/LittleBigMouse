/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Management;
using System.Windows;
using HLab.Notify;
using HLab.Windows.Monitors;
using Microsoft.Win32;

// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenConfig : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }
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
                using (var rootkey = OpenRootRegKey())
                {
                    using (var key = rootkey.OpenSubKey("configs"))
                    {
                        return key?.GetSubKeyNames();
                    }
                }
            }
        }

        public ScreenConfigs.Screen ScreenFromPixel(Point pixel)
        {
            foreach (var screen in AllScreens)
            {
                if (screen.InPixel.Bounds.Contains(pixel)) return screen;
            }

            return null; 
        }
        public ScreenConfigs.Screen ScreenFromMmPosition(Point mm)
        {
            foreach (var screen in AllScreens)
            {
                if (screen.InMm.Bounds.Contains(mm)) return screen;
            }

            return null;
        }

        public ScreenConfig()
        {
            MonitorsOnCollectionChanged(MonitorsService.D.AttachedMonitors,
                new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, MonitorsService.D.AttachedMonitors));

            MonitorsService.D.AttachedMonitors.CollectionChanged += MonitorsOnCollectionChanged;
            this.Subscribe();

            SetPhysicalAuto(false);

        }

        private void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
        {
            if (args.NewItems != null)
                foreach (var monitor in args.NewItems.OfType<DisplayMonitor>())
                {
                    var screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));
                    if (screen != null) continue;

                    screen = new ScreenConfigs.Screen(this, monitor);
                    AllScreens.Add(screen);
                }
            if (args.OldItems != null)
                foreach (var monitor in args.OldItems.OfType<DisplayMonitor>())
                {
                    var screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));

                    if (screen != null) AllScreens.Remove(screen);
                }

            Load();
        }

        public ObservableCollection<ScreenConfigs.Screen> AllScreens => this.Get(()=>new ObservableCollection<ScreenConfigs.Screen>());

        public IEnumerable<ScreenConfigs.Screen> AllBut(ScreenConfigs.Screen screen) => AllScreens.Where(s => !Equals(s, screen));

        public ScreenConfigs.Screen Selected
        {
            get => this.Get<ScreenConfigs.Screen>();
            private set => this.Set(value);
        }

        [TriggedOn(nameof(AllScreens),"Item","Selected")]
        public void UpdateSelected()
        {
            Selected = AllScreens.FirstOrDefault(screen => screen.Selected);
        }

        private const string RootKey = @"SOFTWARE\Mgth\LittleBigMouse";

        internal static RegistryKey OpenConfigRegKey(string configId, bool create)
        {
            using (var key = OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(@"configs\" + configId) : key.OpenSubKey(@"configs\" + configId);
            }
        }

        internal static string ConfigPath(string configId, bool create)
        {
            var path = Path.Combine(Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse", configId);

            if (create) Directory.CreateDirectory(path);

            return path;
        }

        public string ConfigPath(bool create) => ConfigPath(Id, create);

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Id, create);


        [TriggedOn(nameof(AllScreens),"Item","Id")]
        public string Id
        {
            get => this.Get(IdDefault);
            private set => this.Set(value);
        }

        public string IdDefault() => AllScreens.OrderBy(s => s.Id)
                    .Aggregate("", (current, screen) => current + (((current != "") ? "." : "") + screen.Id));


        public void MatchConfig(string id)
        {
            using (var rootkey = OpenRootRegKey())
            {
                using (var key = rootkey.OpenSubKey(@"configs\" + id))
                {
                    var todo = key.GetSubKeyNames().ToList();

                    foreach (var screen in AllScreens)
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
                        DisplayMonitor m = MonitorsService.D.Monitors.FirstOrDefault(
                            d => s == d.Edid.ManufacturerCode + d.Edid.ProductCode + "_" + d.Edid.Serial);

                        if (m == null) return false;
                    }
                }
            }
            return true;
        }

        public static void AttachToDesktop(string configId, string monitorId, bool apply = true)
        {
            //using (RegistryKey monkey = Screen.OpenMonitorRegKey(monitorId))
            //{
            //    id = monkey?.GetValue("DeviceId").ToString();
            //    if (id == null) return;
            //}
            Rect area = new Rect();
            bool primary = false;
            int orientation = 0;

            using (RegistryKey monkey = ScreenConfigs.Screen.OpenConfigRegKey(configId, monitorId))
            {
                area.X = double.Parse(monkey.GetValue("PixelX").ToString());
                area.Y = double.Parse(monkey.GetValue("PixelY").ToString());
                area.Width = double.Parse(monkey.GetValue("PixelWidth").ToString());
                area.Height = double.Parse(monkey.GetValue("PixelHeight").ToString());

                primary = double.Parse(monkey.GetValue("Primary").ToString()) == 1;
                orientation = (int) double.Parse(monkey.GetValue("Orientation").ToString());
            }

            DisplayMonitor monitor = MonitorsService.D.Monitors.FirstOrDefault(
                d => monitorId == d.Edid.ManufacturerCode + d.Edid.ProductCode + "_" + d.Edid.Serial);

            monitor?.AttachToDesktop(primary, area, orientation, apply);
        }

        public void EnumWmi()
        {
            string NamespacePath = "\\\\.\\ROOT\\WMI\\ms_409";
            string ClassName = "WmiMonitorID";

            //Create ManagementClass
            ManagementClass oClass = new ManagementClass(NamespacePath + ":" + ClassName);

            //Get all instances of the class and enumerate them
            foreach (var o in oClass.GetInstances().OfType<ManagementObject>())
            {
                //access a property of the Management object
                Console.WriteLine("ManufacturerName : {0}", o["ManufacturerName"]);
            }
        }



        public void Load()
        {
//            Moving = true; //TODO : Hugly hack
            //SetPhysicalAuto();

            using (this.Suspend())
            {
                using (var k = OpenConfigRegKey())
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

                foreach (ScreenConfigs.Screen s in AllScreens)
                {
                    s.Load();
                }
                
            }

//            Moving = false;
            Saved = true;
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

                    foreach (ScreenConfigs.Screen s in AllScreens)
                        s.Save(k);

                    Saved = true;
                    return true;
                }
                return false;
            }
        }

        public ScreenConfigs.Screen PrimaryScreen => AllScreens.FirstOrDefault(s => s.Primary);

        ///// <summary>
        ///// Moving is true when screen is dragged on gui
        ///// </summary>
        //public bool Moving
        //{
        //    get => this.Get<bool>();
        //    set => this.Set(value);
        //}

        /// <summary>
        /// Mm Outside Bounds updated while moving (screen dragged on gui)
        /// </summary>
        //[TriggedOn(nameof(Moving))]
        [TriggedOn(nameof(AllScreens), "Item", "InMm.OutsideBounds")]
        public Rect PhysicalOutsideBounds => this.Get(() =>
        {
            var outside = new Rect();

            var first = true;
            foreach (var s in AllScreens)
            {
                if (first)
                {
                    outside = s.InMm.OutsideBounds;
                    first = false;
                    continue;
                }

                outside.Union(s.InMm.OutsideBounds);
            }

            return outside;

        });


        /// <summary>
        /// Mm Outside Bounds NOT updated while moving (screen dragged on gui)
        /// </summary>
        //public Rect MovingPhysicalOutsideBounds
        //{
        //    get => this.Get<Rect>();
        //    private set => this.Set(value);
        //}

        //public void ShiftMovingPhysicalBounds(Vector shift)
        //{
        //    Rect r = new Rect(
        //        MovingPhysicalOutsideBounds.TopLeft + shift
        //        , MovingPhysicalOutsideBounds.Size
        //        );
        //    MovingPhysicalOutsideBounds = r;
        //}

        //[TriggedOn(nameof(Moving))]
        //[TriggedOn(nameof(PhysicalOutsideBounds))]
        //private void UpdateMovingPhysicalOutsideBounds()
        //{
        //    if (Moving) return;
        //    MovingPhysicalOutsideBounds = PhysicalOutsideBounds;
        //}


        /// <summary>
        /// Mm Bounds of overall screens without borders
        /// </summary>
        [TriggedOn(nameof(AllScreens), "Item", "BoundsInMm")]
        public Rect PhysicalBounds => this.Get(() =>
        {
            var inside = new Rect();

            var first = true;
            foreach (var s in AllScreens)
            {
                if (first)
                {
                    inside = s.InMm.Bounds;
                    first = false;
                    continue;
                }

                inside.Union(s.InMm.Bounds);
            }

            return inside;               
        });


        /// <summary>
        /// 
        /// </summary>

        public bool Enabled
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        public bool LoadAtStartup
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) { Saved = false; } }
        }

        public bool IsRatio100
        {
            get
            {
                foreach (ScreenConfigs.Screen screen in AllScreens)
                {
                    if (screen.PixelToDipRatio.X != 1) return false;
                    if (screen.PixelToDipRatio.Y != 1) return false;
                }
                return true;
            }
        }

        public bool AdjustPointerAllowed => IsRatio100;

        public bool AdjustPointer
        {
            get => AdjustPointerAllowed && this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        public bool AdjustSpeedAllowed => IsRatio100;

        public bool AdjustSpeed
        {
            get => AdjustSpeedAllowed && this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        public bool AllowCornerCrossing
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }


        public bool HomeCinema
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        public Rect ConfigLocation
        {
            get => this.Get<Rect>(); set => this.Set(value);
        }


        public void SetPhysicalAuto(bool placeall = true)
        {
            if (PrimaryScreen == null) return;

            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }
            // List all screens not positioned
            List<ScreenConfigs.Screen> unatachedScreens = placeall?AllScreens.ToList():AllScreens.Where(s => !s.Placed).ToList();

            // start with primary screen
            Queue<ScreenConfigs.Screen> todo = new Queue<ScreenConfigs.Screen>();
            todo.Enqueue(PrimaryScreen);

            while (todo.Count > 0)
            {
                foreach (ScreenConfigs.Screen s2 in todo)
                {
                    unatachedScreens.Remove(s2);
                }

                ScreenConfigs.Screen placedScreen = todo.Dequeue();

                foreach (ScreenConfigs.Screen screenToPlace in unatachedScreens)
                {
                    if (screenToPlace == placedScreen) continue;

                    bool done = false;

                    //     __
                    //  __| A
                    // B  |__
                    //  __|
                    if (screenToPlace.InPixel.Bounds.X == placedScreen.InPixel.Bounds.Right)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.OutsideBounds.Right + screenToPlace.InMm.LeftBorder;
                        done = true;
                    }
                    //B |___|_
                    //A  |    |
                    if (screenToPlace.InPixel.Bounds.Y == placedScreen.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.OutsideBounds.Bottom + screenToPlace.InMm.TopBorder;
                        done = true;
                    }

                    //     __
                    //  __| B
                    // A  |__
                    //  __|
                    if (screenToPlace.InPixel.Bounds.Right == placedScreen.InPixel.Bounds.X)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.OutsideBounds.Left -
                                                  screenToPlace.InMm.OutsideBounds.Width + screenToPlace.InMm.LeftBorder;
                        done = true;
                    }

                    //A |___|_
                    //B  |    |

                    if (screenToPlace.InPixel.Bounds.Bottom == placedScreen.InPixel.Y)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.OutsideBounds.Top -
                                                  screenToPlace.InMm.OutsideBounds.Height + screenToPlace.InMm.TopBorder;
                        done = true;
                    }


                    //  __
                    // |
                    // |__
                    //  __
                    // |
                    // |__
                    if (screenToPlace.InPixel.Bounds.X == placedScreen.InPixel.Bounds.X)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.X;
                        done = true;
                    }

                    //  ___   ___
                    // |   | |   |
                    if (screenToPlace.InPixel.Bounds.Y == placedScreen.InPixel.Bounds.Y)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.Y;
                        done = true;
                    }

                    // __
                    //   |
                    // __|
                    // __
                    //   |
                    // __|
                    if (screenToPlace.InPixel.Bounds.Right == placedScreen.InPixel.Bounds.Right)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.Bounds.Right - screenToPlace.InMm.Bounds.Width;
                        done = true;
                    }

                    //|___||___|
                    if (screenToPlace.InPixel.Bounds.Bottom == placedScreen.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.Bounds.Bottom -
                                                  screenToPlace.InMm.Bounds.Height;
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
//            return;
            if (PrimaryScreen == null) return;


            //if (Moving) return;
            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }

            List<ScreenConfigs.Screen> done = new List<ScreenConfigs.Screen> {PrimaryScreen};

            List<ScreenConfigs.Screen> todo = AllBut(PrimaryScreen).OrderBy(s => s.Distance(PrimaryScreen)).ToList();

            while (todo.Count > 0)
            {
                ScreenConfigs.Screen screen = todo[0];
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


        public bool AllowOverlaps
        {
            get => this.Get<bool>(); set
            {
                if (this.Set(value))
                {
                    Saved = false;
                }
            }
        }

        public bool AllowDiscontinuity
        {
            get => this.Get<bool>(); set { if (this.Set(value)) Saved = false; }
        }

        public bool Saved
        {
            get => this.Get<bool>(); set => this.Set(value);
        }

        [TriggedOn(nameof(AllScreens),"Item.EffectiveDpiX")]
        public double MaxEffectiveDpiX => this.Get( 
            ()=>AllScreens.Count==0?0:AllScreens.Select(screen => screen.EffectiveDpi.X).Max());

        [TriggedOn(nameof(AllScreens),"Item.EffectiveDpiY")]
        public double MaxEffectiveDpiY => this.Get( 
            ()=>AllScreens.Count==0?0:AllScreens.Select(screen => screen.EffectiveDpi.Y).Max());

        [TriggedOn(nameof(AllScreens),"Item","DeviceNoAbs")]
        public int DeviceNoAbsMin => this.Get(() =>
        {
            return AllScreens.Count == 0 ? 0 : AllScreens.Min(s => s.DeviceNoAbs);
        });
    }
}
